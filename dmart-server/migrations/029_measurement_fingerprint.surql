-- SPEC-029: Fingerprint y versionado del cálculo de scores (auditabilidad).
-- Añade a `measurements` los campos auditable `algorithm_version` (semver del
-- algoritmo que calculó el score) y `fingerprint` (SHA-256 hex de 64 chars de
-- los inputs normalizados). No hay backfill: los registros históricos quedan
-- con `algorithm_version` ausente (parseado como '<legacy>') y `fingerprint`
-- vacío, y el endpoint de auditoría los marca "sin fingerprint".

DEFINE FIELD IF NOT EXISTS algorithm_version ON TABLE measurements TYPE string;
DEFINE FIELD IF NOT EXISTS fingerprint ON TABLE measurements TYPE string;