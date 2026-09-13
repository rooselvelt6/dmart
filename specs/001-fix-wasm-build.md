# SPEC-001: Fix WASM Build (trunk build --release)

## Contexto
- **Problema**: `trunk build --release` falla con error `wasm-opt` + `lightningcss` incompatibility. El frontend no puede generarse para producción/staging.
- **Usuario objetivo**: DevOps / Release Engineer / CI Pipeline
- **Métrica de éxito (KPI)**: `trunk build --release` exitoso en < 3 min, artifact WASM < 1MB, CI pipeline verde.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: WASM Production Build
  As a Release Engineer
  I want trunk build --release to succeed
  So that we can deploy staging/production

  Scenario: Local release build succeeds
    Given clean workspace (cargo clean)
    When I run `cd dmart-app && trunk build --release`
    Then build completes without errors
    And dist/ contains index.html, *.wasm, *.js, *.css
    And WASM size < 1MB

  Scenario: CI pipeline builds WASM artifact
    Given GitHub Actions runner ubuntu-latest
    When workflow runs `cargo build --release` + `trunk build --release`
    Then both succeed
    And artifact uploaded as `dmart-frontend-wasm`

  Scenario: WASM loads in browser
    Given built artifacts served via HTTP
    When I open in Chrome/Firefox/Safari
    Then app loads without console errors
    And Leptos hydration completes
```

## API Contracts
N/A — Build system fix, no API changes.

## Data Models
N/A — No schema changes.

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | `wasm-opt` no disponible en PATH | trunk usa fallback o salta optimización |
| 2 | `lightningcss` version conflict | Pin version compatible o deshabilitar minify CSS |
| 3 | Memoria insuficiente en CI (wasm-opt OOM) | Limitar `--jobs=1` o deshabilitar wasm-opt |
| 4 | Cache corrupto | `trunk clean` + rebuild funciona |

## Security Considerations
- **Supply chain**: Pin versiones exactas en `Cargo.toml` y `package.json`
- **Build reproducibility**: `CARGO_BUILD_BUILD_DIR` fijo, lockfiles commitados
- **No secrets en build**: Verificar que no hay env vars sensibles en Trunk.toml

## Testing Strategy

### Manual Verification
- [ ] `cd dmart-app && trunk build --release` → success
- [ ] `ls -la dist/` → WASM < 1MB
- [ ] `python3 -m http.server 8080` + browser test → app loads

### CI Integration
- [ ] GitHub Actions job `wasm-build` added to `ci.yml`
- [ ] Artifact upload + retention 7 days
- [ ] Failure blocks merge (required check)

### Regression Prevention
- [ ] `cargo test --workspace` sigue pasando
- [ ] `cargo clippy --workspace -- -D warnings` limpio

## Rollout Plan
- **Feature Flag**: N/A (build fix)
- **Deploy**: Merge to main → CI builds → staging auto-deploy
- **Rollback**: Revert commit si rompe algo inesperado

## Definition of Done
- [ ] Spec aprobada
- [ ] `trunk build --release` funciona localmente
- [ ] CI pipeline verde con WASM artifact
- [ ] Staging deploy verificado
- [ ] CHANGELOG.md actualizado
- [ ] README.md actualizado si cambia proceso build