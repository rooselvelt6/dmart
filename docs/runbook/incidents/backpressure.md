# Incidente Backpressure ingest SPEC-031

## Síntoma
`ingest_error_avg_set` sube; MLLP responde ACK con backpressure (SPEC-031 README).

## Comando
```bash
curl -s localhost:3030/obs/health
curl -s localhost:3030/monitores/health
grep -rn 'backpressure' dmart-server/src/server_ingest.rs
```

## Verificar
- Cola ingest con headroom; `ingest_state` no en flag `full`.
- MLLP re-drena el lote retenido tras ACK NAK (SPEC-010 paso 6).

## Escalar
- Persiste > 5 min → SPEC-010 DR con lote retenido + SPEC-031 gate (0 dead-code).
