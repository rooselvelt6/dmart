#!/bin/bash
# dMart Backup Script — SPEC-009
# Ejecuta backup de SurrealKV (dmart.db) y Valkey (RDB/AOF)
# Métricas expuestas via textfile collector para Prometheus

set -euo pipefail

# ─── Configuración ───
BACKUP_ROOT="${BACKUP_STORAGE_PATH:-/backups}"
DATE=$(date -u +%Y-%m-%d-%H%M%S)
TIMESTAMP_ISO=$(date -u +%Y-%m-%dT%H:%M:%SZ)
TMP_DIR=$(mktemp -d)
METRICS_DIR="/var/lib/node_exporter"
METRICS_FILE="$METRICS_DIR/backup.prom"

# Targets a backupear
TARGETS=("surreal" "valkey")

# ─── Cleanup ───
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# ─── Logging ───
log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"
}

# ─── Métricas helpers ───
write_metrics() {
    local target=$1
    local status=$2
    local duration=$3
    local size=$4

    cat >> "$METRICS_FILE" <<EOF
backup_success{target="$target"} $status
backup_duration_seconds{target="$target"} $duration
backup_size_bytes{target="$target"} $size
backup_age_hours{target="$target"} 0
EOF
}

# ─── Inicializar archivo métricas ───
mkdir -p "$METRICS_DIR"
> "$METRICS_FILE"

# ─── 1. SurrealKV (embedded SQLite/SurrealKV) ───
log "Starting SurrealKV backup..."
START_SURREAL=$(date +%s)

# Copia consistente via SQLite .backup (requiere sqlite3)
if command -v sqlite3 >/dev/null 2>&1; then
    if sqlite3 /app/data/dmart.db ".backup '$TMP_DIR/dmart.db'"; then
        gzip -c "$TMP_DIR/dmart.db" > "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz"
        SURREAL_SIZE=$(stat -c%s "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz")
        SURREAL_CHECKSUM=$(sha256sum "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz" | cut -d' ' -f1)
        log "SurrealKV backup OK: $SURREAL_SIZE bytes, sha256=$SURREAL_CHECKSUM"
        SURREAL_STATUS=1
    else
        log "ERROR: sqlite3 .backup failed"
        SURREAL_STATUS=0
        SURREAL_SIZE=0
        SURREAL_CHECKSUM=""
    fi
else
    log "WARN: sqlite3 not found, falling back to file copy (may be inconsistent)"
    cp /app/data/dmart.db "$TMP_DIR/dmart.db"
    gzip -c "$TMP_DIR/dmart.db" > "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz"
    SURREAL_SIZE=$(stat -c%s "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz")
    SURREAL_CHECKSUM=$(sha256sum "$BACKUP_ROOT/surreal/dmart-$DATE.db.gz" | cut -d' ' -f1)
    SURREAL_STATUS=1
fi

END_SURREAL=$(date +%s)
SURREAL_DURATION=$((END_SURREAL - START_SURREAL))
write_metrics "surreal" "$SURREAL_STATUS" "$SURREAL_DURATION" "$SURREAL_SIZE"

# ─── 2. Valkey (Redis-compatible) ───
log "Starting Valkey backup..."
START_VALKEY=$(date +%s)

VALKEY_HOST="${DMART_VALKEY_HOST:-valkey}"
VALKEY_PORT="${DMART_VALKEY_PORT:-6379}"
VALKEY_CLI="redis-cli -h $VALKEY_HOST -p $VALKEY_PORT"

# Trigger BGSAVE
if $VALKEY_CLI BGSAVE; then
    log "BGSAVE triggered, waiting for completion..."
    # Poll LASTSAVE hasta que cambie (máx 300s)
    LASTSAVE=$($VALKEY_CLI LASTSAVE)
    TIMEOUT=300
    while [ "$($VALKEY_CLI LASTSAVE)" = "$LASTSAVE" ]; do
        sleep 2
        TIMEOUT=$((TIMEOUT - 2))
        if [ $TIMEOUT -le 0 ]; then
            log "ERROR: BGSAVE timeout after 300s"
            VALKEY_RDB_STATUS=0
            break
        fi
    done
    if [ $TIMEOUT -gt 0 ]; then
        log "BGSAVE completed"
        VALKEY_RDB_STATUS=1
    fi
else
    log "ERROR: BGSAVE command failed"
    VALKEY_RDB_STATUS=0
fi

# Copiar RDB
if [ $VALKEY_RDB_STATUS -eq 1 ]; then
    # El path del RDB depende de configuración Valkey (dir + dbfilename)
    # Por defecto en contenedor: /data/dump.rdb
    RDB_PATH="/data/dump.rdb"
    if [ -f "$RDB_PATH" ]; then
        cp "$RDB_PATH" "$TMP_DIR/dump.rdb"
        gzip -c "$TMP_DIR/dump.rdb" > "$BACKUP_ROOT/valkey/dump-$DATE.rdb.gz"
        VALKEY_RDB_SIZE=$(stat -c%s "$BACKUP_ROOT/valkey/dump-$DATE.rdb.gz")
        VALKEY_RDB_CHECKSUM=$(sha256sum "$BACKUP_ROOT/valkey/dump-$DATE.rdb.gz" | cut -d' ' -f1)
        log "Valkey RDB backup OK: $VALKEY_RDB_SIZE bytes, sha256=$VALKEY_RDB_CHECKSUM"
    else
        log "WARN: RDB file not found at $RDB_PATH"
        VALKEY_RDB_STATUS=0
        VALKEY_RDB_SIZE=0
        VALKEY_RDB_CHECKSUM=""
    fi
else
    VALKEY_RDB_SIZE=0
    VALKEY_RDB_CHECKSUM=""
fi

# Copiar AOF si existe
AOF_PATH="/data/appendonly.aof"
if [ -f "$AOF_PATH" ]; then
    cp "$AOF_PATH" "$TMP_DIR/appendonly.aof"
    gzip -c "$TMP_DIR/appendonly.aof" > "$BACKUP_ROOT/valkey/appendonly-$DATE.aof.gz"
    VALKEY_AOF_SIZE=$(stat -c%s "$BACKUP_ROOT/valkey/appendonly-$DATE.aof.gz")
    VALKEY_AOF_CHECKSUM=$(sha256sum "$BACKUP_ROOT/valkey/appendonly-$DATE.aof.gz" | cut -d' ' -f1)
    log "Valkey AOF backup OK: $VALKEY_AOF_SIZE bytes"
else
    VALKEY_AOF_SIZE=0
    VALKEY_AOF_CHECKSUM=""
fi

END_VALKEY=$(date +%s)
VALKEY_DURATION=$((END_VALKEY - START_VALKEY))

# Métrica combinada Valkey (OK si RDB OK)
VALKEY_STATUS=$VALKEY_RDB_STATUS
VALKEY_SIZE=$((VALKEY_RDB_SIZE + VALKEY_AOF_SIZE))
write_metrics "valkey" "$VALKEY_STATUS" "$VALKEY_DURATION" "$VALKEY_SIZE"

# ─── 3. Metadata JSON ───
log "Writing metadata..."
cat > "$BACKUP_ROOT/metadata/backup-$DATE.json" <<EOF
{
  "timestamp": "$TIMESTAMP_ISO",
  "targets": ["surreal", "valkey"],
  "sizes": {
    "surreal": $SURREAL_SIZE,
    "valkey_rdb": ${VALKEY_RDB_SIZE:-0},
    "valkey_aof": ${VALKEY_AOF_SIZE:-0}
  },
  "checksums": {
    "surreal": "$SURREAL_CHECKSUM",
    "valkey_rdb": "${VALKEY_RDB_CHECKSUM:-}",
    "valkey_aof": "${VALKEY_AOF_CHECKSUM:-}"
  },
  "duration_seconds": {
    "surreal": $SURREAL_DURATION,
    "valkey": $VALKEY_DURATION
  },
  "status": {
    "surreal": $SURREAL_STATUS,
    "valkey": $VALKEY_STATUS
  }
}
EOF

# ─── 4. Retención (30 días) ───
log "Applying retention policy (30 days)..."
find "$BACKUP_ROOT/surreal" -name "dmart-*.db.gz" -mtime +30 -delete 2>/dev/null || true
find "$BACKUP_ROOT/valkey" -name "*.gz" -mtime +30 -delete 2>/dev/null || true
find "$BACKUP_ROOT/metadata" -name "backup-*.json" -mtime +30 -delete 2>/dev/null || true

# ─── 5. Resumen final ───
log "Backup completed:"
log "  SurrealKV: status=$SURREAL_STATUS, size=$SURREAL_SIZE, duration=${SURREAL_DURATION}s"
log "  Valkey:    status=$VALKEY_STATUS, size=$VALKEY_SIZE, duration=${VALKEY_DURATION}s"

# Exit code: 0 si ambos OK, 1 si alguno falló
if [ $SURREAL_STATUS -eq 1 ] && [ $VALKEY_STATUS -eq 1 ]; then
    exit 0
else
    log "ERROR: One or more targets failed"
    exit 1
fi