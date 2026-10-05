# dMart UCI - Native Deployment Guide

This guide covers deploying dMart UCI on a Linux server using systemd and Caddy/Nginx as reverse proxy.

## Prerequisites

- Linux server (Ubuntu 22.04+, Debian 12+, RHEL 9+)
- SurrealDB 2.x (separate service)
- Valkey/Redis 7+ (for cache, sessions, rate limiting)
- Rust 2024+ (for building)
- 2GB+ RAM, 10GB+ disk

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Caddy/    │────▶│  dmart-     │────▶│  SurrealDB  │
│   Nginx     │     │  server     │     │  (port 8000)│
└─────────────┘     └─────────────┘     └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │   Valkey    │
                    │  (port 6379)│
                    └─────────────┘
```

## 1. System Preparation

### Create service user
```bash
useradd -r -s /bin/false -d /opt/dmart dmart
```

### Create directories
```bash
mkdir -p /opt/dmart/{bin,static,logs}
mkdir -p /var/log/dmart
mkdir -p /var/lib/dmart
chown -R dmart:dmart /opt/dmart /var/log/dmart /var/lib/dmart
```

## 2. Build dMart Server

### Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup default stable
rustup target add wasm32-unknown-unknown
```

### Build
```bash
cd /path/to/dmart
cargo build --release -p dmart-server
cargo build --release -p dmart-app --target wasm32-unknown-unknown
```

### Install binaries
```bash
cp target/release/dmart-server /opt/dmart/bin/
cp -r target/wasm32-unknown-unknown/release/* /opt/dmart/static/
chown -R dmart:dmart /opt/dmart
```

## 3. SurrealDB Setup

### Install SurrealDB
```bash
curl -sSf https://install.surrealdb.com | sh
```

### Create systemd service
```bash
cat > /etc/systemd/system/surrealdb.service << 'EOF'
[Unit]
Description=SurrealDB
After=network.target

[Service]
Type=simple
User=surrealdb
Group=surrealdb
ExecStart=/usr/local/bin/surreal start --user root --pass root --bind 0.0.0.0:8000 file:///var/lib/surrealdb/data
Restart=always
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
```

```bash
useradd -r -s /bin/false surrealdb
mkdir -p /var/lib/surrealdb/data
chown -R surrealdb:surrealdb /var/lib/surrealdb
systemctl daemon-reload
systemctl enable --now surrealdb
```

## 4. Valkey Setup

### Install Valkey
```bash
# Ubuntu/Debian
apt update && apt install -y valkey

# Or from source
# git clone https://github.com/valkey-io/valkey.git
# cd valkey && make && make install
```

### Configure Valkey
```bash
cat > /etc/valkey/valkey.conf << 'EOF'
bind 127.0.0.1
port 6379
maxmemory 500mb
maxmemory-policy allkeys-lru
save 900 1
save 300 10
save 60 10000
appendonly yes
appendfsync everysec
EOF
```

```bash
systemctl enable --now valkey
```

## 5. dMart Server Service

### Install systemd service
```bash
cp deploy/systemd/dmart-server.service /etc/systemd/system/
```

### Create environment file
```bash
cat > /etc/dmart/dmart.env << 'EOF'
# Database
DMART_SURREAL_URL=ws://127.0.0.1:8000/rpc
DMART_SURREAL_USER=root
DMART_SURREAL_PASS=root
DMART_SURREAL_NS=dmart
DMART_SURREAL_DB=uci

# Cache
DMART_VALKEY_URL=redis://127.0.0.1:6379

# Master key (generate with: openssl rand -hex 32)
DMART_MASTER_KEY=your-32-byte-hex-key-here

# JWT
DMART_JWT_SECRET=your-jwt-secret-here
DMART_JWT_EXPIRY=3600

# Server
DMART_BIND_ADDR=127.0.0.1:8080
DMART_WORKERS=4
RUST_LOG=info

# Feature flags
METRICS_EXTENDED=true
DMART_ENV=production
EOF
```

```bash
chmod 600 /etc/dmart/dmart.env
chown dmart:dmart /etc/dmart/dmart.env
```

### Enable and start
```bash
systemctl daemon-reload
systemctl enable --now dmart-server
```

## 6. Reverse Proxy

### Option A: Caddy (Recommended)

```bash
# Install Caddy
apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list
apt update && apt install caddy
```

```bash
cp deploy/caddy/Caddyfile /etc/caddy/Caddyfile
# Edit domain in Caddyfile
systemctl enable --now caddy
```

### Option B: Nginx

```bash
apt install -y nginx certbot python3-certbot-nginx
cp deploy/nginx/dmart.conf /etc/nginx/sites-available/
ln -s /etc/nginx/sites-available/dmart.conf /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx

# SSL certificate
certbot --nginx -d dmart.example.com
```

## 7. Static Files

```bash
# Create static directory structure
mkdir -p /opt/dmart/static/{assets,wasm,errors}
cp -r target/wasm32-unknown-unknown/release/* /opt/dmart/static/
cp -r dmart-app/static/* /opt/dmart/static/ 2>/dev/null || true

# Error pages
cat > /opt/dmart/static/errors/404.html << 'EOF'
<!DOCTYPE html><html><head><title>404 - Not Found</title></head>
<body><h1>404 - Page Not Found</h1><a href="/">Go Home</a></body></html>
EOF

cat > /opt/dmart/static/errors/50x.html << 'EOF'
<!DOCTYPE html><html><head><title>Server Error</title></head>
<body><h1>Server Error</h1><p>Please try again later.</p></body></html>
EOF

chown -R dmart:dmart /opt/dmart/static
```

## 8. SSL Certificates (Let's Encrypt)

### Caddy (automatic)
Caddy handles this automatically via ACME.

### Nginx (manual)
```bash
certbot --nginx -d dmart.example.com
# Auto-renewal
systemctl enable certbot.timer
```

## 9. Verification

### Health checks
```bash
# Server health
curl -f http://localhost:8080/obs/health

# Reverse proxy
curl -f https://dmart.example.com/obs/health

# Metrics
curl -f https://dmart.example.com/obs/metrics

# Database
curl -f http://localhost:8080/obs/health | jq '.database'
```

### Logs
```bash
# Server logs
journalctl -u dmart-server -f

# Reverse proxy logs
journalctl -u caddy -f
# or
journalctl -u nginx -f

# SurrealDB
journalctl -u surrealdb -f

# Valkey
journalctl -u valkey -f
```

## 9. Backup & Recovery

### Automated backup script
```bash
cat > /opt/dmart/bin/backup.sh << 'EOF'
#!/bin/bash
set -euo pipefail

BACKUP_DIR="/opt/dmart/backups/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

# SurrealDB backup
surreal export --conn ws://127.0.0.1:8000/rpc --user root --pass root --ns dmart --db uci "$BACKUP_DIR/surrealdb.sql"

# Compress
tar -czf "$BACKUP_DIR.tar.gz" -C "$BACKUP_DIR" .
rm -rf "$BACKUP_DIR"

# Retain last 7 days
find /opt/dmart/backups -name "*.tar.gz" -mtime +7 -delete
EOF

chmod +x /opt/dmart/bin/backup.sh

# Cron job (daily at 2 AM)
echo "0 2 * * * dmart /opt/dmart/bin/backup.sh" > /etc/cron.d/dmart-backup
```

## 10. Monitoring

### Prometheus
```bash
# Add to prometheus.yml
scrape_configs:
  - job_name: dmart-server
    metrics_path: /obs/metrics
    static_configs:
      - targets: ['dmart.example.com']
```

### Grafana Dashboards
Import dashboards for:
- HTTP metrics (latency, errors, throughput)
- Business metrics (patients, measurements, scales)
- System metrics (CPU, memory, disk)
- Cache metrics (hit rate, connections)
- HL7 ingest metrics

## 10. Troubleshooting

### Server won't start
```bash
# Check logs
journalctl -u dmart-server -n 100

# Check environment
cat /etc/dmart/dmart.env

# Test config
/opt/dmart/bin/dmart-server --help
```

### Database connection issues
```bash
# Test SurrealDB
curl -X POST http://127.0.0.1:8000/rpc \
  -H "Content-Type: application/json" \
  -d '{"method":"signin","params":[{"user":"root","pass":"root"}]}'
```

### Cache issues
```bash
redis-cli -h localhost -p 6379 ping
redis-cli -h localhost -p 6379 info memory
```

### High memory/CPU
```bash
# Check for memory leaks
curl http://localhost:8080/obs/metrics | grep process_resident_memory_bytes

# Check for CPU
top -p $(pgrep dmart-server)
```

## 11. Rollback Procedure

```bash
# 1. Stop service
systemctl stop dmart-server

# 2. Restore previous binary
mv /opt/dmart/bin/dmart-server /opt/dmart/bin/dmart-server.new
mv /opt/dmart/bin/dmart-server.prev /opt/dmart/bin/dmart-server

# 3. Restore static files if needed
rm -rf /opt/dmart/static
mv /opt/dmart/static.prev /opt/dmart/static

# 4. Start service
systemctl start dmart-server

# 5. Verify
curl -f https://dmart.example.com/obs/health
```

## 12. Security Checklist

- [ ] DMART_MASTER_KEY is 32 bytes hex, generated with `openssl rand -hex 32`
- [ ] DMART_JWT_SECRET is strong random string
- [ ] Firewall: only 80, 443, 22 (SSH) open
- [ ] SurrealDB only bound to localhost
- [ ] Valkey only bound to localhost
- [ ] SSH key-only authentication
- [ ] Automatic security updates enabled
- [ ] Backup tested and verified
- [ ] Monitoring alerts configured
- [ ] Runbooks accessible to on-call team

## Support

- Issues: https://github.com/dmart/dmart/issues
- Runbooks: https://wiki.dmart.io/runbooks
- Security: security@dmart.io