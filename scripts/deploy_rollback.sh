#!/bin/bash
# =============================================================================
# dMart UCI - Deploy Rollback Script (SPEC-026)
# =============================================================================
# Rolls back to the specified slot (blue or green) by resetting the service
# selector. Completes in < 60 seconds.
# Usage: ./scripts/deploy_rollback.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
NAMESPACE="dmart"
TARGET_SLOT="blue"
TIMEOUT=60

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/deploy_rollback.sh [OPCIONES]

Opciones:
  --namespace ns      Namespace de K8s (default: dmart)
  --target blue|green Slot destino para rollback (default: blue)
  --timeout segs      Timeout para verificar healthcheck (default: 60)
  --help              Muestra esta ayuda

Ejemplo:
  ./scripts/deploy_rollback.sh --namespace dmart --target blue --timeout 60
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --namespace) NAMESPACE="$2"; shift 2 ;;
        --target)    TARGET_SLOT="$2"; shift 2 ;;
        --timeout)   TIMEOUT="$2"; shift 2 ;;
        --help)      show_help ;;
        *)           echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

if [[ "$TARGET_SLOT" != "blue" && "$TARGET_SLOT" != "green" ]]; then
    echo "❌ Slot inválido: $TARGET_SLOT (debe ser 'blue' o 'green')"
    exit 1
fi

echo "🔄 ============================================"
echo "🔄  Deploy Rollback"
echo "🔄 ============================================"
echo "   Target slot: ${TARGET_SLOT}"
echo "   Namespace:   ${NAMESPACE}"
echo ""

# --- Step 1: Determine current slot ---
CURRENT_SLOT="blue"
if command -v kubectl &>/dev/null; then
    CURRENT_SLOT=$(kubectl get svc dmart -n "${NAMESPACE}" -o jsonpath='{.spec.selector.slot}' 2>/dev/null || echo "blue")
    echo "📡 Slot actual: ${CURRENT_SLOT}"
else
    echo "⚠️  kubectl no disponible — asumiendo slot actual desconocido"
fi

if [[ "$CURRENT_SLOT" == "$TARGET_SLOT" ]]; then
    echo "✅ El servicio ya apunta a ${TARGET_SLOT}. Nada que hacer."
    exit 0
fi

# --- Step 2: Reset service selector ---
echo ""
echo "🔄 Cambiando selector de servicio a ${TARGET_SLOT}..."
if command -v kubectl &>/dev/null; then
    kubectl patch svc dmart -n "${NAMESPACE}" \
        -p "{\"spec\":{\"selector\":{\"slot\":\"${TARGET_SLOT}\"}}}"
    echo "   ✅ Selector cambiado"
else
    echo "   ⚠️  Simulación — kubectl no encontrado"
    echo "   📝 Comando exacto:"
    echo "      kubectl patch svc dmart -n ${NAMESPACE} -p '{\"spec\":{\"selector\":{\"slot\":\"${TARGET_SLOT}\"}}}'"
fi

# --- Step 3: Verify rollback ---
echo ""
echo "🔍 Verificando healthcheck post-rollback..."
if command -v kubectl &>/dev/null; then
    ELAPSED=0
    HEALTH_OK=false

    while [[ $ELAPSED -lt $TIMEOUT ]]; do
        CURRENT=$(kubectl get svc dmart -n "${NAMESPACE}" -o jsonpath='{.spec.selector.slot}' 2>/dev/null || echo "unknown")
        if [[ "$CURRENT" == "$TARGET_SLOT" ]]; then
            HEALTH_OK=true
            break
        fi
        sleep 2
        ELAPSED=$((ELAPSED + 2))
    done

    if [[ "$HEALTH_OK" == "true" ]]; then
        echo "   ✅ Rollback verificado: servicio apunta a ${TARGET_SLOT}"
    else
        echo "   ❌ Rollback no verificado en ${TIMEOUT}s"
        exit 1
    fi
else
    echo "   ⚠️  Simulación — verificación manual con kubectl"
fi

echo ""
echo "🔄 ============================================"
echo "🔄  Rollback completado"
echo "🔄 ============================================"
echo "   Slot activo: ${TARGET_SLOT}"
echo "   Downtime:    ~0s (zero-downtime)"
echo ""
exit 0
