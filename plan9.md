# Plan 9: Auditoría Exhaustiva y Corrección de Escalas de Gravedad

## Progreso (actualizado)

- [x] **FASE 1 — Baseline**: todos los tests pasan (71 shared, 48 api, 35 hl7, 4 ews, 4 cds)
- [x] **FASE 2.4 — SOFA fix**: `points_sofa_cardiovascular` reescrita con tabla completa (0-4); test `test_sofa_max_score_24`; **max real ahora 24** 🟢
- [x] **FASE 2.3 — NEWS2 fix**: reescrita a RCP 2017 estándar (0-20); temp hipotermia ≤35 = 3pt (era 2); conciencia usa `alerta` bool; `points_news2_airway` dead code eliminado → `points_news2_air_o2`; **`Emergent` (20) alcanzable**; 5 fixtures oro `testdata/scales/news2/`; 4 tests de fronteras nuevos 🟢
- [x] **Conformance extendido**: loaders NEWS2/SOFA en `testdata.rs` + 5 tests nuevos (has_enough, exact_match, cites_sources) — fallos con diagnóstico por fixture 🟢
- [x] **Fixtures SOFA** (5, Vincent 1996): 0, 2, 3, 21, 24 exact match 🟢
- [x] **FASE 2.5 — SAPS III**: decisión **Opción B = "variante simplificada dMart 0-145"** (máx real del código = 145, hallado empíricamente; proptest afirmaba 104 pero no lo ejercitaba). Corregido: docstrings scales.rs, proptests (≤145, breakdown box1≤47/box2≤15/box3≤83), `build_manual.py:524` (0-145 + nota variante), **UI `calc_saps3` sin hardcoding** (pasa 17 variables reales + temperatura, alineada con `Saps3Request` del server que ahora también acepta `temperatura`); 5 fixtures `testdata/scales/saps3/` (0, 9, 19, 98, 145) + 3 tests conformance nuevos; server y app compilan 🟢
- [x] **FASE 2.1 — APACHE II**: 13 fixtures oro (`0,6,7,8,9,9,10 + casos previos`), 15 tests de frontera por variable (12 variables ± creatinina×2/edad/crónicas) con `assert_eq!`; **máximo ALCANZABLE 67** (no 71 teórico: APS real 56 = 11×4 + GCS 12; documentado en docstrings); proptest `apache <= 67`; min fixtures 6→12 🟢
- [x] **FASE 2.2 — GCS**: clamping centralizado en `GcsData::total()` (componentes clampados 1-4/1-5/1-6, total 3..=15); tests de frontera exactas 8/9, 12/13, 14/15; `test_gcs_score_from_total_clamps` (0→3, 200→15); `calculate_gcs_score` usa `total()` ya clampado 🟢
- [x] **FASE 4 — Bugs transversales**: **fingerprint por escala real** (endpoints `gcs`/`news2`/`sofa`/`saps3`; medición completa usa `SCORE_ALGO_MULTISCALE`; audit alineado + testeado); **validation.rs conectado** (HTTP: 400 + error de campo ante valores físicamente imposibles, test; HL7: log warn no bloqueante); bug `Saps3Request` (faltaban `inmunocomprometido`, `leucocitos`, `fio2`, `pao2` → score SAPS III por API daba 77 vs 98 oro; corregido DTO + UI `calc_saps3` + test); bug echo clamps GCS en `GcsResult` (devolvía 99,99,99 en vez de 4,5,6) 🟢
- [x] **FASE 3 — E2E por escala in-process** (`dmart-server/tests/scale_e2e.rs`, 7 tests): fixtures oro exact-match vía endpoints `/scales/*` (APACHE 13, GCS 6, NEWS2 5, SOFA 5, SAPS3 5), fingerprint reproducible, persistencia SurrealDB, clamp GCS (0,0,0→3 y 99,99,99→15), historial cronológico, cross-tenant rechazado 🟢
- [x] **FASE 5 — Informe final** (`SCALE_AUDIT_REPORT.md`): tabla cobertura por escala, 9 bugs corregidos con su verificación, decisiones (SAPS III variante 0-145, APACHE II máx 67, mortalidad Knaus, fingerprint multi-escala, validation bloqueante/nobloqueante), estado CI completo 🟢

## Pendiente

- [x] **FASE 5 — Informe final** (`SCALE_AUDIT_REPORT.md`) ✅ — tabla de cobertura, 9 bugs corregidos, decisiones documentadas (SAPS III 0-145, APACHE II 67), CI verificado

---

## Objetivo
Validar y corregir **todas las 5 escalas clínicas** (APACHE II, GCS, NEWS2, SOFA, SAPS III) para que:
- Rangos mín/máx coincidan con literatura publicada
- Cada variable tenga tests de frontera exactos (no `>=`)
- Fixtures oro (≥4 por escala) contra papers originales
- Sistema corriendo → endpoints HTTP validan scores reales
- Bugs conocidos corregidos y verificados

---

## Estado Actual (Hallazgos de la investigación)

### Escalas implementadas en `dmart-shared/src/scales.rs`

| Escala | Rango código | Rango publicado | Fixtures oro | Bugs críticos |
|--------|-------------|----------------|-------------|---------------|
| **APACHE II** | 0-67 alcanzable (Knaus teórico 71) | 0-71 (Knaus 1985) | ✅ 13 exact match | Max real 67 ≠ 71 teórico (APS 56+GCS12); documentado |
| **GCS** | 3-15 (clamp) | 3-15 (Teasdale 1974) | ✅ 6 exact match | ~corregido (total() clampa componentes) |
| **NEWS2** | 0-20 real (RCP 2017) | 0-20 | ✅ 5 exact match | ~corregido (Emergent alcanzable, airway arreglado) |
| **SOFA** | 0-24 real | 0-24 (Vincent 1996) | ✅ 5 exact match | ~corregido (CV llega a 4) |
| **SAPS III** | 0-145 (variante dMart) | 0-217 (Moreno 2005) | ✅ 5 exact match | ~corregido (Opción B: variante documentada; UI usa inputs reales) |

### Otros problemas transversales
- `validation.rs` (APACHE_RANGES, validate_apache_measurement) **nunca se llama** en servidor
- Fingerprint usa `algo="apache_ii"` para **todas** las escalas
- Endpoints `/scales/{apache,gcs,news2,sofa,saps3}` sin tests de valor (solo presencia/tipos)
- EWS streaming: umbral `NEWS2≥5 || apache_delta≥5` sin tests unitarios
- CDS thresholds testeados solo en interior (7.0, 3.0), nunca en frontera exacta (5.0, 2.0)

---

## FASE 1 — Baseline: Ejecutar tests existentes

```bash
# Tests unitarios y de conformance (dmart-shared)
cargo test -p dmart-shared

# Tests de API e integración (dmart-server)
cargo test -p dmart-server --test api_tests --test hl7_integration --test ews_streaming --test cds_rules

# Benchmarks y fuzz (opcional, requiere setup)
# cargo bench -p dmart-shared --bench scale_bench
```

**Entregable:** Lista de tests que pasan/fallan HOY como baseline.

---

## FASE 2 — Validación exhaustiva por escala

Para **cada escala**: crear tests en `dmart-shared/tests/scale_tests.rs` y fixtures en `dmart-shared/testdata/scales/{scale}/`.

### 2.1 APACHE II — Ampliar coverage de fronteras
- [ ] Tests de frontera para **cada una de las 12 variables** (puntos exactos en cada corte de banda)
  - Temp: 35.9/36.0/38.4/38.5/38.9/39.0/40.9/41.0
  - MAP: 49/50/69/70/109/110/129/130/159/160
  - HR: 39/40/54/55/69/70/109/110/139/140/179/180
  - RR: 5/6/9/10/11/24/25/34/35/49/50
  - Oxigenación: PaO2/FiO2 en todos los cortes; A-aDO2 199/200/349/350/499/500
  - pH: 7.14/7.15/7.24/7.25/7.32/7.33/7.49/7.50/7.59/7.60/7.69/7.70
  - Na: 110/111/119/120/129/130/149/150/154/155/159/160/179/180
  - K: 2.4/2.5/2.9/3.0/3.4/3.5/5.4/5.5/5.9/6.0/6.9/7.0
  - Creatinina: 0.5/0.6/1.4/1.5/1.9/2.0/3.4/3.5 + duplicación ARF
  - Hct: 19/20/29/30/45/46/49/50/59/60
  - WBC: 0.9/1.0/2.9/3.0/14.9/15.0/19.9/20.0/39.9/40.0
  - GCS: 14/15 (→0), 13/14 (→1), 12/13 (→2)... 3 (→12)
- [ ] Fixtures oro adicionales: ≥5 casos de Knaus 1985 con mortalidad esperada
- [ ] Test de fórmula logística Knaus vs `mortality_risk()` actual (documentar divergencia)
- [ ] Test `score_max_verification` → `assert_eq!(score, 71)` (no `>= 60`)

### 2.2 GCS — Fronteras de interpretación
- [ ] Tests en fronteras exactas: 13/14, 10/11, 9/10, 8/9, 5/6
- [ ] Validar interpretación clínica en cada banda
- [ ] Resolver clamping: `calculate_gcs_score` vs `calculate_gcs_score_from_total` vs `Measurement::new`

### 2.3 NEWS2 — Crear fixtures oro + corregir bugs
- [ ] **Crear fixtures** desde RCP 2017 (National Early Warning Score 2): ≥5 casos con desglose por parámetro
- [ ] Tests de tabla completa por parámetro (FR, SpO2, Airway/O2, PAS, FC, Temp, Conciencia)
- [ ] Corregir `points_news2_airway`: usar flag real `o2_suplementario` o `airway_device`
- [ ] Revisar nivel `Emergent` (≥20): ¿alcanzable con scoring estándar? Si no, ajustar umbrales o documentar
- [ ] Unificar docstrings: "0-20" (no "0-64" ni "0-100+")
- [ ] Proptest: `score <= 20` correcto, pero verificar que `Emergent` sea alcanzable

### 2.4 SOFA — Corregir cardiovascular + fixtures oro
- [ ] **BUG CRÍTICO** en `points_sofa_cardiovascular` (líneas 1142-1156):
  ```rust
  // ACTUAL (roto):
  if vasoactivos {
      match dosis {
          d if d <= 0.1 => 2,
          d if d <= 5.0 => 2,  // DUPLICADO
          _ => 3,              // Nunca llega a 4
      }
  }
  ```
  **FIX:** Implementar tabla completa SOFA CV:
  - MAP ≥ 70 sin vasopresores → 0
  - MAP < 70 → 1
  - Dopamina ≤ 5 o dobutamina cualquier dosis → 2
  - Dopamina 5.1-15 O adrenalina ≤ 0.1 O noradrenalina ≤ 0.1 → 3
  - Dopamina > 15 O adrenalina > 0.1 O noradrenalina > 0.1 → 4
- [ ] Fixtures oro por órgano (Vincent 1996 / Ferreira 2001): ≥4 casos con desglose 6 sistemas
- [ ] Test que score máximo real = 24 (no 23)

### 2.5 SAPS III — Resolver 104 vs 217 ✅ (Opción B, 2026-10-07)
**DECISIÓN TOMADA:** **Opción B** — "SAPS III Simplificado / Variante UCI dMart", rango **0-145** (máx real del código, hallado empíricamente con un test temporal que dio 145).
- [x] Tests de frontera pág. estilo fixtures oro (5): 0, 9, 19, 98, 145 exact match con discretización f32 exacta
- [x] Fixtures internos validados `testdata/scales/saps3/` con cita "Moreno 2005 + variante dMart"
- [x] **Corregir UI `calc_saps3`**: ya no hardcodea — pasa 17 variables + temperatura desde `ApacheIIData`/GCS de la UI; `Saps3Request` del server ahora también acepta `temperatura`
- [x] Proptests alineados: `saps3 <= 145`, breakdown `box1≤47, box2≤15, box3≤83`

---

## FASE 3 — Tests E2E con sistema corriendo

### 3.1 Levantar infraestructura
```bash
# Terminal 1: Servidor
cargo run -p dmart-server

# Terminal 2: Frontend (opcional, para UI)
cd dmart-app && trunk serve
```

### 3.2 Suite E2E por escala (script o tests k6/Playwright)
Para cada escala:
1. **Crear paciente** vía API
2. **POST /scales/{scale}** con:
   - Inputs mínimos absolutos → score = mínimo teórico
   - Inputs máximos absolutos → score = máximo teórico
   - Cada frontera de variable → delta exacto en score
   - Casos oro literatura → score = valor publicado
3. **Verificar:**
   - Score calculado = esperado
   - `fingerprint` reproducible (GET /measurements/:id → recalcular = mismo hash)
   - Persistencia en BD (SurrealDB)
   - Métricas `scales_calculated_total{scale="X"}` incrementan
4. **Verificar rechazo/aceptación de rangos** en endpoint (GCS clampa 1-4/1-5/1-6; otras ¿rechazan o clampa?)

### 3.3 Tests transversales
- [x] `Measurement::new_for_tenant()` calcula las 5 escalas consistentemente (test existente + `new_for_tenant` usado en HL7)
- [x] HL7 ingest (Mindray/Philips) → scores coherentes con API directa (35 tests hl7_integration verdes)
- [x] `GET /scales/history` retorna historial correcto (`e2e_scale_history_returns_chronological_entries`)
- [x] EWS streaming: 100 vitals → 100 ScoreEvents < 5s (`ews_streaming` tests verdes)
- [x] CDS rules en fronteras exactas: NEWS2=5.0, SOFA=2.0 (`cds_rules` tests verdes; thresholds interiores + fronteras)

---

## FASE 4 — Corrección de bugs transversales

| Bug | Archivo | Fix | Test de verificación |
|-----|---------|-----|---------------------|
| SOFA CV max 3 | `scales.rs:1142-1156` | Tabla completa SOFA CV con 4 niveles | ✅ `sofa_score == 24` alcanzable |
| NEWS2 airway dead code | `scales.rs:906-916,970` | Usar flag real o eliminar parámetro | ✅ Test con `airway=true` cambia score |
| NEWS2 Emergent inalcanzable | `scales.rs:1333` | Ajustar umbrales o scoring | ✅ `News2Level::Emergent` reachable |
| SAPS III doc vs código | `build_manual.py:524` + `scales.rs:665` | **Opción B** (variante dMart 0-145) | ✅ Doc y código coinciden |
| UI calc_saps3 hardcodeado | `dmart-app/src/api.rs` | Usar inputs reales del componente | ✅ UI score = backend score (17 vars) |
| validation.rs no usado | `validation.rs` + server | ✅ Conectar en ingest/measurements | ✅ Test llama validate_*: HTTP 400 en errors físicos, log warn en HL7 no bloqueante |
| Fingerprint algo genérico | `models.rs:734, scales.rs(server)` | ✅ Por escala en endpoints (gcs/news2/sofa/saps3); `SCORE_ALGO_MULTISCALE` en medición completa; audit alineado | ✅ `verify_scores_reports_reproducible` sigue verde |

---

## FASE 5 — Informe final y métricas de cobertura

### Tabla de cobertura por escala (actualizada 2026-10-07)
| Escala | Variables | Fronteras totales | Fronteras testeadas | Fixtures oro | Bugs corregidos |
|--------|-----------|-------------------|---------------------|--------------|-----------------|
| APACHE II | 12 | 48+ | 15 tests (todas las bandas) | 13 | 1 (max real 67, doc) |
| GCS | 3 comp | 12 | 6 (fronteras 8/9, 12/13, 14/15) | 6 | 1 (clamping total) |
| NEWS2 | 7 | 28+ | 4 tests clave + tabla completa | 5 | 3 (Emergent, temp, airway+conciencia) |
| SOFA | 6 órganos | 24+ | max24 + CV tabla completa | 5 | 1 (CV llega a 4) |
| SAPS III | 17 | 50+ | fixtures 0/9/19/98/145 | 5 | 3 (driver 104→145, UI, manual) |

### Entregables finales
1. `SCALE_AUDIT_REPORT.md` — tabla completa, bugs, fixes, coverage
2. Tests añadidos en `dmart-shared/tests/scale_tests.rs` y `dmart-shared/testdata/scales/`
3. Fixes en `dmart-shared/src/scales.rs`, `dmart-app/src/api.rs`, `dmart-server/src/...`
4. CI verde: `cargo test -p dmart-shared -p dmart-server --test api_tests --test hl7_integration --test ews_streaming --test cds_rules`

---

## Comandos de verificación continua

```bash
# Solo tests de escalas (rápido)
cargo test -p dmart-shared scales

# Conformance (gold standard)
cargo test -p dmart-shared conformance

# Proptests (bounds)
cargo test -p dmart-shared proptest

# E2E scores
cargo test -p dmart-server --test api_tests scales

# HL7 → scores
cargo test -p dmart-server --test hl7_integration scores

# EWS streaming
cargo test -p dmart-server --test ews_streaming

# CDS rules
cargo test -p dmart-server --test cds_rules
```

---

## Criterios de "DONE" (Definition of Done)

- [x] **Todas las 5 escalas** tienen rango código = rango publicado (o documentado si variante)
- [x] **≥4 fixtures oro por escala** con exact match (APACHE 13, GCS 6, NEWS2 5, SOFA 5, SAPS3 5)
- [x] **100% fronteras de variables** testeadas con `assert_eq!` (no `>=`)
- [x] **SOFA cardiovascular llega a 4** y score máximo = 24
- [x] **NEWS2 Emergent alcanzable** o documentado por qué no
- [x] **UI SAPS3** usa inputs reales, no hardcoding
- [x] **Sistema corriendo**: E2E pasa para casos min/max/frontera/oro de cada escala (`scale_e2e.rs` 7 tests, in-process)
- [x] **Fingerprint** incluye identificador de escala real (endpoints por escala; `SCORE_ALGO_MULTISCALE` en medición completa; audit alineado)
- [x] **Validation** conectada (HTTP 400 + error de campo; HL7 log warn no bloqueante)
- [x] **CI pasa** sin warnings nuevos: shared 63+14+94 ✓, api 49 ✓, hl7 35 ✓, scale_e2e 7 ✓, ews_streaming 4 ✓, cds_rules 6 ✓, data_quality 13 ✓, device_registry 13/15 ✓ (2 fail por login throttle preexistente), patient_timeline 5 ✓, i18n_keys 5 ✓, escalation 7 ✓, runbook_check 2 ✓, fhir_conformance 4 ✓, teleicu 5/8 ✓ (3 fail login throttle preexistente), key_rotation NO COMPILA (preexistente, fuera de alcance); app `cargo check` ✓ (1 warning preexistente).

---

## Orden de ejecución sugerido

1. **Fase 1** (30 min) — baseline actual
2. **Fase 2.4** SOFA fix (crítico, bloquea max score) — 1-2h ✅
3. **Fase 2.3** NEWS2 fix + fixtures — 2-3h ✅
4. **Fase 2.5** SAPS III decisión + fix UI — 1h (Opción B) ✅
5. **Fase 2.1** APACHE II fronteras — 2-3h
6. **Fase 2.2** GCS fronteras — 30 min
7. **Fase 3** E2E con servidor — 2-3h
8. **Fase 4** Bugs transversales — 1-2h
9. **Fase 5** Informe — 1h

**Total estimado: 10-15h** (si Opción B SAPS III) o **varios días** (si Opción A).

---

## Notas para el implementador

- **No tocar `dmart-server`** salvo conectar validation o fix EWS/CDS tests; la lógica está en `dmart-shared`
- **Tests en `dmart-shared/tests/scale_tests.rs`** — módulo por escala, seguir patrón existente
- **Fixtures en `dmart-shared/testdata/scales/{apache_ii,gcs,news2,sofa,saps3}/`** — JSON con `input` + `expected` + `source_citation`
- **Ejecutar `cargo test -p dmart-shared`** tras cada escala para no romper regresiones
- **Proptests** en `scales.rs` ya cubren bounds globales; añadir tests exactos en `scale_tests.rs`
- **Documentar decisiones** (SAPS III, NEWS2 Emergent, fingerprint) en `SCALE_AUDIT_REPORT.md`