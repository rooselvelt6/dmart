#!/bin/bash
# =============================================================================
# dMart UCI - Cost Forecast Script (SPEC-035)
# =============================================================================
# Generates a simple cost forecast projection for future months.
# Uses linear projection from current costs.
# Usage: ./scripts/cost_forecast.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
MONTHS=6
OUTPUT_PATH="docs/cost/reports/forecast.md"
CURRENT_MONTHLY=1368.50  # Default estimate
GROWTH_RATE=0.02         # 2% monthly growth estimate

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/cost_forecast.sh [OPCIONES]

Opciones:
  --months n               Meses a proyectar (default: 6)
  --output path            Ruta de salida (default: docs/cost/reports/forecast.md)
  --current-monthly cost   Coste mensual actual (default: 1368.50)
  --growth-rate rate       Tasa de crecimiento mensual (default: 0.02)
  --help                   Muestra esta ayuda

Ejemplo:
  ./scripts/cost_forecast.sh --months 12 --output docs/cost/reports/forecast.md
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --months)          MONTHS="$2"; shift 2 ;;
        --output)          OUTPUT_PATH="$2"; shift 2 ;;
        --current-monthly) CURRENT_MONTHLY="$2"; shift 2 ;;
        --growth-rate)     GROWTH_RATE="$2"; shift 2 ;;
        --help)            show_help ;;
        *)                 echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

mkdir -p "$(dirname "${OUTPUT_PATH}")"

GENERATED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
CURRENT_MONTH=$(date +%Y-%m)
BUDGET_MONTHLY=1500

echo "📈 Generando forecast de costes para ${MONTHS} meses..."

# Generate forecast table
FORECAST_TABLE=""
TOTAL_COST=0
for i in $(seq 0 "$MONTHS"); do
    FORECAST_MONTH=$(date -d "${CURRENT_MONTH}-01 + ${i} months" +%Y-%m 2>/dev/null || echo "T+${i}")
    FORECAST_COST=$(echo "scale=2; ${CURRENT_MONTHLY} * (1 + ${GROWTH_RATE}) ^ ${i}" | bc -l 2>/dev/null || echo "0")
    TOTAL_COST=$(echo "scale=2; ${TOTAL_COST} + ${FORECAST_COST}" | bc -l 2>/dev/null || echo "0")

    BUDGET_STATUS="✅"
    if (( $(echo "${FORECAST_COST} > ${BUDGET_MONTHLY}" | bc -l 2>/dev/null || echo 0) )); then
        BUDGET_STATUS="⚠️ EXCEDE BUDGET"
    fi

    FORECAST_TABLE="${FORECAST_TABLE}| ${FORECAST_MONTH} | \$${FORECAST_COST} | ${BUDGET_STATUS} |
"
done

cat > "${OUTPUT_PATH}" <<FORECAST
# Forecast de Costes — dMart UCI

**Generado:** ${GENERATED_AT}
**Mes base:** ${CURRENT_MONTH}
**Coste mensual actual:** \$${CURRENT_MONTHLY}
**Crecimiento estimado:** $(echo "${GROWTH_RATE} * 100" | bc -l 2>/dev/null || echo "2")% mensual
**Budget mensual:** \$${BUDGET_MONTHLY}

---

## Proyección

| Mes | Coste Estimado | Estado Budget |
|-----|----------------|---------------|
${FORECAST_TABLE}
---

## Resumen

- **Total proyectado (${MONTHS} meses):** \$${TOTAL_COST}
- **Promedio mensual:** \$$(echo "scale=2; ${TOTAL_COST} / ${MONTHS}" | bc -l 2>/dev/null || echo "0")
- **Meses que exceden budget:** _ver tabla_

## Recomendaciones

- Si el crecimiento supera el 2% mensual, revisar right-sizing (SPEC-035)
- Monitorear anomalies con \`cost_idle_detect.sh\`
- Actualizar forecast mensualmente con datos reales
- Conectar a cloud billing API para datos precisos

---

_Forecast basado en proyección lineal. Para mayor precisión, usar datos históricos reales._
FORECAST

echo ""
echo "📈 Forecast generado: ${OUTPUT_PATH}"
echo "   Meses proyectados: ${MONTHS}"
echo "   Coste actual: \$${CURRENT_MONTHLY}/mes"
echo "   Total proyectado: \$${TOTAL_COST}"
echo ""
echo "✅ Forecast completado"
exit 0
