# Runbook: High Latency (P95 > 2s)

## Alert: DMartHighLatencyP95
**Severity:** Warning | **Service:** dmart

### Summary
P95 request latency exceeded 2 seconds for 10 minutes.

### Diagnosis
```bash
# Check latency metrics
curl -s http://localhost:8080/obs/metrics | grep http_request_duration_seconds

# Check slow queries
journalctl -u dmart-server -f | grep -i "slow\|latency\|timeout"

# Check SurrealDB performance
curl -s http://localhost:8080/obs/metrics | grep surreal_query_duration_seconds

# Check system resources
htop
free -h
iostat -x 1 3
```

### Resolution
1. Check for slow queries:
   ```bash
   # Look for slow query warnings in logs
   journalctl -u dmart-server -n 200 | grep -i "slow"
   ```

2. Check database indexes:
   ```bash
   # Check if indexes are missing
   journalctl -u dmart-server | grep -i "index\|full.*scan"
   ```

3. Check for lock contention:
   ```bash
   # Check SurrealDB stats
   curl -s http://localhost:8080/obs/metrics | grep surreal
   ```

4. If high CPU:
   ```bash
   # Check for runaway processes
   top -b -n 1 | head -20
   ```

5. Consider adding read replicas or scaling.

### Verification
```bash
# Verify P95 drops below 2s
curl -s http://localhost:8080/obs/metrics | grep http_request_duration_seconds
```

### Escalation
If latency persists, investigate query plans and consider query optimization.