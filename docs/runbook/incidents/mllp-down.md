# Incidente MLLP/Hl7 caído (SPEC-008/011)

## Síntoma
`/obs/health` degradado o alerta `hl7_integration` rojos; contador MLLP sin conexiones.

## Comando
```bash
curl -s localhost:3030/obs/health
ps aux | grep -E 'dmart-server|MLLP'
cargo clippy -p dmart-server --bin dmart-server -- -D warnings   # SI compila, reinicide
```

## Verificar
- Puerto MLLP escuchando: `ss -ltnp | grep 2575` (SPEC-031).
- Backpressure: la cola ingest no debe notificar `ingest_error_avg_set` > 0.
- 63 tests (hl7_integration 32 + api 31) verdes.

## Escalar
- Si `lsnr` no levanta tras 2 reinicios → on-call senior + SPEC-010 (restore DR).
- MTTR objetivo < 15 min.
