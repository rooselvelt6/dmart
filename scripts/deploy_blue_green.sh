#!/bin/bash
# =============================================================================
# dMart UCI - Blue/Green Deploy Script (SPEC-026)
# =============================================================================
# Performs a zero-downtime blue/green deployment. Applies the green deployment,
# waits for healthcheck, swaps the service selector, and maintains rollback window.
# Usage: ./scripts/deploy_blue_green.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
IMAGE_TAG=""
NAMESPACE="dmart"
HEALTHCHECK_URL="/obs/health"
SWITCH_TIMEOUT=300
ROLLBACK_WINDOW=1800

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/deploy_blue_green.sh [OPCIONES]

Opciones:
  --image tag               Imagen a desplegar (requerido)
  --namespace ns            Namespace de K8s (default: dmart)
  --healthcheck-url url     Ruta de healthcheck (default: /obs/health)
  --switch-timeout segs     Timeout para swap de servicio (default: 300)
  --rollback-window segs    Tiempo de ventana de rollback (default: 1800)
  --help                    Muestra esta ayuda

Ejemplo:
  ./scripts/deploy_blue_green.sh --image ghcr.io/ucigtm/dmart-server:v1.3.0
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --image)            IMAGE_TAG="$2"; shift 2 ;;
        --namespace)        NAMESPACE="$2"; shift 2 ;;
        --healthcheck-url)  HEALTHCHECK_URL="$2"; shift 2 ;;
        --switch-timeout)   SWITCH_TIMEOUT="$2"; shift 2 ;;
        --rollback-window)  ROLLBACK_WINDOW="$2"; shift 2 ;;
        --help)             show_help ;;
        *)                  echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

if [[ -z "$IMAGE_TAG" ]]; then
    echo "❌ Se requiere --image <tag>"
    show_help
fi

# Detect active slot
ACTIVE_SLOT="blue"
if command -v kubectl &>/dev/null; then
    ACTIVE_SLOT=$(kubectl get svc dmart -n "${NAMESPACE}" -o jsonpath='{.spec.selector.slot}' 2>/dev/null || echo "blue")
    echo "📡 Slot activo actual: ${ACTIVE_SLOT}"
fi

# Determine target slot
if [[ "$ACTIVE_SLOT" == "blue" ]]; then
    TARGET_SLOT="green"
else
    TARGET_SLOT="blue"
fi

echo "🚀 ============================================"
echo "🚀  Blue/Green Deploy"
echo "🚀 ============================================"
echo "   Imagen:     ${IMAGE_TAG}"
echo "   Namespace:  ${NAMESPACE}"
echo "   Slot activo: ${ACTIVE_SLOT}"
echo "   Target:     ${TARGET_SLOT}"
echo ""

# --- Step 1: Apply green deployment ---
echo "📋 Paso 1/5: Aplicando deployment ${TARGET_SLOT}..."
if command -v kubectl &>/dev/null; then
    cat <<EOF | kubectl apply -n "${NAMESPACE}" -f -
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dmart-${TARGET_SLOT}
  labels:
    app: dmart
    slot: ${TARGET_SLOT}
spec:
  replicas: 3
  selector:
    matchLabels:
      app: dmart
      slot: ${TARGET_SLOT}
  template:
    metadata:
      labels:
        app: dmart
        slot: ${TARGET_SLOT}
    spec:
      containers:
        - name: dmart
          image: ${IMAGE_TAG}
          ports:
            - containerPort: 8080
          readinessProbe:
            httpGet:
              path: ${HEALTHCHECK_URL}
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          livenessProbe:
            httpGet:
              path: ${HEALTHCHECK_URL}
              port: 8080
            initialDelaySeconds: 15
            periodSeconds: 20
EOF
    echo "   ✅ Deployment ${TARGET_SLOT} aplicado"
else
    echo "   ⚠️  kubectl no encontrado — simulando aplicación"
    echo "   📝 Comando que se ejecutaría:"
    echo "      kubectl apply -n ${NAMESPACE} -f dmart-${TARGET_SLOT}-deployment.yaml"
fi

# --- Step 2: Wait for healthcheck ---
echo ""
echo "📋 Paso 2/5: Esperando healthcheck de ${TARGET_SLOT} (timeout: ${SWITCH_TIMEOUT}s)..."
HEALTH_OK=false

if command -v kubectl &>/dev/null; then
    ELAPSED=0
    while [[ $ELAPSED -lt $SWITCH_TIMEOUT ]]; do
        READY=$(kubectl get deployment "dmart-${TARGET_SLOT}" -n "${NAMESPACE}" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
        if [[ "${READY:-0}" -ge 1 ]]; then
            HEALTH_OK=true
            echo "   ✅ Deployment ${TARGET_SLOT} saludable (${READY} replicas listas)"
            break
        fi
        sleep 5
        ELAPSED=$((ELAPSED + 5))
        echo "   ⏳ Esperando... (${ELAPSED}s/${SWITCH_TIMEOUT}s)"
    done

    if [[ "$HEALTH_OK" != "true" ]]; then
        echo "   ❌ Healthcheck falló después de ${SWITCH_TIMEOUT}s"
        echo "   🧹 Limpiando deployment fallido..."
        kubectl delete deployment "dmart-${TARGET_SLOT}" -n "${NAMESPACE}" 2>/dev/null || true
        exit 1
    fi
else
    echo "   ⚠️  kubectl no encontrado — simulando healthcheck"
    echo "   📝 Comando que se ejecutaría:"
    echo "      kubectl rollout status deployment/dmart-${TARGET_SLOT} -n ${NAMESPACE} --timeout=${SWITCH_TIMEOUT}s"
    HEALTH_OK=true
fi

# --- Step 3: Swap service selector ---
echo ""
echo "📋 Paso 3/5: Intercambiando service selector a ${TARGET_SLOT}..."
if command -v kubectl &>/dev/null; then
    kubectl patch svc dmart -n "${NAMESPACE}" -p "{\"spec\":{\"selector\":{\"slot\":\"${TARGET_SLOT}\"}}}"
    echo "   ✅ Service selector cambiado a ${TARGET_SLOT}"
else
    echo "   ⚠️  kubectl no encontrado — simulando swap"
    echo "   📝 Comando que se ejecutaría:"
    echo "      kubectl patch svc dmart -n ${NAMESPACE} -p '{\"spec\":{\"selector\":{\"slot\":\"${TARGET_SLOT}\"}}}'"
fi

# --- Step 4: Verify active slot ---
echo ""
echo "📋 Paso 4/5: Verificando que el tráfico apunta a ${TARGET_SLOT}..."
if command -v kubectl &>/dev/null; then
    CURRENT_SLOT=$(kubectl get svc dmart -n "${NAMESPACE}" -o jsonpath='{.spec.selector.slot}' 2>/dev/null || echo "unknown")
    if [[ "$CURRENT_SLOT" == "$TARGET_SLOT" ]]; then
        echo "   ✅ Service apuntando a ${TARGET_SLOT}"
    else
        echo "   ❌ Service apunta a ${CURRENT_SLOT}, esperaba ${TARGET_SLOT}"
        exit 1
    fi
else
    echo "   ⚠️  Simulación: verificar manualmente con kubectl get svc dmart"
fi

# --- Step 5: Rollback window ---
echo ""
echo "📋 Paso 5/5: Ventana de rollback abierta por ${ROLLBACK_WINDOW}s"
echo "   El slot anterior (${ACTIVE_SLOT}) permanece standby."
echo "   Para rollback: ./scripts/deploy_rollback.sh --namespace ${NAMESPACE} --target ${ACTIVE_SLOT}"

echo ""
echo "🚀 ============================================"
echo "🚀  Blue/Green Deploy completado"
echo "🚀 ============================================"
echo "   Slot activo:  ${TARGET_SLOT}"
echo "   Slot standby: ${ACTIVE_SLOT}"
echo "   Imagen:       ${IMAGE_TAG}"
echo ""
exit 0
