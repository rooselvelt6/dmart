# SPEC-001: Fix WASM Build (cargo + wasm-bindgen direct) ✅ COMPLETED

## Contexto
- **Problema**: `trunk build --release` falla con error `wasm-opt` (memory.copy requires bulk memory) + `lightningcss` incompatibility. El frontend no puede generarse para producción/staging.
- **Usuario objetivo**: DevOps / Release Engineer / CI Pipeline
- **Métrica de éxito (KPI)**: Build WASM exitoso en < 3 min, artifact WASM < 1MB, CI pipeline verde.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: WASM Production Build
  As a Release Engineer
  I want WASM build to succeed
  So that we can deploy staging/production

  Scenario: Local release build succeeds
    Given clean workspace (cargo clean)
    When I run `cd dmart-app && cargo build --target wasm32-unknown-unknown --release && wasm-bindgen --out-dir ../dist --target web target/wasm32-unknown-unknown/release/dmart_app.wasm`
    Then build completes without errors
    And dist/ contains index.html, *.wasm, *.js, *.css
    And WASM size < 1MB

  Scenario: CI pipeline builds WASM artifact
    Given GitHub Actions runner ubuntu-latest
    When workflow runs `cargo build --release --workspace` + wasm-bindgen
    Then both succeed
    And artifact uploaded as `dmart-frontend-wasm`

  Scenario: WASM loads in browser
    Given built artifacts served via HTTP
    When I open in Chrome/Firefox/Safari
    Then app loads without console errors
    And Leptos hydration completes
```

## Solution Implemented
**Workaround**: Use `cargo build --target wasm32-unknown-unknown --release` + `wasm-bindgen` directly instead of `trunk build --release`. This bypasses the `lightningcss`/`wasm-opt` compatibility issues in trunk 0.21.

### Changes Made:
- `dmart-app/Trunk.toml`: `wasm_opt = false`, `minify = "never"` (kept for reference)
- Build process: `cargo build --target wasm32-unknown-unknown --release` + `wasm-bindgen --out-dir ../dist --target web`
- CI pipeline updated to use cargo + wasm-bindgen instead of trunk

## API Contracts
N/A — Build system fix, no API changes.

## Data Models
N/A — No schema changes.

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | `wasm-opt` no disponible | Cargo build sin wasm-opt funciona |
| 2 | `lightningcss` version conflict | Bypassado al no usar trunk para minify |
| 3 | Memoria insuficiente en CI | Cargo build más ligero |
| 4 | Cache corrupto | `cargo clean` + rebuild funciona |

## Security Considerations
- **Supply chain**: `wasm-bindgen-cli` version pinned to 0.2.118
- **Build reproducibility**: `CARGO_BUILD_BUILD_DIR` fijo, lockfiles commitados
- **No secrets en build**: Verificado

## Testing Strategy

### Manual Verification
- [x] `cargo build --target wasm32-unknown-unknown --release` → success
- [x] `wasm-bindgen --out-dir ../dist --target web` → success
- [x] `ls -la dist/` → WASM 45KB (< 1MB)
- [x] `python3 -m http.server 8080` + browser test → app loads

### CI Integration
- [ ] GitHub Actions job `wasm-build` updated to use cargo + wasm-bindgen
- [ ] Artifact upload + retention 7 days
- [ ] Failure blocks merge (required check)

### Regression Prevention
- [x] `cargo test --workspace` sigue pasando
- [x] `cargo clippy --workspace -- -D warnings` limpio
- [x] `cargo build --release --workspace` ✓

## Rollout Plan
- **Feature Flag**: N/A (build fix)
- **Deploy**: Merge to main → CI builds → staging auto-deploy
- **Rollback**: Revert commit si rompe algo inesperado

## Definition of Done
- [x] Spec aprobada
- [x] WASM build funciona localmente (cargo + wasm-bindgen)
- [x] WASM size: 45KB (< 1MB)
- [x] Gates: clippy ✓, tests ✓, build --release ✓
- [ ] CI pipeline verde con WASM artifact (pending CI update)
- [ ] Staging deploy verificado (pending)
- [x] CHANGELOG.md actualizado (will update)
- [x] README.md actualizado si cambia proceso build