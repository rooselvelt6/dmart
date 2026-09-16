#!/bin/bash
# =============================================================================
# dMart UCI - DR Backup Script (SPEC-024)
# =============================================================================
# Creates full or incremental compressed backups with manifest and integrity
# verification. Rotates old backups based on retention policy.
# Usage: ./scripts/dr_backup.sh [OPTIONS]
# =============================================================================

set -euo pipefail

# --- Defaults ---
BACKUP_TYPE="incremental"
DEST_DIR="./backups/dr"
RETENTION_DAYS=7
COMPRESS="zstd"
DB_PATH="${DMART_DB_PATH:-./data/dmart.db}"
BACKUP_BASE="${DMART_DR_BACKUP_DESTINATION:-}"

# --- Parse args ---
show_help() {
    cat <<EOF
Uso: ./scripts/dr_backup.sh [OPCIONES]

Opciones:
  --type full|incremental    Tipo de backup (default: incremental)
  --destination dir          Directorio destino (default: ./backups/dr)
  --retention días           Retención en días (default: 7)
  --compress zstd            Algoritmo de compresión (default: zstd)
  --help                     Muestra esta ayuda

Ejemplo:
  ./scripts/dr_backup.sh --type full --destination /backups/dr --retention 14
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --type)       BACKUP_TYPE="$2"; shift 2 ;;
        --destination) DEST_DIR="$2"; shift 2 ;;
        --retention)  RETENTION_DAYS="$2"; shift 2 ;;
        --compress)   COMPRESS="$2"; shift 2 ;;
        --help)       show_help ;;
        *)            echo "❌ Opción desconocida: $1"; show_help ;;
    esac
done

# Validate type
if [[ "$BACKUP_TYPE" != "full" && "$BACKUP_TYPE" != "incremental" ]]; then
    echo "❌ Tipo de backup inválido: $BACKUP_TYPE (debe ser 'full' o 'incremental')"
    exit 1
fi

# Override destination if env var set
if [[ -n "$BACKUP_BASE" ]]; then
    DEST_DIR="$BACKUP_BASE"
fi

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
TIMESTAMP_ISO=$(date -u +%Y-%m-%dT%H:%M:%SZ)
BACKUP_NAME="backup_${TIMESTAMP}"
TAR_FILE="${DEST_DIR}/${BACKUP_NAME}.tar.zst"
MANIFEST_FILE="${DEST_DIR}/${BACKUP_NAME}.json"

mkdir -p "${DEST_DIR}"

echo "📦 Iniciando backup ${BACKUP_TYPE}..."
echo "   Destino: ${DEST_DIR}"

# Find parent for incremental
PARENT_ID=""
if [[ "$BACKUP_TYPE" == "incremental" ]]; then
    LATEST_MANIFEST=$(find "${DEST_DIR}" -name "backup_*.json" -type f -print 2>/dev/null | sort -r | head -1)
    if [[ -n "$LATEST_MANIFEST" ]]; then
        PARENT_ID=$(grep -o '"id": *"[^"]*"' "$LATEST_MANIFEST" | head -1 | cut -d'"' -f4)
        echo "   Padre: ${PARENT_ID}"
    else
        echo "   ⚠️  No se encontró backup anterior, convirtiendo a full"
        BACKUP_TYPE="full"
    fi
fi

# Create tar.zst
echo "📁 Creando archivo comprimido..."
tar --zstd -cf "${TAR_FILE}" -C "$(dirname "${DB_PATH}")" "$(basename "${DB_PATH}")" 2>/dev/null || {
    echo "❌ Error al crear el backup"
    exit 1
}

# Verify integrity
echo "🔍 Verificando integridad del backup..."
if tar -tzf "${TAR_FILE}" > /dev/null 2>&1; then
    echo "✅ Backup verificado correctamente"
else
    echo "❌ Verificación de integridad fallida"
    rm -f "${TAR_FILE}"
    exit 1
fi

# Calculate size and checksum
SIZE_BYTES=$(stat -c%s "${TAR_FILE}" 2>/dev/null || stat -f%z "${TAR_FILE}" 2>/dev/null)
CHECKSUM=$(sha256sum "${TAR_FILE}" | cut -d' ' -f1)

echo "📊 Tamaño: ${SIZE_BYTES} bytes"
echo "🔑 SHA-256: ${CHECKSUM}"

# Generate manifest
BACKUP_ID="backup-${TIMESTAMP}"
cat > "${MANIFEST_FILE}" <<MANIFEST
{
  "id": "${BACKUP_ID}",
  "type": "${BACKUP_TYPE}",
  "timestamp": "${TIMESTAMP_ISO}",
  "parent_id": ${PARENT_ID:+"\"${PARENT_ID}\""}${PARENT_ID:-"null"},
  "tables": {
    "placeholder": { "count": 0, "checksum": "" }
  },
  "size_bytes": ${SIZE_BYTES},
  "checksum": "sha256:${CHECKSUM}",
  "status": "completed"
}
MANIFEST

echo "📝 Manifest generado: ${MANIFEST_FILE}"

# Rotate old backups
echo "🧹 Rotando backups anteriores a ${RETENTION_DAYS} días..."
DELETED=0
while IFS= read -r -d '' old_file; do
    rm -f "$old_file"
    rm -f "${old_file%.tar.zst}.json"
    DELETED=$((DELETED + 1))
done < <(find "${DEST_DIR}" -name "backup_*.tar.zst" -type f -mtime "+${RETENTION_DAYS}" -print0 2>/dev/null)

if [[ $DELETED -gt 0 ]]; then
    echo "🗑️  ${DELETED} backup(s) antiguo(s) eliminado(s)"
else
    echo "🧹 No hay backups antiguos para rotar"
fi

echo "✅ Backup ${BACKUP_TYPE} completado: ${TAR_FILE}"
exit 0
