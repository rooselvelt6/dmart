# Runbook: WORM Chain Integrity Failure

## Alert: WormChainFailure
**Severity:** Critical | **Service:** dmart

### Summary
The WORM audit chain integrity verification failed - hash chain broken or tampered.

### Diagnosis
```bash
# Check audit chain verification
curl -s -X POST http://localhost:8080/api/admin/audit/verify | jq '.'

# Check audit chain status in metrics
curl -s http://localhost:8080/obs/metrics | grep audit_chain_integrity_ok

# Check audit logs for chain verification errors
journalctl -u dmart-server -f | grep -i "chain.*integrity\|audit.*chain\|worm"

# Check audit chain head
curl -s "http://localhost:8080/api/admin/audit/export" | head -20
```

### Resolution
1. **Immediate - Do not write to audit log until investigated**

2. Check the chain verification details:
   ```bash
   # Get detailed verification report
   curl -s -X POST http://localhost:8080/api/admin/audit/verify | jq '.'
   
   # Check which batch/record failed
   # Look for: logs_total, logs_valid, logs_invalid, batches_total
   ```

3. Check for recent tampering or corruption:
   ```bash
   # Check audit log file integrity
   # The chain is stored in SurrealDB - check for disk corruption
   journalctl -u dmart-server -n 100 | grep -i "audit\|chain\|hash\|worm"
   
   # Check disk health
   smartctl -a /dev/sda
   fsck -n /app/data
   ```

4. If chain is broken due to bug (not tampering):
   ```bash
   # Check recent code changes to audit
   git log --oneline -20 -- src/audit/
   
   # Check for schema changes
   git log --oneline -20 -- migrations/
   
   # If recent deployment, consider rollback
   # git revert HEAD
   # systemctl restart dmart-server
   ```

5. **If tampering suspected - SECURITY INCIDENT**
   ```bash
   # Preserve evidence
   # Do NOT restart the server
   # Contact security team immediately
   # Preserve disk image if possible
   ```

### Verification
```bash
# Verify chain integrity restored
curl -s -X POST http://localhost:8080/api/admin/audit/verify | jq '.ok'

# Check metrics
curl -s http://localhost:8080/obs/metrics | grep audit_chain_integrity_ok
# Should return: audit_chain_integrity_ok 1
```

### Escalation
**This is a critical security incident.** If tampering is confirmed:
1. Do NOT restart the server (preserves evidence)
2. Contact security team immediately
3. Preserve disk image
4. Notify compliance/Legal if patient data affected
5. Consider the chain broken - all subsequent logs are suspect