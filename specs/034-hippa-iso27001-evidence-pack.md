# SPEC-034: HIPAA/NIST 800-53 / ISO 27001 Evidence Pack — Auditoría lista

## Contexto

- **Problema a resolver**: dMart gestiona PHI (Protected Health Information). Para firmar un contrato hospitalario se requiere evidencia de cumplimiento: mapeo de controles a HIPAA (45 CFR 164), NIST 800-53 e ISO 27001, con evidencia auditable de cada control. Hoy la documentación estaría incompleta para una auditoría.
- **Usuario objetivo**: Administrador hospitalario / Compliance Officer / Auditor externo
- **Métrica de éxito (KPI)**: Evidence pack completo listo para auditoría en < 5 días de trabajo; 100% de controles HIPAA/ISO27001/IEC 62443 mapeados con evidencia; reporte auto-generado

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Evidence Pack de Cumplimiento
  As a compliance officer
  I want un paquete de evidencia completo de controles de seguridad
  So that una auditoría HIPAA/NIST/ISO 27001 se pueda preparar sin bloqueos

  Scenario: Control catalog mapeado
    Given el sistema dMart
    When un auditor abre `docs/compliance/CONTROL_CATALOG.md`
    Then existen las 118 familias de controles NIST 800-53 relevantes
    And cada control tiene: estado, evidencia appuntada, responsable, fecha
    And hay un subconjunto HIPAA (45 CFR 164.308/310/312) mapeado

  Scenario: Data flow inventory
    Given el sistema dMart
    When un auditor abre `docs/compliance/DATA_FLOWS.md`
    Then se documentan todos los flujos de datos clínicos (HL7, API, WS, backups)
    And cada flujo tiene: fuentes, destinaciones, formato, encriptación, retención
    And el inventario cubre storage, procesamiento, y transmisión de PHI

  Scenario: Evidence auto-generada
    Given dMart corriendo en staging/prod
    When un auditor ejecuta `./scripts/compliance_generate.sh`
    Then se genera `docs/compliance/evidence/{timestamp}/`
    And contiene: config versionado, audit logs (scores), backup verificación, gate del CI
    And un reporte JSON con checks por control

  Scenario: Risk assessment trimestral
    Given el proceso de compliance activo
    When pasa el trimestre
    Then se genera `docs/compliance/risk_assessment/{quarter}.md`
    And documenta: riesgos identificados, probabilidad, impacto, mitigación
    And los riesgos abiertos tienen deadline y dueño

  Scenario: Vendor / BAA documentation
    Given un integrador externo accede a PHI
    When se necesita documentación
    Then existe plantilla BAA (Business Associate Agreement)
    And la lista de subprocesadores está actualizada
    And cada subprocesador tiene su BAA firmado

  Scenario: Incident response evidence
    Given un incidente de seguridad registrado
    When el auditor revisa `docs/compliance/INCIDENTS.md`
    Then cada incidente tiene: severidad, timeline, impacto PHI, root cause, remediation
    And los incidentes se correlacionan con el runbook SPEC-011
```

## API Contracts

### Scripts de Compliance

```bash
# Generación de evidence pack
./scripts/compliance_generate.sh \
  --env staging \
  --output docs/compliance/evidence/$(date +%Y%m%d-%H%M)/ \
  --include audit_logs \
  --include backup_verification \
  --include ci_gates

# Verificación de controles
./scripts/compliance_check.sh \
  --controls NIST:AC-2,HIPAA:164.308(a)(3),ISO:A.9.2.1 \
  --report compliance_report_$(date +%Y%m%d).json

# Risk assessment
./scripts/risk_assessment.sh \
  --quarter 2026-Q3 \
  --output docs/compliance/risk_assessment/2026-Q3.md
```

### Reporte JSON de controles

```json
{
  "generated_at": "2026-09-16T00:00:00Z",
  "environment": "staging",
  "controls_total": 118,
  "controls_implemented": 98,
  "controls_in_progress": 15,
  "controls_deviated": 5,
  "controls": [
    {
      "id": "NIST:AC-2",
      "hipaa_mapping": "164.312(a)(1)",
      "iso_mapping": "A.9.2.1",
      "status": "implemented",
      "evidence": [
        "docs/compliance/evidence/20260916-0000/authz_implementation.md",
        "src/rbac.rs",
        "tests/api_tests.rs"
      ],
      "control_description": "Account Management",
      "implementation_notes": "RBAC granular SPEC-004; JWT refresh; tenant isolation SPEC-025",
      "last_reviewed": "2026-09-16"
    }
  ]
}
```

### Control Catalog Structure

```markdown
# docs/compliance/CONTROL_CATALOG.md

| Control ID | Familia | Título | Estándar (HIPAA/NIST/ISO) | Estado | Evidencia | Responsable |
|------------|---------|--------|--------------------------|--------|-----------|-------------|
| AC-1 | Access Control | Policy y procedimientos | 164.312(a)(1) / ISO A.9 | ✅ Implemented | `docs/compliance/evidence/...` | DevOps |
| AU-2 | Audit & Accountability | Event logging | 164.308(a)(1)(ii)(D) / A.12.4 | ✅ Implemented | audit logs en SurrealDB | Backend |
| RA-5 | Risk Assessment | Vulnerability scanning | 164.308(a)(1)(ii)(A) / A.12.6.1 | 🟡 In progress | `cargo audit` en CI | Security |
```

## Data Models

### Control States

```rust
enum ControlStatus {
    Implemented,      // evidencia lista y aprobada
    InProgress,       // parcial, deadline definido
    NotImplemented,   // gap documentado con plan
    Deviated,         // desviación aprobada con justificación
    NotApplicable,    // no aplica al sistema
}
```

### Descripción de controles clave

| Categoría | Controles clave | Espec/Código que lo cubre |
|-----------|----------------|---------------------------|
| Access Control | AC-1..AC-25, 164.312(a) | SPEC-004 (RBAC), SPEC-025 (tenancy) |
| Audit & Accountability | AU-1..AU-16, 164.308(a)(1)(ii)(D) | SPEC-029 (fingerprint), audit log |
| Contingency Planning | CP-1..CP-13, 164.308(a)(7) | SPEC-009/010/014 (backup/DR), SPEC-024 |
| System & Info Integrity | SI-1..SI-21 | SPEC-031 (ingest hardening), CI gates |
| Risk Assessment | RA-1..RA-7, 164.308(a)(1)(ii)(A) | este spec |
| Incident Response | IR-1..IR-10, 164.308(a)(6) | SPEC-011 (runbook), SPEC-008/019 (alertas) |
| Physical & Environmental | PE-1..PE-20, 164.310 | infraestructura hospital (fuera de alcance soft) |
| Cryptography | SC-8, SC-12, SC-13, 164.312(e) | TLS en HTTP/MLLP, encryption at rest |

### Aplicabilidad ISO 27001:2022

| Anexo A | Título | NIST map | Estado |
|---------|--------|----------|--------|
| A.5 | Políticas de seguridad | AC-1, AT-1 | Implemented |
| A.6 | Organización | PS-1 | Implemented |
| A.8 | Gestión de activos | CM-8 | Implemented |
| A.9 | Control de acceso | AC-2..AC-8 | Implemented |
| A.10 | Criptografía | SC-8, SC-13 | Implemented |
| A.12 | Seguridad operacional | AU-6, SI-4 | Implemented |
| A.16 | Gestión de incidentes | IR-1..IR-10 | Implemented |

## Edge Cases

| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Auditor pide evidencia de control sin doc | `compliance_check.sh` reporta "no evidence" y genera gap |
| 2 | Evidencia expira (config cambió) | Checksum de evidence comprobado; marca "stale" |
| 3 | Control aplica a infraestructura, no a app | Estado "NotApplicable" con justificación |
| 4 | HIPAA vs ISO conflicto en control | Se documenta el mapeo más estricto |
| 5 | Incidente sin resolverse | IRS-XX abierto; se reporta en risk assessment |
| 6 | BAA vencido | Alerta y lista de vencimientos en weekly report |
| 7 | NIST/ISO version updates | Catalog versionado + mapping revisado en CRA trimestral |

## Security Considerations

### Threat Model (STRIDE)

| Threat | Mitigación |
|--------|------------|
| Spoofing | Evidence firmado; logs inmutables (append-only) |
| Tampering | Evidence checksums; git history en compliance/ |
| Repudiation | Audit trail de cambios a controles (quién/cuándo) |
| Information Disclosure | Evidence con PHI redactado; solo managers pueden acceder |
| Denial of Service | Backups redundantes de docs/compliance/ |
| Elevation of Privilege | Solo compliance reviewers editan catalog |

### Data Classification
- [x] PHI (referenciado en fabric flows)
- [x] Clinical Data
- [x] Operational/Metadata (control status, evidence)

### Cumplimiento Legal (Cuba)
- Ley 81 del Medio Ambiente no aplica; art. 127 de Ley 2021 (TELECOM) / Ley 118 (protección de datos) revisado en `docs/compliance/LEGAL.md`
- Regulacion futuro de Salud Digital cubana: track en risk assessment como "regulatory watch"

## Testing Strategy

### Unit Tests
- [ ] `CONTROL_CATALOG.md` tiene todos los controles con campos obligatorios
- [ ] Mapeo HIPAA/NIST/ISO validado contra listas de referencia
- [ ] `compliance_generate.sh` produce estructura correcta

### Integration Tests
- [ ] Evidencia auto-generada en staging
- [ ] Reporte JSON de controles parseable y válido
- [ ] Risk assessment quarterly generado

### Audits simulados
- [ ] "Auditoría simulada" trimestral con checklist de 100 preguntas
- [ ] Gap analysis: 0 controles críticos sin dueño

## Rollout Plan

### Feature Flag
```yaml
compliance:
  enabled: true
  evidence_dir: docs/compliance/evidence/
  auto_generate: "weekly"
  risk_assessment_quarterly: true
```

### Rollout
| Paso | Alcance | Duración | Criterio go/no-go |
|------|---------|----------|-------------------|
| 1 | Catalog + data flows + evidence scripts | 1 semana | 100% controles mapeados |
| 2 | HIPAA 164.308/310/312 mapeado | 2 días | Auditors recorren sin gaps |
| 3 | ISO 27001 Anexo A mapeado | 2 días | Mapping documentado |
| 4 | BAA + subprocesadores + incidentes | 1 día | Plantillas completas |
| 5 | Auditoría simulada | trimestral | 0 hallazgos mayor |

### Rollback Procedure
- docs/compliance es versionado en git con el resto del repo; revert = git revert

## Definition of Done

- [ ] `specs/034-hippa-iso27001-evidence-pack.md` (este archivo)
- [ ] `docs/compliance/CONTROL_CATALOG.md`
- [ ] `docs/compliance/DATA_FLOWS.md`
- [ ] `docs/compliance/LEGAL.md` (leyes cubanas aplicables)
- [ ] `scripts/compliance_generate.sh`
- [ ] `scripts/compliance_check.sh`
- [ ] `scripts/risk_assessment.sh`
- [ ] Plantilla BAA + lista de subprocesadores
- [ ] Incidentes correlacionados con runbook SPEC-011
- [ ] Tests: catalog validation + scripts
- [ ] ROADMAP, CHANGELOG actualizados

## Gate CI SPEC-034
```bash
./scripts/compliance_check.sh --controls NIST:AC-2,ISO:A.9.2.1 --report /tmp/comp.json
cat /tmp/comp.json | jq '.controls_implemented >= 98'   # = true
```

## Métricas
- Controles mapeados: 100% (118 NIST relevantes + HIPAA + ISO)
- Controles implementados: ≥ 98 (83%)
- Tiempo de preparación de auditoría: < 1 h (auto-generado)
- Hallazgos críticos en auditoría simulada: 0