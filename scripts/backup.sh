#!/bin/bash
# =============================================================================
# dMart UCI - Backup Script
# =============================================================================
# Creates compressed, timestamped backups of the SurrealDB database.
# Usage: ./scripts/backup.sh [output_dir]
# =============================================================================

set -euo pipefail

BACKUP_DIR="${1:-./backups}"
DB_PATH="${DMART_DB_PATH:-./data/dmart.db}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="${BACKUP_DIR}/dmart_backup_${TIMESTAMP}.tar.gz"
RETENTION_DAYS=${RETENTION_DAYS:-7}

mkdir -p "${BACKUP_DIR}"

if [ ! -f "${DB_PATH}" ]; then
    echo "ERROR: Database not found at ${DB_PATH}"
    exit 1
fi

echo "📦 Creating backup: ${BACKUP_FILE}"
tar -czf "${BACKUP_FILE}" -C "$(dirname "${DB_PATH}")" "$(basename "${DB_PATH}")"

# Verify backup integrity
echo "🔍 Verifying backup integrity..."
tar -tzf "${BACKUP_FILE}" > /dev/null 2>&1 && echo "✅ Backup verified successfully" || {
    echo "❌ Backup verification failed!"
    rm -f "${BACKUP_FILE}"
    exit 1
}

# Calculate size
SIZE=$(du -h "${BACKUP_FILE}" | cut -f1)
echo "📊 Backup size: ${SIZE}"

# Rotate old backups
echo "🧹 Cleaning backups older than ${RETENTION_DAYS} days..."
find "${BACKUP_DIR}" -name "dmart_backup_*.tar.gz" -mtime "+${RETENTION_DAYS}" -delete

echo "✅ Backup completed: ${BACKUP_FILE}"
