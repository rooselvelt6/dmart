# Runbook: dMart Server Down

## Alert: DMartServerDown
**Severity:** Critical | **Service:** dmart

### Summary
Prometheus cannot scrape the dMart server for more than 2 minutes.

### Diagnosis
```bash
# Check if the server process is running
systemctl status dmart-server

# Check the logs
journalctl -u dmart-server -f

# Check if the port is listening
ss -tlnp | grep 8080

# Check if the process exists
ps aux | grep dmart-server
```

### Resolution
1. If process not running:
   ```bash
   systemctl start dmart-server
   ```

2. If process running but not responding:
   ```bash
   systemctl restart dmart-server
   ```

3. Check disk space:
   ```bash
   df -h /app/data
   ```

4. Check memory:
   ```bash
   free -h
   ```

### Verification
```bash
curl -f http://localhost:8080/obs/health
curl -f http://localhost:8080/obs/metrics
```

### Escalation
If restart fails, check SurrealDB connectivity and disk space.