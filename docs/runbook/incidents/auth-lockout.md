# Incidente Lockout MFA / auth (SPEC-004)

## Síntoma
Cierre de sesiones o fallo MFA en masa; `mfa.verification` rechaza; usuarios bloqueados.

## Comando
```bash
# desbloqueo controlado SOLO con token DR del cofre (SPEC-004)
dmart_ops auth unlock --scope oncall --ttl 30 --reason dr-runbook
curl -s -H "Authorization: Bearer $(dmart_ops auth token)" localhost:3030/obs/health
```

## Verificar
- RBAC (SPEC-004) reconoce el rol `oncall` y revoca a los 30 min.
- Audit trail del desbloqueo con `fingerprint` SPEC-029.

## Escalar
- Bloqueo persiste → auditoría de SPEC-004 + restore de cofre (SPEC-010).
