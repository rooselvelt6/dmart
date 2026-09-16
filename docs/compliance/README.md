# Compliance Evidence Pack — dMart UCI

Paquete de evidencia de cumplimiento para auditorías HIPAA/NIST 800-53/ISO 27001 (SPEC-034).

## Documentos

| Documento | Descripción |
|-----------|-------------|
| [CONTROL_CATALOG.md](./CONTROL_CATALOG.md) | Catálogo de controles de seguridad mapeados |
| [DATA_FLOWS.md](./DATA_FLOWS.md) | Inventario de flujos de datos clínicos |
| [LEGAL.md](./LEGAL.md) | Marco legal cubano aplicable |
| [INCIDENTS.md](./INCIDENTS.md) | Registro y plantilla de incidentes |

## Scripts

| Script | Descripción |
|--------|-------------|
| `scripts/compliance_generate.sh` | Genera evidence pack auto-generado |
| `scripts/compliance_check.sh` | Valida controles contra el catálogo |
| `scripts/risk_assessment.sh` | Genera risk assessment trimestral |

## Uso Rápido

```bash
# Generar evidence pack
./scripts/compliance_generate.sh

# Verificar controles específicos
./scripts/compliance_check.sh --controls "NIST:AC-2,ISO:A.9.2.1" --report report.json

# Generar risk assessment trimestral
./scripts/risk_assessment.sh --quarter 2026-Q3
```

## Referencia

- **SPEC-034**: HIPAA/ISO27001 Evidence Pack
- **SPEC-011**: Runbook de respuesta a incidentes
- **SPEC-024**: Disaster Recovery
