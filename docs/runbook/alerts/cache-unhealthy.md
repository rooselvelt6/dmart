# Runbook: Cache Disconnected / Unhealthy

## Alert: CacheDisconnected / CacheUnhealthy
**Severity:** Warning/Critical | **Component:** cache

### Summary
Valkey cache disconnected or unhealthy after multiple reconnection attempts.

### Diagnosis
```bash
# Check cache connection status
curl -s http://localhost:8080/obs/metrics | grep cache_connected

# Check reconnection attempts
curl -s http://localhost:8080/obs/metrics | grep cache_reconnect_attempts

# Check Valkey process
systemctl status valkey

# Check Valkey logs
journalctl -u valkey -f

# Test connectivity
redis-cli -h localhost -p 6379 ping

# Check Valkey memory
redis-cli -h localhost -p 6379 info memory
```

### Resolution

#### CacheDisconnected (Warning - 2m disconnected)
1. Check Valkey service:
   ```bash
   systemctl status valkey
   ```

2. If stopped, restart:
   ```bash
   systemctl restart valkey
   ```

3. Check connectivity after restart:
   ```bash
   redis-cli -h localhost -p 6379 ping
   ```

#### CacheUnhealthy (Critical - failed reconnection after 3 attempts)
1. Check Valkey process:
   ```bash
   systemctl status valkey
   ```

2. Check Valkey logs for errors:
   ```bash
   journalctl -u valkey -n 100
   ```

3. Check disk space (Valkey needs space for AOF/RDB):
   ```bash
   df -h /var/lib/valkey
   df -h /app/data
   ```

4. Check memory:
   ```bash
   free -h
   redis-cli -h localhost -p 6379 info memory
   ```

5. If Valkey process dead, restart:
   ```bash
   systemctl restart valkey
   ```

6. If persistent issues, check disk health:
   ```bash
   smartctl -a /dev/sda
   ```

7. If disk full, clean up:
   ```bash
   # Check Valkey data size
   du -sh /var/lib/valkey
   
   # Consider increasing maxmemory or cleaning old keys
   redis-cli -h localhost -p 6379 config set maxmemory 500mb
   ```

### Verification
```bash
# Verify cache connection restored
curl -s http://localhost:8080/obs/metrics | grep cache_connected
# Should return: cache_connected 1

# Test actual connectivity
redis-cli -h localhost -p 6379 ping
# Should return: PONG
```

### Escalation
If Valkey cannot be recovered:
1. Check if application works without cache (degraded mode)
2. Consider failing over to backup Valkey instance
3. If data loss acceptable, flush and restart