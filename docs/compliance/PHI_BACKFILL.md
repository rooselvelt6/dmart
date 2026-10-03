# Backfill de PHI en reposo (P0.2)

Procedimiento one-shot para cifrar las filas legacy que quedaron con la PHI en
claro antes de la migración de envelopes. Es **idempotente**: sólo toca filas sin
`phi`, por lo que se puede relanzar sin miedo.

No es un proceso continuo. Si el servidor está en pie, `cipher()` usa
`DMART_MASTER_KEY` y las filas se leen igual: el backfill sólo cambia la
representación en reposo, nunca el contrato de la API.

## Uso

```bash
# 1. Simulación: no escribe nada, sólo cuenta y lista qué haría.
cargo run -p dmart-server --bin backfill_phi -- --dry-run

# 2. Aplicación sobre una copia de la base (nunca sobre producción en caliente).
cp data/dmart.db data/dmart.db.bak
cargo run -p dmart-server --bin backfill_phi -- --db-path data/dmart.db --table patients

# 3. Verificación y segunda pasada (debe decir 0 pendientes).
cargo run -p dmart-server --bin backfill_phi -- --dry-run
```

### Flags

| Flag | Por defecto | Efecto |
|---|---|---|
| `--dry-run` | `false` | Sólo informa: `pendientes=N sellados=0 errores=0`. |
| `--table` | `all` | `patients`, `measurements`, `push_subscription` o `all`. |
| `--tenant` | — | Acota a un hospital. No aplica a `push_subscription` (no tiene `tenant_id`): avisa por stderr y procesa la tabla entera. |
| `--batch-size` | `100` | Tamaño del lote de lectura por cursor. |
| `--max-errors` | `0` (ilimitado) | Corta la pasada al alcanzar N filas fallidas. `0` = sigue hasta el final. |
| `--db-path` | `$DMART_DB_PATH` o `./data/dmart.db` | Fichero `SurrealKV`. |

Códigos de salida:

| Código | Significado |
|---|---|
| `0` | Se recorrió todo y no hubo filas fallidas. |
| `1` | Alguna fila no se pudo cifrar, pero se recorrió la tabla entera. |
| `2` | Abortado por `--max-errors`: quedan filas pendientes. |
| `3` | Fallo duro: no se abrió la base o falta `DMART_MASTER_KEY`. No se escribió nada. |

### Clave maestra

El binario **no genera** clave. Si `DMART_MASTER_KEY` falta, está vacía o es la
clave efímera por defecto, aborta con código 3 antes de escribir. Es
deliberado: un envelope sellado con una clave que el servidor no tiene deja la
PHI ilegible de forma irreversible (`phi_backfill::require_master_key`). Sale con
código 3 sin haber tocado la base.

## Qué se cifra y qué queda en claro

| Tabla | En claro (columnas de filtro/operación) | Dentro de `phi` |
|---|---|---|
| `patients` | `patient_id`, `tenant_id`, `estado_gravedad`, `fecha_ingreso_uci`, `desenlace_uci`, `bi_*`, `created_at`, `updated_at` | historia clínica, cédula, nombre, apellido, dirección, fecha de nacimiento, edad, sexo, color de piel, nacionalidad, país, ciudad, lugar de nacimiento, estado, médico tratante |
| `measurements` | `measurement_id`, `patient_id`, `tenant_id`, timestamp, scores, `fingerprint`, `severity` | `apache_data`, `gcs_data`, `notas` |
| `push_subscription` | `user_id`, `endpoint`, `p256dh`, `auth`, `created_at` | `endpoint`, `user_agent` |

`endpoint` se mantiene en claro a propósito: sin la URL en claro no hay entrega
posible (es el destino del POST). Aun así viaja también dentro del envelope, que
es lo que ata la fila a su AAD, así que un envelope movido de fila no abre.

Los índices ciegos `bi_hc`, `bi_ced`, `bi_nombre` no se tocan: son
deterministas por diseño (HMAC) y son lo que permite seguir buscando por
historia clínica o cédula sin descifrar. El backfill no los recalcula ni los
borra.

### `notas` en `measurements`

`notas` es texto libre escrito por el clínico: es PHI. Antes de este trabajo era
una columna en claro de la fila; ahora vive dentro de `MeasurementPhi`. Por eso
las filas nuevas ya no la guardan en claro (`MeasurementRow` ya no tiene ese
campo) y `open_measurement` la lee del envelope, con default vacío para no
romper filas antiguas.

`open_measurement` y `open_push_sub` tienen una rama explícita para filas sin
`phi`: leen los valores legacy de la propia fila. Sin esa rama, el backfill
sellaría un envelope con los valores por defecto y **destruiría** los signos
vitales, el GCS o el `user_agent` de la fila. Si algún día se toca una de esas
dos funciones, ese test (`tests/phi_backfill.rs`) es el que lo detecta.

## Migraciones

| Fichero | Efecto |
|---|---|
| `migrations/051_phi_measurements.surql` | `DEFINE FIELD phi ON measurements` + índice de apoyo. |
| `migrations/055_phi_push.surql` | `DEFINE FIELD phi ON push_subscription` (tabla `SCHEMAFULL`, requiere el campo explícito). |

Se añaden al final de la lista en `src/migrations.rs`. No se ha tocado ninguna
migración ya aplicada. Los ficheros se numeran con la convención del repo; el
número de versión real es la posición en la lista.

## Recuperación ante fallos

El paso por lote usa un cursor sobre `patient_id` / `measurement_id` /
`endpoint`, así que una fila ilegible no hace girar el bucle: se cuenta como
error, se registra en `failed_ids` y el cursor avanza. Antes de arreglar el dato,
relanza con `--table <tabla> --max-errors 1` para localizar exactamente la fila
por su log.

Si el `UPDATE` no devuelve fila (id de record descuadrado del id lógico, típico de
importaciones manuales), el backfill **falla esa fila en vez de contarla como
sellada**: las métricas no mienten. Corrige el `patient_id` / `measurement_id` y
relanza.

Rollback: `cp data/dmart.db.bak data/dmart.db`. El backup previo es obligatorio
porque la operación no es reversible fila a fila (no hay copia del texto en
claro).

## Fuera de alcance

- `camas` (`CamaPhi.paciente_nombre`): ya tiene envelope y su propio flujo de
  despliegue; no se mezcla aquí.
- `care_plan`, `reports`: requieren decidir antes qué campos son PHI y cuáles no
  (su semántica es clínica, no de identidad), y tienen su propio AAD.
- `audit_logs`: son inmutables por diseño; cifrarlos rompería la trazabilidad.
  Requieren una decisión de retención explícita.
- `device_registry`: `fabricante`/`modelo` no son PHI, y `serial` es inventario,
  no dato de paciente. Se resuelve en una iteración posterior.

## Verificación

```bash
cargo test -p dmart-server --lib
cargo test -p dmart-server --test phi_backfill
```

`tests/phi_backfill.rs` cubre: sellado de las tres tablas con sus campos
legacy, ausencia de PHI en claro tras el backfill, rehidratación íntegra,
idempotencia (2ª pasada = 0), `--dry-run`, `--table`, `--tenant`, clave maestra
ausente (en subproceso, porque el cifrador es un `OnceLock` global), fila
ilegible sin bucle infinito y corte por `--max-errors`.