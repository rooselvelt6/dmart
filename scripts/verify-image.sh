#!/usr/bin/env bash
# =============================================================================
# verify-image.sh — Verifica firma y SBOM de una imagen publicada (SPEC-048)
#
# Uso:
#   scripts/verify-image.sh [IMAGE_REF]
#   scripts/verify-image.sh ghcr.io/ucigtm/dmart-server:v1.0.0
#
# Requiere: cosign (https://docs.sigstore.dev/cosign/installation/)
# Salida: exit 0 si firma + attestation CycloneDX verifican, != 0 si no.
# =============================================================================
set -euo pipefail

IMAGE_REF="${1:-ghcr.io/ucigtm/dmart-server:latest}"

OIDC_ISSUER="https://token.actions.githubusercontent.com"
CERT_IDENTITY_REGEX="^https://github.com/${GITHUB_REPOSITORY:-ucigtm/dmart}/.github/workflows/release-image.yml@.*$"

command -v cosign >/dev/null 2>&1 || {
  echo "❌ cosign no está instalado. Ver https://docs.sigstore.dev/cosign/installation/"
  exit 2
}

echo "🔎 Verificando firma keyless de ${IMAGE_REF}"
cosign verify \
  --certificate-oidc-issuer "$OIDC_ISSUER" \
  --certificate-identity-regexp "$CERT_IDENTITY_REGEX" \
  "$IMAGE_REF"

echo "🔎 Verificando attestation SBOM (CycloneDX) de ${IMAGE_REF}"
cosign verify-attestation \
  --type cyclonedx \
  --certificate-oidc-issuer "$OIDC_ISSUER" \
  --certificate-identity-regexp "$CERT_IDENTITY_REGEX" \
  "$IMAGE_REF"

echo "✅ Firma y SBOM verificados para ${IMAGE_REF}"
