# Rotación de claves de PHI (P1.3)

Cómo cambiar la clave con la que se cifra la PHI **sin downtime** y sin dejar
filas ilegibles.

- **Qué se rota**: la clave maestra de la que se derivan las subclaves de
  cifrado de PHI (`AES-256-GCM`) y, por separado, la de índices ciegos.
- **Qué NO rota**: los índices ciegos (`bi_hc`, `bi_ced`, `bi_nombre`). Derivan
  de la clave del `kid` `default` y se quedan ahí; si rotaran, las búsquedas por
  MRN/cédula devolverían 0 resultados sin ningún error visible.
- **Alcance**: `patients`, `measurements`, `push_subscription`, `camas`,
  `care_plan` y `audit_logs` (todo lo que tiene columna `phi`).

## Cómo funciona

Cada envelope lleva dentro el `kid` de la clave con la que se escribió:

```text
v2.<kid>.<base64(DMART_A2 || nonce(12) || ciphertext || tag_GCM(16))>
```

Al leer se usa **la clave que declara el envelope**, no la que esté activa. Un
`kid` sin clave en el keyring es un error explícito (`UnknownKeyId`) que nombra
la clave que falta: no se prueban las demás a ciegas, porque devolver PHI
legible con una clave equivocada sería peor que fallar.

Los envelopes anteriores a P1.3 son el mismo base64 **sin** prefijo, y se
resuelven contra el `kid` `default` (la clave de `DMART_MASTER_KEY`). Por eso
desplegar esta versión no necesita migración: no se toca ni una fila.

El `kid` va dentro del texto del envelope y no en una columna aparte porque las
tablas PHI son `SCHEMAFULL`: una columna nueva sin `ALTER` se descartaría en
silencio.

## Procedimiento

Nueve pasos. Los cuatro primeros no detienen el servicio; a partir del 5 sí hace
falto reiniciar, y el 8 y el 9 sí escriben.

### 1. Generar la clave nueva

```bash
openssl rand -base64 32
```

Anótala en la bóveda de secretos. Es un valor de 32 bytes en base64; el
servidor no valida "fortaleza" del keyring porque 32 bytes aleatorios de `rand`
ya la son (sí valida que la clave sea de **exactamente** 32 bytes y que el
formato sea correcto).

### 2. Desplegar el keyring con la clave nueva ya cargada, sin activarla

```bash
DMART_MASTER_KEYS=default=<clave-actual>,k2026-03=<clave-nueva>
DMART_ACTIVE_KEY_ID=default
```

Se reinicia. Desde este momento el proceso **lee** con las dos claves y
**escribe** con `default`, o sea exactamente como antes. Es el estado en el que
se puede estar sin riesgo.

Comprueba que arrancó y con qué clave:

```bash
# en los logs de arranque:
# cifrado de PHI: keyring de rotación (DMART_MASTER_KEYS) active=default kids=["default","k2026-03"]
```

### 3. Activar la clave nueva (sigue escribiendo con la vieja)

```bash
DMART_ACTIVE_KEY_ID=k2026-03
```

Se reinicia. A partir de aquí, lo nuevo se cifra con `k2026-03` y lo antiguo se
sigue leyendo con `default`. **Este es el estado en el que se puede estar
indefinidamente**: es seguro y no requiere escribir nada.

### 4. Re-cifrar el histórico (por lotes, reanudable)

Primero en seco, para ver el trabajo:

```bash
DMART_MASTER_KEYS=... DMART_ACTIVE_KEY_ID=k2026-03 \
  cargo run --release -p dmart-server --bin backfill_phi -- --reencrypt --dry-run
```

Si el `--dry-run` dice 0 pendientes, sáltate al paso 6.

Y luego de verdad, con la guardia de `--expect-key`:

```bash
DMART_MASTER_KEYS=... DMART_ACTIVE_KEY_ID=k2026-03 \
  cargo run --release -p dmart-server --bin backfill_phi -- --reencrypt --expect-key k2026-03
```

`--expect-key` hace que el job aborte si la clave activa del proceso no es la que
crees: sin él, ejecutarlo con el `DMART_ACTIVE_KEY_ID` equivocado "terminaría"
con éxito sin haber movido nada, porque todo lo que encuentra ya estaría bajo
la clave que el proceso cree activa.

El job es **idempotente**: cada fila se re-sella con un único `UPDATE` atómico
y deja de ser seleccionada por el filtro, así que se puede cortar (`Ctrl-C`,
caída, límite de `--max-errors`) y reanudar. Si el proceso muere a mitad, la
siguiente pasada sigue donde quedó.

Notas del job:

- Re-sella también los envelopes **v1** (sin `kid`) que estaban bajo `default`:
  al reescribirlos, su `kid` queda explícito.
- **No toca** las filas sin envelope (`phi` vacío): eso es el backfill de
  SPEC-052, `backfill_phi` sin `--reencrypt`.
- Recalcula los índices ciegos de `patients`, pero no cambian de valor (ver
  arriba).
- No toca `care_plan` ni `audit_logs`: el `details` de la auditoría forma parte
  del hash de la cadena WORM, y re-sellarlo aquí rompería la verificación.

Códigos de salida: `0` sin errores, `1` quedan filas para la siguiente pasada,
`2` se cortó por `--max-errors`, `3` no pudo ni empezar (falta la clave, base
ilegible, `--expect-key` descuadrado).

Acota el trabajo igual que el backfill:

```bash
--table patients --tenant hosp-a --batch-size 500 --max-errors 20
```

### 5. Verificar que ya no queda nada bajo la clave vieja

```bash
cargo run --release -p dmart-server --bin backfill_phi -- --reencrypt --dry-run --expect-key k2026-03
```

Debe decir `a re-cifrar=0`. Un número distinto significa que hay escrituras
nuevas con la clave vieja (señal de que el paso 3 no está en todos los
despliegues) o filas que fallan.

Comprobación en la base de datos, sin depender del job:

```surql
SELECT count() FROM patients WHERE phi != NONE AND !string::starts_with(phi, 'v2.k2026-03.') GROUP ALL;
```

### 6. Retirar la clave vieja del keyring

```bash
DMART_MASTER_KEYS=k2026-03=<clave-nueva>
DMART_ACTIVE_KEY_ID=k2026-03
```

Se reinicia. **No retires `default` si quedan filas con `bi_hc`/`bi_ced`/
`bi_nombre`**: la clave de índices ciegos deriva de ella y, si desaparece, las
búsquedas por MRN/cédula dejan de encontrar nada. Es el único elemento del
keyring que no se puede rotar junto con el resto.

### 7. Reiniciar todos los despliegues

Todavía no se puede borrar la clave vieja de la bóveda: hay despliegues viejos
(rollbacks, réplicas de lectura) que la siguen necesitando.

### 8. Verificar en la aplicación

- Abrir un paciente de los más antiguos (cifrado en el paso 2 o antes).
- Buscar por MRN y por cédula: debe encontrarlo.
- Escribir un paciente nuevo y comprobar que su envelope empieza por
  `v2.k2026-03.`.

### 9. Retirar la clave vieja de la bóveda

Sólo cuando ningún despliegue usa `default` y el backup ya no la necesita. Un
backup restaurado con la clave vieja **no** es restaurable sin ella: es el coste
de rotar, y la razón de que los pasos 2–3 existan (durante ellos la clave vieja
sigue en el keyring aunque ya no se use para escribir).

## Rotar la clave de índices ciegos

No se hace con este procedimiento. Es un cambio incompatible hacia adelante:
hay que reindexar las columnas `bi_*`, y las búsquedas dejan de funcionar
hasta que lo estén. Requiere un `UPDATE ... SET bi_hc = ...` sobre todas las
filas, fuera del alcance de P1.3.

## Qué hacer si algo falla

| Síntoma | Causa | Qué hacer |
|---|---|---|
| El arranque falla con `DMART_ACTIVE_KEY_ID is required when DMART_MASTER_KEYS has more than one key` | Hay varias claves y no se dijo cuál cifra | Poner `DMART_ACTIVE_KEY_ID` |
| El arranque falla con `no está en DMART_MASTER_KEYS` | El `kid` activo no existe en el keyring | Corregir el nombre; no inventar uno nuevo |
| Error `no es base64 válido` / `no son 32 bytes` al arrancar | Clave mal copiada (base64 roto o de otra longitud) | `openssl rand -base64 32` de nuevo y volver a copiar |
| Un `GET` falla con `PHI envelope cifrado con la clave 'k...', que este despliegue no tiene cargada` | Falta una clave del keyring | Restituirla en `DMART_MASTER_KEYS` y reiniciar. **No** es corrupción de la fila |
| Un `GET` falla con `PHI envelope inválido` | Envelope alterado, copiado entre registros, o de otro tenant | Investigar como corrupción de datos; no se arregla re-cifrando |
| El job termina con código 1 | Quedaron filas sin procesar | Mirar `failed_ids` en el log y relanzar |
| El job termina con código 2 | `--max-errors` alcanzado | Correger la causa de los fallos y relanzar |
| `--expect-key` aborta | `DMART_ACTIVE_KEY_ID` no es el esperado | Arranca el job con el entorno correcto |

## Ver también

- `docs/compliance/PHI_BACKFILL.md`: sellado de filas legacy (`phi` vacío).
- `.env.example`: las variables con su comentario.
- `dmart-server/src/crypto.rs`: `KeyProvider`, formato del envelope y la
  matriz de lectura.
- `dmart-server/src/phi_backfill.rs`: filtro, cursor y métricas del job.
