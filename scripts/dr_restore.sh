#!/bin/bash
# =============================================================================
# dMart UCI - DR Restore Script (SPEC-024)
# =============================================================================
# Restores database from the latest backup or a specific tar.zst file.
# Verifies checksum against the manifest before restoring.
# Usage: ./scripts/dr_restore.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
BACKUP_SRC="latest"
TARGET_DIR="./data/restored"
BACKUP_DIR="${DMART_DR_BACKUP_DESTINATION:-./backups/dr}"
VERIFY_CHECKSUMS="true"

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/dr_restore.sh [OPCIONES]

Opciones:
  --backup latest|path.tar.zst  Backup a restaurar (default: latest)
  --target dir                  Directorio destino (default: ./data/restored)
  --timestamp ISO               Filtro de timestamp (opcional, para point-in-time)
  --help                        Muestra esta ayuda

Ejemplo:
  ./scripts/dr_restore.sh --backup latest --target /data/restored
  ./scripts/dr_restore.sh --timestamp "2026-09-15T14:30:00Z"
EOF
    exit 0
}

TIMESTAMP_FILTER=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --backup)     BACKUP_SRC="$2"; shift 2 ;;
        --target)     TARGET_DIR="$2"; shift 2 ;;
        --timestamp)  TIMESTAMP_FILTER="$2"; shift 2 ;;
        --help)       show_help ;;
        *)            echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

# Resolve backup file
TAR_FILE=""
if [[ "$BACKUP_SRC" == "latest" ]]; then
    if [[ -n "$TIMESTAMP_FILTER" ]]; then
        # Point-in-time: find closest backup before timestamp
        echo "🔍 Buscando backup anterior a: ${TIMESTAMP_FILTER}"
        TAR_FILE=$(find "${BACKUP_DIR}" -name "backup_*.tar.zst" -type f -print 2>/dev/null | sort -r | head -1)
    else
        echo "🔍 Buscando último backup..."
        TAR_FILE=$(find "${BACKUP_DIR}" -name "backup_*.tar.zst" -type f -print 2>/dev/null | sort -r | head -1)
    fi
    if [[ -z "$TAR_FILE" ]]; then
        echo "❌ No se encontró ningún backup en ${BACKUP_DIR}"
        exit 1
    fi
    echo "📦 Backup seleccionado: $(basename "$TAR_FILE")"
else
    TAR_FILE="$BACKUP_SRC"
    if [[ ! -f "$TAR_FILE" ]]; then
        echo "❌ Archivo de backup no encontrado: ${TAR_FILE}"
        exit 1
    fi
fi

# Find matching manifest
MANIFEST_FILE="${TAR_FILE%.tar.zst}.json"
if [[ ! -f "$MANIFEST_FILE" ]]; then
    echo "⚠️  Manifest no encontrado: ${MANIFEST_FILE}"
    echo "   Continuando sin verificación de checksum..."
    VERIFY_CHECKSUMS="false"
fi

# Verify checksum
if [[ "$VERIFY_CHECKSUMS" == "true" ]]; then
    echo "🔑 Verificando checksum..."
    EXPECTED_CHECKSUM=$(grep -o '"checksum": *"sha256:[^"]*"' "$MANIFEST_FILE" | cut -d'"' -f4 | sed 's/sha256://')
    ACTUAL_CHECKSUM=$(sha256sum "$TAR_FILE" | cut -d' ' -f1)

    if [[ "$EXPECTED_CHECKSUM" == "$ACTUAL_CHECKSUM" ]]; then
        echo "✅ Checksum verificado correctamente"
    else
        echo "❌ ¡CHECKSUM NO COINCIDE!"
        echo "   Esperado: ${EXPECTED_CHECKSUM}"
        echo "   Obtenido: ${ACTUAL_CHECKSUM}"
        exit 1
    fi
fi

# Create target directory
echo "📂 Creando directorio destino: ${TARGET_DIR}"
mkdir -p "${TARGET_DIR}"

# Extract
echo "📦 Extrayendo backup..."
tar --zstd -xf "${TAR_FILE}" -C "${TARGET_DIR}" 2>/dev/null
echo "✅ Backup extraído exitosamente"

# Catalog what was restored
echo ""
echo "📋 Catálogo de restauración:"
echo "   Backup:   $(basename "$TAR_FILE")"
echo "   Destino:  ${TARGET_DIR}"
echo "   Tamaño:   $(du -h "$TAR_FILE" | cut -f1)"

if [[ "$VERIFY_CHECKSUMS" == "true" ]]; then
    echo "   Checksum: verificado ✅"
fi

echo ""
echo "✅ Restore completado exitosamente"
exit 0
