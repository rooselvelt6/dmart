#!/bin/bash
# =============================================================================
# dMart UCI - Risk Assessment Script (SPEC-034)
# =============================================================================
# Generates a quarterly risk assessment markdown document with a template
# for identifying, evaluating, and tracking security risks.
# Usage: ./scripts/risk_assessment.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
QUARTER=""
OUTPUT_PATH=""

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/risk_assessment.sh [OPCIONES]

Opciones:
  --quarter YYYY-QN     Trimestre (ej: 2026-Q3)
  --output path         Ruta de salida (default: docs/compliance/risk_assessment/YYYY-QN.md)
  --help                Muestra esta ayuda

Ejemplo:
  ./scripts/risk_assessment.sh --quarter 2026-Q3 --output docs/compliance/risk_assessment/2026-Q3.md
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --quarter) QUARTER="$2"; shift 2 ;;
        --output)  OUTPUT_PATH="$2"; shift 2 ;;
        --help)    show_help ;;
        *)         echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

# Default quarter from current date
if [[ -z "$QUARTER" ]]; then
    MONTH=$(date +%m)
    YEAR=$(date +%Y)
    Q=$(( (MONTH - 1) / 3 + 1 ))
    QUARTER="${YEAR}-Q${Q}"
fi

# Default output path
if [[ -z "$OUTPUT_PATH" ]]; then
    OUTPUT_PATH="docs/compliance/risk_assessment/${QUARTER}.md"
fi

mkdir -p "$(dirname "${OUTPUT_PATH}")"

GENERATED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)

echo "📝 Generando Risk Assessment para ${QUARTER}..."

cat > "${OUTPUT_PATH}" <<RISK
# Risk Assessment — ${QUARTER}

**Generado:** ${GENERATED_AT}
**Responsable:** Compliance Officer / Security Lead
**Revisión:** Trimestral (SPEC-034)

## Resumen Ejecutivo

- Total de riesgos identificados: _pendiente_
- Riesgos críticos abiertos: _pendiente_
- Riesgos mitigados este trimestre: _pendiente
- Score de riesgo general: _pendiente_

## Riesgos Identificados

| ID | Riesgo | Probabilidad | Impacto | Score | Mitigación | Dueño | Estado | Deadline |
|----|--------|-------------|---------|-------|------------|-------|--------|----------|
| RSK-001 | Fallo en backup DR sin detección | Baja | Crítico | 12 | DR drill trimestral + alertas | SRE | Abierto | _definir_ |
| RSK-002 | Brecha de acceso no autorizado PHI | Baja | Crítico | 12 | RBAC (SPEC-004) + audit logs | Security | Mitigado | — |
| RSK-003 | Pérdida de datos por fallo DB | Baja | Crítico | 12 | Backup hourly + verify | SRE | Mitigado | — |
| RSK-004 | Regulación cubana sin claridad | Media | Medio | 8 | Regulatory watch + legal review | Compliance | Abierto | _definir_ |
| RSK-005 | Vendor/BAA vencido | Baja | Alto | 6 | Revisión trimestral | Compliance | Abierto | _definir_ |
| RSK-006 | Zero-day en dependencias Rust | Baja | Alto | 6 | cargo audit + dependabot | Security | Abierto | _definir_ |
| RSK-007 | Over-provisioning de recursos | Alta | Bajo | 4 | Right-sizing (SPEC-035) | DevOps | Abierto | _definir_ |
| RSK-008 | Incidente sin runbook documentado | Baja | Medio | 4 | SPEC-011 runbook | Security | Mitigado | — |

## Matriz de Probabilidad × Impacto

|  | Impacto Bajo | Impacto Medio | Impacto Alto | Impacto Crítico |
|--|-------------|---------------|--------------|-----------------|
| **Prob. Alta** | 4 | 8 | 12 | 16 |
| **Prob. Media** | 2 | 4 | 6 | 8 |
| **Prob. Baja** | 1 | 2 | 3 | 4 |

Score = Probabilidad × Impacto (escala 1-4 cada uno)

## Acciones de Mitigación Este Trimestre

- [ ] Ejecutar DR drill y documentar resultado
- [ ] Revisar BAA de todos los subprocesadores
- [ ] Ejecutar cargo audit y actualizar dependencias
- [ ] Completar right-sizing en staging
- [ ] Revisar cambios regulatorios cubanos

## Riesgos Transferidos / Aceptados

_Ninguno actualmente_

## Próxima Revisión

- Fecha: _definir inicio del próximo trimestre_
- Responsable: Compliance Officer
- Formato: Mismo template con scores actualizados
RISK

echo ""
echo "📝 Risk Assessment generado: ${OUTPUT_PATH}"
echo "   Trimestre: ${QUARTER}"
echo "   Riesgos template: 8 (pre-cargados con riesgos conocidos del proyecto)"
echo ""
echo "✅ Risk Assessment completado"
exit 0
