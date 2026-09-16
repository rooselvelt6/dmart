#!/bin/bash
# =============================================================================
# dMart UCI - Cost Idle Detect Script (SPEC-035)
# =============================================================================
# Detects idle resources (directories, deployments) that haven't been used
# in a configurable number of days.
# Usage: ./scripts/cost_idle_detect.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
THRESHOLD_DAYS=7
DRY_RUN="false"

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/cost_idle_detect.sh [OPCIONES]

Opciones:
  --threshold-days days   Días sin actividad para considerar idle (default: 7)
  --dry-run               Solo reportar, no actuar (default: false)
  --help                  Muestra esta ayuda

Ejemplo:
  ./scripts/cost_idle_detect.sh --threshold-days 7 --dry-run
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --threshold-days) THRESHOLD_DAYS="$2"; shift 2 ;;
        --dry-run)        DRY_RUN="true"; shift ;;
        --help)           show_help ;;
        *)                echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

echo "🔍 ============================================"
echo "🔍  Detección de Recursos Idle"
echo "🔍 ============================================"
echo "   Umbral: ${THRESHOLD_DAYS} días sin actividad"
echo "   Modo:   $([ "$DRY_RUN" == "true" ] && echo "dry-run (solo reporte)" || echo "activo")"
echo ""

IDLE_COUNT=0

# --- Check directories ---
echo "📁 Verificando directorios..."

# Check for common idle directories
for check_dir in "./data/staging" "./tmp" "./e2e" "./dev" "./.cache"; do
    if [[ -d "$check_dir" ]]; then
        # Check last modification time
        LAST_MOD=$(find "$check_dir" -maxdepth 1 -type f -printf '%T@\n' 2>/dev/null | sort -rn | head -1 || echo "0")
        if [[ -n "$LAST_MOD" ]] && [[ "$LAST_MOD" != "0" ]]; then
            DAYS_OLD=$(echo "($LAST_MOD - $(date +%s)) / 86400" | bc 2>/dev/null || echo "0")
            DAYS_OLD=${DAYS_OLD#-}  # absolute value
            if [[ "$DAYS_OLD" -ge "$THRESHOLD_DAYS" ]]; then
                echo "   ⚠️  ${check_dir} — idle ${DAYS_OLD} días"
                IDLE_COUNT=$((IDLE_COUNT + 1))
                if [[ "$DRY_RUN" != "true" ]]; then
                    echo "      📝 Acción sugerida: revisar y limpiar"
                fi
            fi
        fi
    fi
done

# --- Check Kubernetes deployments (if kubectl available) ---
if command -v kubectl &>/dev/null; then
    echo ""
    echo "☸️  Verificando deployments en Kubernetes..."

    # Check for staging/dev namespaces
    for ns in staging dev e2e; do
        DEPLOYMENTS=$(kubectl get deployments -n "$ns" -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || echo "")
        if [[ -n "$DEPLOYMENTS" ]]; then
            for deploy in $DEPLOYMENTS; do
                # Check last activity (simplified: check if pods are running)
                READY=$(kubectl get deployment "$deploy" -n "$ns" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
                if [[ "${READY:-0}" -eq 0 ]]; then
                    echo "   ⚠️  ${ns}/${deploy} — sin replicas activas"
                    IDLE_COUNT=$((IDLE_COUNT + 1))
                fi
            done
        fi
    done

    # Check for stopped/zero-replica deployments
    echo ""
    echo "☸️  Verificando deployments con replicas=0..."
    ZERO_REPLICAS=$(kubectl get deployments --all-namespaces -o jsonpath='{range .items[*]}{.metadata.namespace}/{.metadata.name} {.spec.replicas}{"\n"}{end}' 2>/dev/null | grep ' 0$' || echo "")
    if [[ -n "$ZERO_REPLICAS" ]]; then
        while IFS= read -r line; do
            echo "   ⚠️  ${line} — idle (0 replicas)"
            IDLE_COUNT=$((IDLE_COUNT + 1))
        done <<< "$ZERO_REPLICAS"
    fi
else
    echo ""
    echo "⚠️  kubectl no disponible — saltando verificación K8s"
fi

# --- Summary ---
echo ""
echo "🔍 ============================================"
echo "🔍  Resumen de Detección"
echo "🔍 ============================================"
echo "   Recursos idle detectados: ${IDLE_COUNT}"

if [[ $IDLE_COUNT -eq 0 ]]; then
    echo "   ✅ No se detectaron recursos idle"
else
    echo "   ⚠️  Se encontraron ${IDLE_COUNT} recursos potencialmente idle"
    if [[ "$DRY_RUN" == "true" ]]; then
        echo "   📝 Modo dry-run: sin acciones tomadas"
    else
        echo "   📝 Revisar manualmente antes de eliminar"
    fi
fi

echo ""
echo "✅ Detección de idle completada"
exit 0
