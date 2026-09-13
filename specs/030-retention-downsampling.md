# SPEC-030: Retención y Downsampling de Mediciones (Crecimiento Controlado)

## Contexto
- **Problema**: `measurement` crece sin límite (100 camas × varias mediciones/min). Sin downsampling ni retención, la tabla de series temporales degrada consultas, backups y disco. Actualmente no hay política: pérdida de datos progresiva o costo creciente.
- **Usuario objetivo**: Ops / Backend
- **Métrica de éxito (KPI)**: crecimiento estable documentado (< preset defy por mes), consultas `/api/stats` p95 < 500ms con 100k pacientes, restore probado con política vigente.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Retención y downsampling de mediciones
  As a hospital IT
  I want que los datos se conserven indefinidamente pero a granularidad decreciente
  So that la DB nunca crezca sin control ni pierda significado clínico

  Scenario: Política de retención por capas
    Given mediciones con timestamp
    When pasa el job diario de retención
    Then se agregan shows > 365 días a measurement_hourly (AVG/MIN/MAX por hora)
    And se agregan shows > 1825 días a measurement_daily
    And se borran los buckets raw ya agregados y mayores a 1825 días
    And se registra en la tabla retention_job (counts, duración, estado)

  Scenario: Capa deshabilitada
    Given DMART_RETENTION_MODE=disabled en config
    When corre el job
    Then no borra ni agrega nada (log info)

  Scenario: Consulta de stats con datos agregados
    Given measurement_hourly poblada
    When GET /api/stats?granularity=hourly&range=2years
    Then responde ~ 24x más barato que raw (mismo O(índice))
    And nunca materializa la serie completa en memoria
```

## Capas de Datos

| Capa | Granularidad | Retención | Tabla | Tamaño estimado / 100 camas |
|------|--------------|-----------|-------|------------------------------|
| Raw | 1 min (picos 1 s) | 365 días | `measurement` | ~220 GB/año crudo → objetivo bajo 60 GB |
| Hourly | 1 h | 5 años | `measurement_hourly` | ~2.6 GB/año |
| Daily | 1 día | ∞ | `measurement_daily` | ~37 MB/año |

Ajuste ~12× (raw↔hourly): 60 tr/min → 5 tr/h por cama.

Algoritmo de agregación por bucket (SQL funcional, no window sobre toda la serie):
```
AVG(value), MIN(value), MAX(value), COUNT(*) AS points, COUNT_IF(invalid) AS invalid
```

## API Contracts

| Método | Path | Cambio |
|--------|------|--------|
| GET | `/api/stats` | mod: `granularity=raw/hourly/daily` param; default `hourly` para rangos >90d |
| POST | `/api/admin/retention/run` | nuevo (admin): dispara job bajo demanda |
| GET | `/api/admin/retention/status` | nuevo (admin): última ejecución, counts, próximo run |

## Data Models / Migraciones
```sql
-- migrations/XXX_retention_hourly.surql
DEFINE TABLE measurement_hourly SCHEMAFULL;
DEFINE FIELD bucket ON measurement_hourly TYPE datetime;      -- start of hour
DEFINE FIELD patient_id ON measurement_hourly TYPE record<Patient>;
DEFINE FIELD apache_ii_min/apache_ii_max/apache_ii_avg TYPE float;
DEFINE FIELD gcs_min/gcs_max/gcs_avg TYPE float;
DEFINE FIELD points ON measurement_hourly TYPE int;
DEFINE FIELD invalid ON measurement_hourly TYPE int DEFAULT 0;
DEFINE INDEX idx_hourly_patient_bucket ON measurement_hourly COLUMNS patient_id, bucket;

-- measurement_daily, análogo con DAY
DEFINE TABLE retention_job SCHEMAFULL;
DEFINE FIELD started_at / finished_at / status / aggregated_raw / deleted_raw TYPE ...;
```
- `DEFINE EVENT on_measurement_insert` para contar puntos/PII no requerido (mejor job por lotes).

## Edge Cases
| # | Caso | Comportamiento |
|---|------|----------------|
| 1 | Bucket parcial (última hora no cerrada) | se agrega solo cuando el bucket está completo (grace 1 h) |
| 2 | Job corre en horario pico | backoff: ejecutar solo si `no_tx_conflicts`; batch de 10k registros |
| 3 | Doble corrida / crash en medio | `retention_job.status=running` con idempotencia: agregar solo buckets no cubiertos |
| 4 | Retention deshabilitada a mitad | se pausa sin datos corruptos (job transaccional por bucket) |
| 5 | Consulta crossover raw+hourly | stats resuelve por suffi bucket; nunca mezcla granos en un punto |
| 6 | Disco crítico | alerta (SPEC-005) si free space < 10% → job fuerza `hourly` antes del raw purge |

## Security Considerations
- Acceso admin-only para disparar job; `measurement` PHI: el job borra raw pero los agregados **no incluyen** valores identificables (solo stats).
- Auditoría: `retention_job` queda como trail de lo borrado (quora clínica).

## Testing Strategy
- [ ] Unit: `aggregate_hourly_bucket()` (AVG/MIN/MAX, invalid count, bucket strict)
- [ ] Unit: idempotencia (correr 2× → mismo resultado, sin dobles)
- [ ] Integration: job completo contra SurrealKV embedded con 10k mediciones
- [ ] Load (k6): `/api/stats?granularity=hourly` p95 < 500ms
- [ ] Restore test: backup post-política se restaura y `retention_job` se reconstruye

## Rollout Plan
- Phase A (flag `DMART_RETENTION_MODE=disabled` default) → Phase B (`enabled`, umbral 365/1825 días) tras carga en staging.
- Cron: tarea diaria en `dmart-server` (`tokio::time`) + endpoint manual.

## Definition of Done
- [ ] Spec aprobada
- [ ] Tablas `measurement_hourly`, `measurement_daily`, `retention_job` + migraciones
- [ ] Job diario implementado (batch, idempotente, transaccional)
- [ ] `/api/stats` con granularidad + endpoints admin
- [ ] Tests + load verdes
- [ ] Documentación (README ops, CHANGELOG, ROADMAP) actualizada