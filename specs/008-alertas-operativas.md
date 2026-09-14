# SPEC-008: Alertas Operativas (webhook/email)

## Contexto

- **Problema a resolver**: El sistema expone métricas Prometheus (`/metrics`) y dashboards Grafana (SPEC-005), pero no hay alertas automáticas que notifiquen al equipo cuando algo falla (CPU alta, memoria, disco, DB caída, ingest HL7 degradado).
- **Usuario objetivo**: Equipo de operaciones / DevOps / On-call
- **Métrica de éxito (KPI)**: 0 falsas alarmas/semana tras tuning; notificación < 1 min desde trigger; MTTR < 15 min

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Alertas Operativas
  As a operador de guardia
  I want recibir notificaciones automáticas ante fallos
  So that pueda reaccionar antes de que el clínico lo note

  Scenario: CPU alta sostenida
    Given el servidor corre en staging
    When CPU > 80% durante 5 min
    Then AlertManager dispara alerta "HighCPU"
    And notificación llega a webhook/email configurado

  Scenario: Memoria alta (RSS)
    Given el servidor corre
    When RSS > 90% del límite del contenedor (1GB) durante 5 min
    Then AlertManager dispara alerta "HighMemory"
    And notificación incluye pod/container name

  Scenario: Disco lleno
    Given el servidor corre
    When espacio libre en /app/data < 10% (o < 2GB)
    Then AlertManager dispara alerta "DiskSpaceLow"

  Scenario: Base de datos no responde
    Given el servidor corre
    When healthcheck /obs/health falla 3 veces seguidas (90s)
    Then AlertManager dispara alerta "DatabaseDown"
    And severity = critical

  Scenario: Cache (Valkey) desconectado
    Given el servidor corre
    When cache_connected == 0 durante 2 min
    Then AlertManager dispara alerta "CacheDisconnected"

  Scenario: Ingest HL7 degradado (SPEC-031)
    Given ingest HL7 activo
    When ingest_error_avg > 0.1 (10% errores) durante 5 min
    Then AlertManager dispara alerta "HL7IngestDegraded"

  Scenario: Falsa alarma controlada
    Given alertas configuradas con for: 5m
    When métrica spike momentáneo < 1 min
    Then NO se dispara alerta (for: evita flapping)

  Scenario: Silenciar alerta en mantenimiento
    Given ventana de mantenimiento programada
    When se activa silence en AlertManager
    Then no llegan notificaciones durante la ventana
```

## API Contracts

No nuevos endpoints. Configuración vía:
- `prometheus/rules/alerts-operational.yml` (reglas Prometheus)
- `alertmanager/config.yml` (ruting, receivers, inhibits)
- Variables de entorno en `docker-compose.prod.yml`:
  - `ALERT_WEBHOOK_URL` (opcional)
  - `ALERT_EMAIL_SMTP_HOST`, `ALERT_EMAIL_FROM`, `ALERT_EMAIL_TO`

## Data Models

### Nuevas reglas Prometheus (alerts-operational.yml)
```yaml
groups:
- name: operational
  interval: 30s
  rules:
  - alert: HighCPU
    expr: rate(process_cpu_seconds_total[5m]) * 100 > 80
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High CPU usage on {{ $labels.instance }}"
      description: "CPU > 80% for 5m"

  - alert: HighMemory
    expr: (process_resident_memory_bytes / 1073741824) > 0.9
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High memory usage on {{ $labels.instance }}"
      description: "RSS > 90% of 1GB limit"

  - alert: DiskSpaceLow
    expr: (node_filesystem_avail_bytes{mountpoint="/app/data"} / node_filesystem_size_bytes{mountpoint="/app/data"}) < 0.1
    for: 2m
    labels:
      severity: critical
    annotations:
      summary: "Low disk space on {{ $labels.instance }}"
      description: "Free space < 10%"

  - alert: DatabaseDown
    expr: up{job="dmart-server"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "dmart-server down on {{ $labels.instance }}"
      description: "Healthcheck failing"

  - alert: CacheDisconnected
    expr: cache_connected == 0
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "Valkey cache disconnected on {{ $labels.instance }}"

  - alert: HL7IngestDegraded
    expr: ingest_error_avg > 0.1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "HL7 ingest error rate > 10% on {{ $labels.instance }}"
```

### AlertManager config (config.yml)
```yaml
global:
  resolve_timeout: 5m

route:
  group_by: ['alertname', 'instance']
  group_wait: 30s
  group_interval: 5m
  repeat_interval: 4h
  receiver: 'default'
  routes:
  - match:
      severity: critical
    receiver: 'critical'
    continue: true

receivers:
- name: 'default'
  webhook_configs:
  - url: '{{ .Env.ALERT_WEBHOOK_URL }}'
    send_resolved: true
  email_configs:
  - to: '{{ .Env.ALERT_EMAIL_TO }}'
    from: '{{ .Env.ALERT_EMAIL_FROM }}'
    smarthost: '{{ .Env.ALERT_EMAIL_SMTP_HOST }}'
    send_resolved: true

- name: 'critical'
  webhook_configs:
  - url: '{{ .Env.ALERT_WEBHOOK_URL }}'
    send_resolved: true
  email_configs:
  - to: '{{ .Env.ALERT_EMAIL_TO }}'
    from: '{{ .Env.ALERT_EMAIL_FROM }}'
    smarthost: '{{ .Env.ALERT_EMAIL_SMTP_HOST }}'
    send_resolved: true

inhibit_rules:
- source_match:
    severity: 'critical'
  target_match:
    severity: 'warning'
  equal: ['instance']
```

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | AlertManager no disponible | Prometheus buffer local, reintenta |
| 2 | Webhook/email falla | Reintentos exponenciales (AlertManager default) |
| 3 | Múltiples instancias (HA futuro) | group_by instance evita duplicados |
| 4 | Métrica no existe (ej. node_exporter) | Regla evalúa a "no data" → no alerta |
| 5 | Flapping (métrica cruza umbral) | `for: 5m` absorbe spikes < 5 min |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | AlertManager valida webhook signature (opcional) |
| Tampering | Config en volume read-only, secrets en env |
| Repudiation | Alertas inmutables en AlertManager log |
| Information Disclosure | No PHI en labels/annotations; solo metadata operacional |
| Denial of Service | Rate limit en AlertManager receivers |
| Elevation of Privilege | AlertManager solo lectura en Prometheus |

### Data Classification
- [ ] PHI
- [ ] PII
- [ ] Clinical Data
- [x] Operational/Metadata (metrics, instance, severity)

### Auth/Autz Requirements
- AlertManager UI: basic auth o restringido a red interna
- Webhook: HMAC signature verification (futuro)

## Testing Strategy

### Unit Tests
- [ ] `test_alert_rule_syntax` (promtool test rules)
- [ ] `test_alertmanager_config_valid` (amtool check-config)

### Integration Tests
- [ ] Prometheus + AlertManager up en docker-compose
- [ ] Disparar alerta sintética → verificar webhook/email recibido

### Load Test
- [ ] 100 alertas simultáneas → todas entregadas < 30s

## Rollout Plan

### Feature Flag
```bash
# En .env.prod
ENABLE_ALERTS=true  # default false en staging
```

### Canary Deployment
| Paso | Tráfico | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Solo staging (alertas a log) | 24h | 0 falsas alarmas |
| 2 | Staging + webhook test | 48h | Webhook recibe payload correcto |
| 3 | Prod con email real | — | On-call recibe alertas |

### Rollback Procedure
1. `ENABLE_ALERTS=false` en .env.prod → `docker compose up -d`
2. Verificar AlertManager detenido
3. Métricas Prometheus siguen funcionando

## Definition of Done

- [ ] Spec aprobada en PR (1+ reviewer)
- [ ] `prometheus/rules/alerts-operational.yml` creado + `promtool test rules` pasa
- [ ] `alertmanager/config.yml` creado + `amtool check-config` pasa
- [ ] `docker-compose.prod.yml` incluye servicio `alertmanager` (opcional, detrás de flag)
- [ ] Variables de entorno documentadas en `.env.prod.example`
- [ ] Tests: `promtool test rules` + `amtool check-config` en CI
- [ ] Documentación actualizada (ROADMAP, CHANGELOG)
- [ ] Deploy staging verificado con alerta de prueba