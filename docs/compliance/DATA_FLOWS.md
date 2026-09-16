# Data Flows — dMart UCI

Inventario de flujos de datos clínicos y operacionales para compliance HIPAA/NIST/ISO 27001 (SPEC-034).

## Flujos de Datos

### 1. HL7/MLLP Ingest

| Campo | Descripción |
|-------|-------------|
| **Fuentes** | Monitores ICU (bedside), HL7v2 interfaces, MLLP gateways hospitalarios |
| **Destinos** | dmart-server (ingest pipeline), SurrealDB |
| **Formato** | HL7v2.x (ADT, ORU, MDM), MLLP framing |
| **Encriptación** | TLS 1.2+ en tránsito (MLLP over TLS), PHI encryptado en SurrealDB |
| **Retención** | Raw 30 días (downsampling después), processed indefinida (clinical record) |
| **Clasificación** | PHI / Clinical Data |

### 2. REST API

| Campo | Descripción |
|-------|-------------|
| **Fuentes** | Frontend Leptos (WASM), integraciones externas autenticadas |
| **Destinos** | dmart-server, SurrealDB |
| **Formato** | JSON over HTTPS (HTTP/1.1, HTTP/2) |
| **Encriptación** | TLS 1.3 obligatorio, JWT autenticación |
| **Retención** | Solicitud-response (sin retención en tránsito), logs 90 días |
| **Clasificación** | PHI / Clinical Data / Operational |

### 3. WebSocket/SSE Realtime

| Campo | Descripción |
|-------|-------------|
| **Fuentes** | Frontend Leptos (WASM), dashboards en tiempo real |
| **Destinos** | dmart-server (SSE/WebSocket handler) |
| **Formato** | JSON events over WSS (WebSocket Secure) |
| **Encriptación** | TLS 1.3 (WSS), autenticación via JWT en upgrade |
| **Retención** | Conexión activa (sin persistencia de eventos en tránsito) |
| **Clasificación** | Clinical Data (real-time vitals, scores) |

### 4. Backups

| Campo | Descripción |
|-------|-------------|
| **Fuentes** | SurrealDB snapshots, dmart-server backup scripts |
| **Destinos** | S3/MinIO (local hospitalario), directorio local /backups/dr |
| **Formato** | tar.zst (compresión zstd), manifests JSON |
| **Encriptación** | AES-256-GCM en-at-rest (S3 SSE-KMS), TLS en tránsito |
| **Retención** | 7 días (incremental), 30 días (full), conforme SPEC-024 |
| **Clasificación** | PHI / Clinical Data |

### 5. Tele-ICU Live Session

| Campo | Descripción |
|-------|-------------|
| **Fuentes** | Monitores ICU, WebRTC streams, datafeeds en tiempo real |
| **Destinos** | dmart-server (tele-ICU handler), frontend Leptos |
| **Formato** | WebRTC (audio/video), JSON (data), binary (waveforms) |
| **Encriptación** | DTLS-SRTP (WebRTC), TLS 1.3 (data channels) |
| **Retención** | Sessión en vivo (no persistente), metadata 90 días |
| **Clasificación** | PHI / Clinical Data |

## Flujos de Auditoría

| Flujo | Descripción | Retención |
|-------|-------------|-----------|
| Audit logs | Cambios a datos clínicos (quién/cuándo) | 7 años |
| Access logs | Autenticación/autorización | 90 días |
| Backup manifests | Metadatos de cada backup | Indefinida |

## Diagrama Simplificado

```
[Monitores ICU] --HL7/MLLP/TLS--> [dmart-server] --JWT/TLS--> [SurrealDB]
                                         |                           |
                                    [Backups] --zstd/AES--> [S3/MinIO]
                                         |
                                    [Frontend] --WSS--> [Realtime events]
                                         |
                                    [Tele-ICU] --WebRTC/DTLS--> [Streams]
```

## Notas de Compliance

- Todos los flujos con PHI usan encriptación en tránsito (TLS 1.2+)
- PHI en reposo siempre encriptado (AES-256)
- Retención de datos clínicos: indefinida (clinical record)
- Retención de audit logs: 7 años (HIPAA requirement)
- Network segmentation: tráfico clínico en VLAN dedicada (infraestructura hospitalaria)
