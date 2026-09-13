# Pruebas de Carga k6 - Fase 5.8

## Ejecutar

```bash
# Iniciar servidor backend
cd dmart && cargo run --release --bin dmart-server

# En otra terminal, ejecutar tests
k6 run tests/load/auth.js        # Autenticación
k6 run tests/load/scales.js      # Escalas clínicas
k6 run tests/load/fhir.js        # FHIR R4 API
k6 run tests/load/hl7.js         # HL7 MLLP health
k6 run tests/load/run_all.js     # Todos secuenciales
```

## Configuración

- **Stages**: Ramp-up → Stress → Ramp-down
- **Thresholds**: p95 < 500ms, error rate < 5%
- **Escenarios**: auth, scales, fhir, hl7

## Tests incluidos

| Archivo | Endpoints | Usuarios máx |
|---------|-----------|-------------|
| auth.js | /auth/login, /patients, /patients/stats | 100 |
| scales.js | /scales/apache, /scales/sofa, /scales/news2, /scales/saps3, /scales/gcs | 100 |
| fhir.js | /fhir/Patient, /fhir/Patient/{id}, /fhir/Patient/{id}/DiagnosticReport, /fhir/Patient/{id}/DiagnosticReport/QR | 100 |
| hl7.js | /health (simula carga MLLP) | 50 |
| metrics.js | /obs/metrics (scrape Prometheus) | 1000 rps (target SPEC-005: p95 < 100ms) |

## Métricas clave

- `http_req_duration`: latencia p95
- `http_req_failed`: tasa de errores HTTP
- `*_failures`: tasa de errores de negocio
