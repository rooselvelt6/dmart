#!/bin/bash
# =============================================================================
# dMart UCI - DR Verify Script (SPEC-024)
# =============================================================================
# Verifies backup integrity: extracts to tempdir, verifies checksum against
# manifest, and writes a JSON verification report.
# Usage: ./scripts/dr_verify.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
BACKUP_SRC="latest"
REPORT_PATH="/tmp/dr_verify_report.json"
BACKUP_DIR="${DMART_DR_BACKUP_DESTINATION:-./backups/dr}"

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/dr_verify.sh [OPCIONES]

Opciones:
  --backup latest|path       Backup a verificar (default: latest)
  --report path              Ruta del reporte JSON (default: /tmp/dr_verify_report.json)
  --help                     Muestra esta ayuda

Ejemplo:
  ./scripts/dr_verify.sh --backup latest --report /tmp/verify.json
  ./scripts/dr_verify.sh --backup ./backups/dr/backup_20260915-140000.tar.zst
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --backup)  BACKUP_SRC="$2"; shift 2 ;;
        --report)  REPORT_PATH="$2"; shift 2 ;;
        --help)    show_help ;;
        *)         echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

# Resolve backup file
TAR_FILE=""
if [[ "$BACKUP_SRC" == "latest" ]]; then
    echo "🔍 Buscando último backup..."
    TAR_FILE=$(find "${BACKUP_DIR}" -name "backup_*.tar.zst" -type f -print 2>/dev/null | sort -r | head -1)
    if [[ -z "$TAR_FILE" ]]; then
        echo "❌ No se encontró ningún backup en ${BACKUP_DIR}"
        exit 1
    fi
else
    TAR_FILE="$BACKUP_SRC"
    if [[ ! -f "$TAR_FILE" ]]; then
        echo "❌ Archivo de backup no encontrado: ${TAR_FILE}"
        exit 1
    fi
fi

BACKUP_NAME=$(basename "$TAR_FILE")
MANIFEST_FILE="${TAR_FILE%.tar.zst}.json"
CHECKSUMS_MATCH="false"

echo "🔍 Verificando backup: ${BACKUP_NAME}"

# Extract to tempdir
TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR}"' EXIT

echo "📦 Extrayendo a directorio temporal..."
tar --zstd -xf "${TAR_FILE}" -C "${TMPDIR}" 2>/dev/null
echo "✅ Extracción exitosa"

# Verify checksum
if [[ -f "$MANIFEST_FILE" ]]; then
    echo "🔑 Verificando checksum contra manifest..."
    EXPECTED_CHECKSUM=$(grep -o '"checksum": *"sha256:[^"]*"' "$MANIFEST_FILE" | cut -d'"' -f4 | sed 's/sha256://')
    ACTUAL_CHECKSUM=$(sha256sum "$TAR_FILE" | cut -d' ' -f1)

    if [[ "$EXPECTED_CHECKSUM" == "$ACTUAL_CHECKSUM" ]]; then
        CHECKSUMS_MATCH="true"
        echo "✅ Checksum verificado: ${ACTUAL_CHECKSUM}"
    else
        echo "❌ Checksum NO coincide"
        echo "   Esperado: ${EXPECTED_CHECKSUM}"
        echo "   Obtenido: ${ACTUAL_CHECKSUM}"
    fi
else
    echo "⚠️  Manifest no encontrado, calculando checksum del tar"
    ACTUAL_CHECKSUM=$(sha256sum "$TAR_FILE" | cut -d' ' -f1)
fi

# Ensure report directory exists
mkdir -p "$(dirname "${REPORT_PATH}")"

# Write report
VERIFY_TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
cat > "${REPORT_PATH}" <<REPORT
{
  "backup_name": "${BACKUP_NAME}",
  "backup_path": "${TAR_FILE}",
  "verified_at": "${VERIFY_TIMESTAMP}",
  "checksums_match": ${CHECKSUMS_MATCH},
  "actual_checksum": "sha256:${ACTUAL_CHECKSUM}",
  "extracted_files": $(find "${TMPDIR}" -type f | wc -l),
  "report_path": "${REPORT_PATH}"
}
REPORT

echo ""
echo "📝 Reporte generado: ${REPORT_PATH}"

if [[ "$CHECKSUMS_MATCH" == "true" ]]; then
    echo "✅ Verificación completada exitosamente"
    exit 0
else
    echo "❌ Verificación fallida - checksums no coinciden"
    exit 1
fi
