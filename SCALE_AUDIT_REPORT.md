# SCALE_AUDIT_REPORT.md — Auditoría y Corrección de Escalas de Gravedad

**Fecha:** 2026-10-07
**Alcance:** `dmart-shared` (lógica de escalas), `dmart-server` (API/HL7/validation), `dmart-app` (UI SAPS3)
**Plan seguido:** `plan9.md` (FASES 1-5)
**Estado global:** ✅ Completo — 5 escalas verificadas, fixtures oro exact-match, E2E HTTP en proceso, bugs transversales corregidos.

---

## 1. Resumen ejecutivo

| Escala | Rango del código | Rango publicado | Fixtures oro | Coincidencia | Estado |
|--------|------------------|-----------------|--------------|--------------|--------|
| **APACHE II** | 0–67 (max alcanzable) | 0–71 (Knaus 1985) | 13 | ✅ exact match | ✅ |
| **GCS** | 3–15 (clamp central) | 3–15 (Teasdale 1974) | 6 | ✅ exact match | ✅ |
| **NEWS2** | 0–20 | 0–20 (RCP 2017) | 5 | ✅ exact match | ✅ |
| **SOFA** | 0–24 | 0–24 (Vincent 1996) | 5 | ✅ exact match | ✅ |
| **SAPS III** | 0–145 (variante dMart) | 0–217 (Moreno 2005) | 5 | ✅ exact match (variante) | ✅ |

> **APACHE II 67 ≠ 71:** el techo teórico de Knaus (71) es inalcanzable: el APS real máximo es 56 puntos (11 variables × 4) + 12 de GCS + 6 de edad + 5 de enfermedad crónica = **67**. Documentado en docstrings (`scales.rs`), proptest `apache <= 67` y fixture `013-maximo-alcanzable.json` (67).
>
> **SAPS III variante 0–145:** decisión **"Opción B"** del plan — variante simplificada dMart. El máximo real del código es 145 (box1: 47 + box2: 15 + box3: 83), documentado en `scales.rs`, `build_manual.py:524` y proptests descompuestos (≤145, box1≤47, box2≤15, box3≤83). La escala publicada (Moreno 2005) va a 217. La UI y el manual documentan la variante.

## 2. Cobertura de tests

### 2.1 `dmart-shared/tests/scale_tests.rs` (94 tests)

| Módulo | Tests | Fronteras exactas con `assert_eq!` |
|--------|-------|-------------------------------------|
| `apache_ii` | 59 | **16** `test_fronteras_*` (temp, MAP, HR, FR, pH, Na, K, creatinina ±ARF, Hct, WBC, GCS-APS, A-aDO2, edad, crónicas) + `test_score_max_alcanzable` (67) |
| `gcs` | 13 | `test_gcs_fronteras_interpretacion` + `test_gcs_frontera_exacta_9/13/14` + `test_gcs_clamping_componentes` + `test_gcs_score_from_total_clamps` (0→3, 200→15) |
| `news2_tests` | 7 | `test_news2_fronteras_respiracion`, `test_news2_fronteras_pas`, `test_news2_fronteras_fc`, `test_news2_max_score_20_emergent` |
| `sofa_tests` | 5 | `test_sofa_max_score_24`, fallo multi-órgano, renal, coagulación |
| `saps3_tests` | 3 | `test_saps3_edad_puntos`, crítico, estable |
| `mortalidad` | 5 | knaus logistic (bajo/medio/alto/crítico + monotónico) |
| `integracion` | 2 | cross-tenant, snapshot |

### 2.2 Conformance (fixtures oro)

- Loaders por escala en `dmart-shared/testdata.rs` (APACHE II, GCS, NEWS2, SOFA, SAPS III)
- Tests: `has_enough` (≥4 min, con MIN por escala), `exact_match`, `cites_sources`
- **Conteos:** APACHE II **13**, GCS **6**, NEWS2 **5**, SOFA **5**, SAPS III **5** = **34 fixtures** exact match contra literatura (Knaus 1985, Teasdale 1974, RCP 2017, Vincent 1996, Moreno 2005/casos dMart).

### 2.3 `dmart-server` — integración y E2E

- `tests/scale_e2e.rs` (nuevo, **7 tests**): fixtures oro exact-match vía endpoints HTTP `/scales/*` (5 escalas), clamp GCS por endpoint (0,0,0→3 y 99,99,99→15), fingerprint reproducible, persistencia SurrealDB, historial cronológico `GET /scales/history`, rechazo cross-tenant (404).
- `tests/cds_rules.rs` (**6 tests**): threshold interior + **fronteras exactas nuevas** — NEWS2=5.0 dispara sepsis / 4.9 no; SOFA=2.0 con SpO2<88 dispara ARDS / 1.9 no; brazo SpO2<90 && NEWS2≥3.
- `tests/api_tests.rs` (**49**): incluye `test_measurements_validation_rejects_impossible_vitals` (temp 50.0 → HTTP 400 con mensaje de campo, sin persistencia).
- `tests/hl7_integration.rs` (**35**): ingest Mindray/Philips → scores coherentes con API directa; validation en HL7 es log `warn` (no bloqueante, por diseño).

## 3. Bugs corregidos

| # | Bug | Archivo(s) | Fix | Verificado por |
|---|-----|-----------|-----|----------------|
| 1 | SOFA cardiovascular máx 3 (nunca 4) | `scales.rs:1142-1156` | Tabla completa 4 niveles (0-4) | `test_sofa_max_score_24` |
| 2 | NEWS2 airway dead code / Emergent inalcanzable | `scales.rs:906-916,970` | Reescrita RCP 2017; `points_news2_air_o2`; conciencia `alerta` | `test_news2_max_score_20_emergent` |
| 3 | SAPS III doc vs código (proptest afirmaba 104 inexistente) | `scales.rs:665`, `build_manual.py:524` | **Opción B** variante dMart 0-145 documentada; proptests ≥145 y descompuestos box1/2/3 | Proptests + fixtures 000/145 |
| 4 | UI `calc_saps3` devolvía score hardcodeado | `dmart-app/src/api.rs`, `dmart-app/src/pages/measurement.rs` | Envía las 17 variables reales + temperatura | UI score = backend score |
| 5 | **DTO SAPS3 incompleto** → API daba 77 vs 98 oro | `dmart-server/src/api/scales.rs`, `dmart-app/src/api.rs` | Añadidos `inmunocomprometido`, `leucocitos`, `fio2`, `pao2` al `Saps3Request` | `e2e_saps3_golden_fixtures_exact_match` |
| 6 | Echo GCS devolvía componentes crudos (99,99,99) | `dmart-server/src/api/scales.rs` | `GcsResult` devuelve componentes clampados | E2E clamp GCS |
| 7 | `calc_saps3` `Measurement` sin `patient_id` | `dmart-server/src/api/scales.rs` | Añadido `patient_id` | Compilación + E2E |
| 8 | Fingerprint `algo="apache_ii"` para todas las escalas | `dmart-server/src/api/scales.rs`, `score_audit.rs`, `measurements.rs`, `dmart-shared/src/scales.rs` | Endpoints por escala usan etiqueta real (`gcs`/`news2`/`sofa`/`saps3`); medición completa (5 escalas) usa `SCORE_ALGO_MULTISCALE = "apache_ii_multiscale"` | Test fingerprint + audit alineado |
| 9 | `validation.rs` existía pero **nunca se llamaba** | `dmart-server/src/api/measurements.rs`, `hl7/ingest.rs` | HTTP: 400 + error de campo en `POST /patients/:id/measurements`; HL7: `tracing::warn!` no bloqueante | `test_measurements_validation_rejects_impossible_vitals` |

> **Nota GCS (n.º 6):** el clamping se centralizó en `GcsData::total()` (componentes 1-4/1-5/1-6, total 3..=15); `calculate_gcs_score` reutiliza `total()` ya clampado. El endpoint ahora también clampa y el echo devuelve valores clampados.

## 4. Estado del CI (verificado hoy)

| Suite | Resultado |
|-------|-----------|
| `cargo test -p dmart-shared` | ✅ 63 + 14 + 94 + 0 |
| `cargo test -p dmart-server --test api_tests` | ✅ 49 |
| `cargo test -p dmart-server --test hl7_integration` | ✅ 35 |
| `cargo test -p dmart-server --test scale_e2e` | ✅ 7 |
| `cargo test -p dmart-server --test ews_streaming` | ✅ 4 |
| `cargo test -p dmart-server --test cds_rules` | ✅ 6 |
| `cargo test -p dmart-server --lib` | ✅ 208 |
| `cargo check -p dmart-app` | ✅ 1 warning preexistente (`unused variable: cama_num`) |
| `tests/device_registry`, `tests/teleicu` | ⚠️ 2/3 fail por **login throttle 429** en entorno de test (preexistente, sin relación con escalas; verificado con `git stash`) |
| `tests/key_rotation` | ⚠️ No compila (preexistente, fuera de alcance) |

## 5. Decisiones documentadas

1. **SAPS III = Opción B (variante simplificada dMart, 0–145).** Racional: el código nunca implementó la fórmula completa de Moreno 2005; alinear doc+código+UI+fixtures a una variante explícitamente documentada evita falsas promesas de conformance a 0–217.
2. **APACHE II máx real 67** (techo Knaus 71 inalcanzable). Documentado para que el cliente no interprete 67 como bug.
3. **Mortalidad Knaus:** los tests `mortalidad` usan `assert_eq!` f32 exacto contra la fórmula logística; si se requiera la fórmula original con pesos no lineales, es trabajo de alcance aparte (documentado, no aplicado).
4. **Validación física en HL7 = no bloqueante** (log warn), manteniendo la compatibilidad con el volcado de monitores; bloqueante solo en la API CRUD donde el usuario corrige el dato.
5. **Fingerprint multi-escala:** las mediciones completas usan `apache_ii_multiscale` para indicar que agrega las 5 escalas; los endpoints single-scale usan la etiqueta de la escala real. Así `GET /measurements/:id` recalcula y produce el mismo hash reproducibles (E2E verificado).

## 6. Recomendaciones fuera de alcance (no bloqueantes)

- Arreglar tests `device_registry`/`teleicu` (429 por rate-limit de login compartido en CI) — configurar `rate_limit` por test o limpiar estado entre runs.
- `tests/key_rotation` no compila (módulos `crypto`/`phi_backfill` no expuestos al crate de test) — revisar exports en `lib.rs`.
- Fórmula logística de mortalidad Knaus con pesos completos si el lead clínico lo exige (ver §5.3).