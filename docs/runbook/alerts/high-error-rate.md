# Runbook: High 5xx Error Rate

## Alert: DMartHighErrorRate
**Severity:** Critical | **Service:** dmart

### Summary
5xx error ratio on mutations exceeded 1% over 5 minutes.

### Diagnosis
```bash
# Check recent error logs
journalctl -u dmart-server -f | grep -i "error\|5xx\|500"

# Check metrics for error breakdown
curl -s http://localhost:8080/obs/metrics | grep http_requests_errors_total

# Check SurrealDB connectivity
curl -s http://localhost:8080/obs/health
```

### Resolution
1. Check SurrealDB connection:
   ```bash
   # Test DB connection
   curl -s http://localhost:8080/obs/health | jq '.database'
   ```

2. Check for schema issues:
   ```bash
   # Check recent migrations
   journalctl -u dmart-server -n 100 | grep -i "migration\|schema"
   ```

3. Check for resource exhaustion:
   ```bash
   free -h
   df -h
   ```

4. If SurrealDB issues, restart DB:
   ```bash
   systemctl restart surrealdb
   systemctl restart dmart-server
   ```

### Verification
```bash
# Check error rate drops
curl -s http://localhost:8080/obs/metrics | grep http_requests_errors_total
```

### Escalation
If errors persist, check SurrealDB logs and consider rolling back recent deployments.