# Legal & Regulatory — dMart UCI

Marco legal cubano aplicable a dMart UCI. Este documento es de referencia para compliance y se actualiza cuando hay cambios regulatorios.

## Regulaciones Aplicables

### Protección de Datos Personales

- **Ley 118 de 2021** — Protección de datos personales. Establece principios de tratamiento de datos personales, incluyendo consentimiento, minimización, y derechos de los titulares.
- **Ley 81 del Medio Ambiente** — No aplica directamente al software, pero relevante para infraestructura (residuos electrónicos).
- **Art. 127 de la Ley de Telecomunicaciones (2021)** — Regulación de servicios de telecomunicaciones y protección de datos en transmisión.

### Protección de Datos de Salud

- **Normativa de Salud Digital cubana** (pendiente de regulación formal) — dMart opera bajo el marco de protección de datos de salud como PHI (Protected Health Information) bajo estándares internacionales (HIPAA como referencia).
- La clasificación de datos de salud como "sensibles" implica: consentimiento explícito, encriptación obligatoria, y auditoría de accesos.

### Cumplimiento Internacional (referencia)

- **HIPAA (45 CFR 164)** — Se usa como estándar de referencia para controles de seguridad de datos de salud, aunque no es directamente exigible en Cuba. Se mapea para contratos internacionales y mejores prácticas.
- **ISO 27001:2022** —Framework de gestión de seguridad de información adoptado como referencia.
- **NIST 800-53** — Controles de seguridad relevantes mapeados en `CONTROL_CATALOG.md`.

## Estado Actual

| Regulación | Estado | Notas |
|------------|--------|-------|
| Ley 118/2021 (datos personales) | ✅ Cumplimiento parcial | Política de privacidad publicada |
| Ley de Telecomunicaciones (art. 127) | ✅ Cumplimiento | Tratamiento seguro de datos en tránsito |
| Salud Digital cubana | 🟡 Regulatory watch | Sin regulación formal aún |
| HIPAA (referencia) | ✅ Mapeado | Controls en CONTROL_CATALOG.md |
| ISO 27001 (referencia) | ✅ Mapeado | Controls en CONTROL_CATALOG.md |

## Acciones Requeridas

1. Revisar nuevos decretos o resoluciones sobre salud digital en Cuba
2. Actualizar privacy policy cuando cambien leyes de datos personales
3. Mantener BAA template actualizado para integradores externos
4. Documentar cualquier transferencia internacional de datos

## Responsable

- Compliance Officer / Administrador hospitalario
- Actualización: trimestral o cuando haya cambios regulatorios
