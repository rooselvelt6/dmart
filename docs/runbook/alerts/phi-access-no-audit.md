# Runbook: PHI Access Without Audit

## Alert: PhiAccessWithoutAudit
**Severity:** Critical | **Service:** dmart

### Summary
Detected data access/modification without corresponding audit log entry.

### Diagnosis
```bash
# Check audit logs for missing entries
curl -s http://localhost:8080/obs/metrics | grep audit_events_total

# Check recent audit log failures
journalctl -u dmart-server -f | grep -i "audit.*fail\|audit.*error"

# Check recent audit logs
curl -s "http://localhost:8080/api/admin/audit?limit=100" | jq '.[] | select(.result=="failure")'
```

### Resolution
1. Check audit service health:
   ```bash
   # Verify audit service is running
   curl -s http://localhost:8080/obs/health | jq '.audit'
   ```

2. Check for code paths bypassing audit:
   ```bash
   # Look for recent code changes
   git log --oneline -20
   
   # Check for direct DB access bypassing audit
   grep -r "db\." src/ | grep -v "audit"
   ```

2. Verify audit middleware is active:
   ```bash
   # Check middleware chain
   grep -r "audit" src/middleware/
   ```

3. If recent deployment, consider rollback:
   ```bash
   # Check recent deployments
   git log --oneline -10
   
   # Rollback if needed
   git revert HEAD
   cargo build --release
   systemctl restart dmart-server
   ```

### Verification
```bash
# Verify audit events are being logged
curl -s "http://localhost:8080/api/admin/audit?limit=10" | jq '.'
curl -s http://localhost:8080/obs/metrics | grep audit_events_total
```

### Escalation
If audit is completely down, this is a compliance emergency. Immediate escalation to security team.