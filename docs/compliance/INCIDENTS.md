# Incident Register — dMart UCI

Registro de incidentes de seguridad y disponibilidad. Cada incidente se correlaciona con el runbook de respuesta a incidentes SPEC-011.

## Plantilla de Registro de Incidente

### IRS-XXXX: [Título del incidente]

| Campo | Valor |
|-------|-------|
| **ID** | IRS-XXXX |
| **Fecha/Hora** | YYYY-MM-DD HH:MM:SS UTC |
| **Severidad** | P1 (Crítico) / P2 (Mayor) / P3 (Menor) / P4 (Info) |
| **Estado** | Abierto / En investigación / Resuelto / Cerrado |
| **Categoría** | Seguridad / Disponibilidad / Integridad de datos / Performance |

**Descripción:**
Descripción del incidente detectado.

**Impacto PHI:**
Sí/No — Si aplica, descripción del impacto a datos de salud.

**Timeline:**
| Hora | Evento |
|------|--------|
| HH:MM | Detección |
| HH:MM | Investigación iniciada |
| HH:MM | Contención |
| HH:MM | Remediación |
| HH:MM | Cierre |

**Runbook utilizado:**
SPEC-011 — Referencia al paso del runbook ejecutado.

**Root Cause:**
Causa raíz identificada.

**Remediación:**
Acciones tomadas para resolver y prevenir recurrencia.

**Lessons Learned:**
Lecciones aprendidas y mejoras implementadas.

---

## Correlación con Runbook SPEC-011

Cada incidente debe mapearse a los pasos del runbook:

1. **Detección** → Alerta SPEC-008 o monitoreo manual
2. **Clasificación** → Severidad P1-P4 según impacto clínico
3. **Contención** → Acciones inmediatas (isolation, rollback, etc.)
4. **Erradicación** → Eliminar causa raíz
5. **Recuperación** → Restaurar servicio (DR SPEC-024 si aplica)
6. **Post-mortem** → Documentar en este archivo

## Historial de Incidentes

_Registrar incidentes aquí con formato IRS-XXXX_

- _No hay incidentes registrados actualmente_
