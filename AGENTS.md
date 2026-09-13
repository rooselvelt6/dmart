# AGENTS.md — Convenciones del proyecto dMart UCI

## Testing (MUY IMPORTANTE: tiempo limitado del usuario)

**NUNCA** ejecutar `cargo test --workspace`, `cargo test` en la raíz ni
`cargo llvm-cov --workspace`. Eso compila `dmart-app` (WASM Leptos) y
`dmart-server/fuzz` (libFuzzer) y tarda horas.

Solo se usa `-p dmart-server` con targets acotados:

```bash
# Todos los tests relevantes del backend (rápido, ~15s)
cargo test -p dmart-server --test api_tests --test hl7_integration

# Spec-004 (auth/authz + refresh tokens + RBAC) — ~8s
cargo test -p dmart-server --test api_tests

# Tests unitarios del lib — rápido
cargo test -p dmart-server --lib

# Coverage acotada (solo server, sin workspace)
cargo llvm-cov -p dmart-server --lib --test api_tests
```

La compilación incremental caliente tarda <1s (bins de ~640MB ya construidos
en `target/debug`). Si hace falta compilar el workspace, `--no-run` primero
con timeout.

## Stack

- Rust 2024 (rust-toolchain.toml) workspace: dmart-shared / dmart-server / dmart-app (WASM Leptos) / dmart-server/fuzz
- SurrealDB local `SerdeKv` en tests (`test_db()` crea TempDir por test)
- Tests E2E HTTP in-process con `tower::ServiceExt::oneshot`

## Ramas / commits

- No commitear sin que el usuario lo pida explícitamente.
- Mensajes de commit estilo conventional (feat:/fix:/test:/docs:).