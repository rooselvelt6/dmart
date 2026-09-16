#!/bin/bash
# =============================================================================
# dMart UCI - Compliance Check Script (SPEC-034)
# =============================================================================
# Validates that requested controls exist in CONTROL_CATALOG.md and writes
# a JSON report with controls_implemented count.
# Usage: ./scripts/compliance_check.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
CONTROLS=""
REPORT_PATH="/tmp/compliance_check.json"
CATALOG_PATH="docs/compliance/CONTROL_CATALOG.md"

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/compliance_check.sh [OPCIONES]

Opciones:
  --controls lista   Lista de controles separados por coma (ej: NIST:AC-2,ISO:A.9.2.1)
  --report path      Ruta del reporte JSON (default: /tmp/compliance_check.json)
  --help             Muestra esta ayuda

Ejemplo:
  ./scripts/compliance_check.sh --controls "NIST:AC-2,ISO:A.9.2.1,HIPAA:164.308" --report report.json
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --controls) CONTROLS="$2"; shift 2 ;;
        --report)   REPORT_PATH="$2"; shift 2 ;;
        --help)     show_help ;;
        *)          echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

if [[ -z "$CONTROLS" ]]; then
    echo "❌ Se requiere --controls <lista>"
    show_help
fi

# Verify catalog exists
if [[ ! -f "$CATALOG_PATH" ]]; then
    echo "❌ Catálogo no encontrado: ${CATALOG_PATH}"
    exit 1
fi

echo "🔍 Verificando controles de compliance..."

# Parse controls
IFS=',' read -ra CONTROL_LIST <<< "$CONTROLS"

CONTROLS_TOTAL=0
CONTROLS_FOUND=0
CONTROLS_MISSING=0
MISSING_CONTROLS=""

CATALOG_CONTENT=$(cat "$CATALOG_PATH")

for ctrl in "${CONTROL_LIST[@]}"; do
    # Normalize: extract the control ID part (e.g., NIST:AC-2 → AC-2, ISO:A.9.2.1 → A.9.2.1)
    CONTROL_ID=$(echo "$ctrl" | sed 's/^[A-Z]*://' | xargs)

    CONTROLS_TOTAL=$((CONTROLS_TOTAL + 1))

    if echo "$CATALOG_CONTENT" | grep -q "$CONTROL_ID"; then
        CONTROLS_FOUND=$((CONTROLS_FOUND + 1))
        echo "   ✅ ${ctrl} — encontrado"
    else
        CONTROLS_MISSING=$((CONTROLS_MISSING + 1))
        MISSING_CONTROLS="${MISSING_CONTROLS}${ctrl},"
        echo "   ❌ ${ctrl} — NO encontrado en catálogo"
    fi
done

# Clean trailing comma
MISSING_CONTROLS="${MISSING_CONTROLS%,}"

# Calculate implemented count from catalog
CONTROLS_IMPLEMENTED=$(grep -c "✅ Implemented" "$CATALOG_PATH" 2>/dev/null || echo "0")
CONTROLS_IN_PROGRESS=$(grep -c "🟡 In progress" "$CATALOG_PATH" 2>/dev/null || echo "0")

# Generate report
mkdir -p "$(dirname "${REPORT_PATH}")"
CHECK_TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

cat > "${REPORT_PATH}" <<REPORT
{
  "generated_at": "${CHECK_TIMESTAMP}",
  "controls_checked": ${CONTROLS_TOTAL},
  "controls_found": ${CONTROLS_FOUND},
  "controls_missing": ${CONTROLS_MISSING},
  "controls_implemented_total": ${CONTROLS_IMPLEMENTED},
  "controls_in_progress_total": ${CONTROLS_IN_PROGRESS},
  "missing_controls": ${MISSING_CONTROLS:+\"${MISSING_CONTROLS}\"}${MISSING_CONTROLS:-"null"},
  "catalog_path": "${CATALOG_PATH}",
  "report_path": "${REPORT_PATH}"
}
REPORT

echo ""
echo "📝 Reporte: ${REPORT_PATH}"
echo "   Controles verificados: ${CONTROLS_TOTAL}"
echo "   Encontrados:          ${CONTROLS_FOUND}"
echo "   Faltantes:            ${CONTROLS_MISSING}"
echo "   Implementados (catálogo): ${CONTROLS_IMPLEMENTED}"

if [[ $CONTROLS_MISSING -gt 0 ]]; then
    echo ""
    echo "⚠️  Hay controles sin evidencia en el catálogo"
fi

echo ""
echo "✅ Verificación completada"
exit 0
