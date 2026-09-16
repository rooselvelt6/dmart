#!/bin/bash
# =============================================================================
# dMart UCI - Deploy Status Script (SPEC-026)
# =============================================================================
# Shows the current status of blue/green deployments and the active slot.
# Usage: ./scripts/deploy_status.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
NAMESPACE="dmart"

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/deploy_status.sh [OPCIONES]

Opciones:
  --namespace ns    Namespace de K8s (default: dmart)
  --help            Muestra esta ayuda

Ejemplo:
  ./scripts/deploy_status.sh --namespace dmart
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --namespace) NAMESPACE="$2"; shift 2 ;;
        --help)      show_help ;;
        *)           echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

echo "📊 ============================================"
echo "📊  dMart Deploy Status"
echo "📊 ============================================"
echo ""

if ! command -v kubectl &>/dev/null; then
    echo "⚠️  kubectl no encontrado en PATH"
    echo ""
    echo "📝 Comandos de verificación manual:"
    echo "   kubectl get deployment dmart-blue -n ${NAMESPACE}"
    echo "   kubectl get deployment dmart-green -n ${NAMESPACE}"
    echo "   kubectl get svc dmart -n ${NAMESPACE} -o jsonpath='{.spec.selector}'"
    exit 0
fi

# Active slot
echo "📡 Slot activo:"
ACTIVE_SLOT=$(kubectl get svc dmart -n "${NAMESPACE}" -o jsonpath='{.spec.selector.slot}' 2>/dev/null || echo "desconocido")
echo "   → ${ACTIVE_SLOT}"
echo ""

# Blue deployment
echo "🔵 Deployment Blue:"
BLUE_READY=$(kubectl get deployment dmart-blue -n "${NAMESPACE}" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
BLUE_DESIRED=$(kubectl get deployment dmart-blue -n "${NAMESPACE}" -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "0")
BLUE_IMAGE=$(kubectl get deployment dmart-blue -n "${NAMESPACE}" -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null || echo "N/A")
BLUE_STATUS="inactive"
[[ "$BLUE_READY" -ge 1 ]] 2>/dev/null && BLUE_STATUS="active"
echo "   Image:     ${BLUE_IMAGE}"
echo "   Replicas:  ${BLUE_READY:-0}/${BLUE_DESIRED:-0}"
echo "   Status:    ${BLUE_STATUS}"
echo ""

# Green deployment
echo "🟢 Deployment Green:"
GREEN_READY=$(kubectl get deployment dmart-green -n "${NAMESPACE}" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
GREEN_DESIRED=$(kubectl get deployment dmart-green -n "${NAMESPACE}" -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "0")
GREEN_IMAGE=$(kubectl get deployment dmart-green -n "${NAMESPACE}" -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null || echo "N/A")
GREEN_STATUS="inactive"
[[ "$GREEN_READY" -ge 1 ]] 2>/dev/null && GREEN_STATUS="active"
echo "   Image:     ${GREEN_IMAGE}"
echo "   Replicas:  ${GREEN_READY:-0}/${GREEN_DESIRED:-0}"
echo "   Status:    ${GREEN_STATUS}"
echo ""

# Canary info
echo "🐤 Canary:"
CANARY_DEPLOYED=$(kubectl get deployment dmart-canary -n "${NAMESPACE}" 2>/dev/null && echo "yes" || echo "no")
if [[ "$CANARY_DEPLOYED" == "yes" ]]; then
    CANARY_READY=$(kubectl get deployment dmart-canary -n "${NAMESPACE}" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
    CANARY_IMAGE=$(kubectl get deployment dmart-canary -n "${NAMESPACE}" -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null || echo "N/A")
    echo "   Image:   ${CANARY_IMAGE}"
    echo "   Replicas: ${CANARY_READY:-0}"
else
    echo "   No hay canary activo"
fi
echo ""

echo "📊 ============================================"
exit 0
