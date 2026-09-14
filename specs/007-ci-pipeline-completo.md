# SPEC-007: CI Pipeline Completo

## Contexto

- **Problema a resolver**: El pipeline CI/CD debe ejecutar todos los gates de calidad (fmt, clippy, tests, fuzz, load, wasm, e2e, security, docker) en cada PR y push a main, garantizando que solo código verde llegue a staging/producción.
- **Usuario objetivo**: Equipo de desarrollo (backend, frontend, DevOps), CI/CD automation
- **Métrica de éxito (KPI)**: CI time < 15 min (paralelismo jobs), 0 advisories críticos, 100% tests passing, WASM < 1MB, Docker image ~50MB

## Acceptance Criteria (Gherkin)

```gherkin
Feature: CI Pipeline Completo
  As a desarrollador
  I want un pipeline CI que ejecute todos los gates en paralelo
  So that detectemos regresiones antes de mergear a main

  Scenario: PR abierto contra main ejecuta todos los jobs
    Given un PR con cambios en dmart-server, dmart-shared, dmart-app
    When se abre el PR
    Then spec-lint valida formato de specs
    And rust-fmt verifica formato
    And rust-clippy compila sin warnings
    And rust-test-lib ejecuta tests de librería
    And rust-test-integration ejecuta tests API
    And hl7-integration-test ejecuta tests HL7 MLLP
    And prop-test ejecuta proptest
    And fuzz-test ejecuta fuzzing (30s por target)
    And wasm-build compila frontend y verifica tamaño < 1MB
    And security-audit ejecuta cargo audit
    And todos los jobs pasan en < 15 min

  Scenario: Push a main ejecuta pipeline completo + release
    Given push a rama main
    When pipeline se dispara
    Then todos los jobs anteriores corren
    And release-build compila binarios release
    And release-build sube artifact dmart-server
    And notify resume resultados

  Scenario: Load test opcional (label o push)
    Given PR con label "load-test" O push a main
    When pipeline corre
    Then load-test levanta backend y ejecuta k6 (auth, scales, fhir, hl7)
    And k6 results se suben como artifact

  Scenario: Docker build verifica imagen multi-stage
    Given PR con cambios en Dockerfile o docker-compose
    When pipeline corre
    Then docker-build construye imagen
    And imagen < 60MB
    And healthcheck pasa en contenedor
```

## API Contracts

No aplica (pipeline/infraestructura).

## Data Models

No aplica.

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Cache cargo corrompido | Job falla rápido, re-run limpia cache |
| 2 | WASM build falla por lightningcss | Usar cargo + wasm-bindgen (bypass trunk) |
| 3 | k6 no encuentra backend | Espera healthcheck, reintenta 3x |
| 4 | cargo audit advisory crítico | Job falla, bloquea merge |
| 5 | Fuzzing encuentra crash | Job falla, reporta input minimal |
| 6 | E2E test flaky | Re-run automático 1x, luego falla |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | GitHub Actions OIDC, secrets en env protegidos |
| Tampering | Artifacts firmados, SBOM futuro |
| Repudiation | Audit log de GitHub Actions inmutable |
| Information Disclosure | Secrets solo en env, no en logs |
| Denial of Service | Timeouts en todos los jobs, resource limits |
| Elevation of Privilege | Permisos mínimos por job, no-write en repo |

### Data Classification
- [ ] PHI
- [ ] PII
- [ ] Clinical Data
- [x] Operational/Metadata (CI logs, artifacts)

### Auth/Autz Requirements
- Roles permitidos: CI bot (GitHub Actions)
- Permisos: `contents: read`, `actions: read`, `security-events: write`

## Testing Strategy

### Unit Tests
- [ ] N/A (pipeline config)

### Property-Based Tests
- [ ] N/A

### Fuzzing Targets
- [ ] N/A

### Integration Tests
- [ ] Pipeline end-to-end en PR de prueba

### Load Test (k6)
- [ ] Scenario: 100 VUs, 2 min, p95 < 500ms (ya en tests/load/)

## Rollout Plan

### Feature Flag
No aplica (infra).

### Canary Deployment

| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | PR interno (feature branch) | 1 run | Todos jobs verde |
| 2 | PR contra main (staging) | 1 run | + load-test label |
| 3 | Push a main (prod) | Siempre | Release build + artifact |

### Rollback Procedure
1. Revert PR que rompió CI
2. Re-run pipeline en main
3. Verificar notify summary verde

## Definition of Done

- [ ] Spec aprobada en PR (1+ reviewer)
- [ ] `.github/workflows/ci.yml` incluye todos los jobs listados
- [ ] `docker-build` job añadido (construye + verifica tamaño + healthcheck)
- [ ] `wasm-opt` job añadido (SPEC-011, opcional en esta spec)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` pasa
- [ ] `cargo fmt --all -- --check` pasa
- [ ] `cargo test --workspace --lib --test api_tests --test hl7_integration` pasa
- [ ] `cargo test --workspace --lib proptest` pasa
- [ ] `cargo build --release --workspace` exitoso
- [ ] WASM build < 1MB verificado en CI
- [ ] Docker build exitoso, imagen < 60MB
- [ ] `cargo audit --workspace --deny warnings` pasa (0 critical)
- [ ] k6 load test ejecutable con label
- [ ] Playwright E2E tests pasan contra build real
- [ ] Documentación actualizada (ROADMAP, CHANGELOG)
- [ ] Deploy staging verificado con `docker compose -f docker-compose.prod.yml up`