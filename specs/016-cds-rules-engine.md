# SPEC-016: Clinical Decision Support Rules Engine — FHIR PlanDefinition + Reglas

## Contexto
La UCI genera alertas (SPEC-008/011) y scores (SPEC-014). Falta **motor de reglas clínicas** que evalúe condiciones y proponga acciones basadas en evidencia (ej: sepsis bundle, ARDS protocol, ventilación protectora, anticoagulación). Debe ser **declarativo** (FHIR `PlanDefinition`), versionado y auditable (SPEC-029).

## Objetivo
Motor de reglas que:
- Carga `PlanDefinition` FHIR R4 desde DB (versionadas, activas)
- Evalúa condiciones sobre `PatientEvent` (timeline SPEC-015) + scores actuales (SPEC-014)
- Dispara `ActivityDefinition` (acción: alerta, orden, notificación, protocol)
- Expone `POST /api/cds/evaluate` (input: patient_id, context) → `CarePlan` FHIR con acciones
- Integra con alertas SPEC-008/011 y EWS SPEC-014

## Alcance
- Módulo `cds_rules` en lib (allow SPEC-031 documentado)
- Tabla `plan_definition` (FHIR PlanDefinition JSONB + versión + activa)
- Evaluador de expresiones **CEL** (Common Expression Language) para condiciones
- Tipos de acción: `Alert`, `Order`, `Notification`, `Protocol`, `Referral`
- Endpoint `POST /api/cds/evaluate` + `GET /api/cds/plans` (listar activas)
- Seed: 3 planes base (Sepsis-3 bundle, ARDSnet ventilation, Anticoagulation ICU)

## Fuera de alcance
- Autoría visual de reglas (backlog SPEC-024)
- ML-based rule suggestion (SPEC-032)

## Definition of Done
- [ ] `specs/016-cds-rules-engine.md` (este archivo)
- [ ] `pub mod cds_rules;` en `lib.rs` con allow SPEC-031
- [ ] Migración `DMART_016_cds_plans.surql` (tabla PlanDefinition + índice)
- [ ] Modelo `PlanDefinition`, `ActivityDefinition`, `CarePlan` (FHIR subset)
- [ ] Evaluador CEL (`cel-crate`) sobre `PatientEvent` + scores + patient data
- [ ] Handler `POST /api/cds/evaluate` + `GET /api/cds/plans`
- [ ] Seed SQL con 3 planes base (Sepsis, ARDS, Anticoag)
- [ ] Test `dmart-server/tests/cds_rules.rs` (≥ 3 planes, evaluación, acciones)
- [ ] `cargo fmt` + `cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings` = 0/0
- [ ] Cobertura SPEC-027 no decrece

## Gate CI SPEC-016
```bash
cargo test -p dmart-server --test cds_rules --test patient_timeline 2>&1 | grep "test result: ok"
cargo clippy -p dmart-server --lib --bin dmart-server -- -D warnings 2>&1 | grep '^error' | wc -l  # = 0
```

## Métricas
- Latencia evaluación < 10 ms (10 reglas)
- 100 % planes con fingerprint SPEC-029
- Acciones trazadas a `patient_event` tipo `Alert`/`Order`