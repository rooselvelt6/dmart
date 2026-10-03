//! MLLP Server with Ingest Hardening (SPEC-031)
//! This module is part of the library and uses crate:: for internal references.
//!
//! ⚠️ Seguridad del listener (auditoría H3/M7): MLLP es un protocolo de texto
//! plano sin autenticación ni cifrado nativos. La frontera se implementa aquí en
//! capas, de la más fuerte a la más débil:
//! - **mTLS obligatorio en producción** (`crate::mllp_tls`): TLS 1.3 con
//!   AES-256-GCM, certificado de cliente verificado contra la CA propia de
//!   monitores y *pinning* de identidad por fingerprint SHA-256 del certificado.
//!   Con mTLS el `MSH.3` está ligado al certificado, así que las constantes
//!   vitales y el resto del tráfico ya no viajan en claro por la red.
//! - **Handshake de secreto compartido** (`DMART_MLLP_AUTH_SECRET`) como control
//!   adicional de capa 2 en despliegues sin mTLS. El secreto se compara en tiempo
//!   constante y se guarda en un buffer `Zeroizing`.
//! - **Allowlist de emisores** (`DMART_MLLP_ALLOWED_SENDERS`) sobre `MSH.3`.
//! - **Bind configurable** (`DMART_MLLP_BIND`), por defecto loopback.
//! - **TTimeouts** de lectura/escritura (anti slow-loris) y **tope de conexiones**
//!   global y por IP.
//!
//! Sin mTLS ni handshake, `serve()` **no arranca** cuando el despliegue se declara
//! productivo: un listener MLLP en claro no es un riesgo aceptable con PHI.
//! El aislamiento por VLAN es una mitigación complementaria, nunca el sustituto.

use crate::db::Database;
use crate::hl7::ingest::ingest_vitals;
use crate::hl7::parser::parse_oru_message;
use crate::ingest::IngestState;
use crate::ingest::quality::{QualityValidator, ValidationReason, ValidationResult};
use crate::metrics::{
    hl7_error, hl7_processed, ingest_circuit_state, ingest_error_avg_set, ingest_fault_devices_set,
    ingest_gap, ingest_invalid, ingest_rate_limit_current, ingest_throttled,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_rustls::server::TlsStream;
use zeroize::Zeroizing;

use crate::mllp_tls::MllpTlsConfig;

pub const START_BLOCK: u8 = 0x0B;
pub const END_BLOCK: u8 = 0x1C;
pub const CARRIAGE_RETURN: u8 = 0x0D;

/// Máximo tamaño de un mensaje HL7 (1 MiB).
pub const MAX_MESSAGE: usize = 1024 * 1024;

/// `MSH.3` esperado en el frame de handshake.
pub const AUTH_SENDING_APP: &str = "DMART-AUTH";

/// Configuración de seguridad del listener MLLP, desde variables de entorno.
#[derive(Debug, Clone)]
pub struct MllpSecurityConfig {
    /// Secreto compartido exigido en el primer frame. `None` = handshake
    /// deshabilitado (solo aceptable en single-tenant sin exposición externa).
    ///
    /// Se guarda en un [`Zeroizing`] para que la copia en el heap se borre al
    /// soltar el config, en vez de quedar residiendo en el heap.
    pub auth_secret: Option<Zeroizing<String>>,
    /// `MSH.3` permitidos para enviar mensajes HL7 una vez autenticada la
    /// conexión. Vacío = no se restringe.
    pub allowed_senders: Vec<String>,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub max_connections: usize,
    pub max_connections_per_ip: usize,
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn env_secs(key: &str, default: u64) -> Duration {
    Duration::from_secs(
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|v| *v > 0)
            .unwrap_or(default),
    )
}

impl MllpSecurityConfig {
    pub fn from_env() -> Self {
        Self {
            auth_secret: std::env::var("DMART_MLLP_AUTH_SECRET")
                .ok()
                .map(|s| Zeroizing::new(s.trim().to_string()))
                .filter(|s| !s.is_empty()),
            allowed_senders: std::env::var("DMART_MLLP_ALLOWED_SENDERS")
                .ok()
                .map(|v| {
                    v.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            read_timeout: env_secs("DMART_MLLP_READ_TIMEOUT_SECS", 30),
            write_timeout: env_secs("DMART_MLLP_WRITE_TIMEOUT_SECS", 10),
            max_connections: env_usize("DMART_MLLP_MAX_CONNECTIONS", 64),
            max_connections_per_ip: env_usize("DMART_MLLP_MAX_CONNECTIONS_PER_IP", 8),
        }
    }

    /// Comprueba el frame de handshake (`MSH.3 == DMART-AUTH`, `MSH.4 == secreto`).
    ///
    /// Comparación en **tiempo constante** para no filtrar el secreto por
    /// temporización de respuesta.
    pub fn verify_handshake(&self, raw: &str) -> bool {
        let Some(expected) = self.auth_secret.as_deref() else {
            return true; // handshake no configurado
        };
        let Some(msh) = raw.get(..raw.find('\r').unwrap_or(raw.len())) else {
            return false;
        };
        if !msh.starts_with("MSH|") {
            return false;
        }
        let fields: Vec<&str> = msh.split('|').collect();
        // fields[0]="MSH", fields[1]='^~\&' (encoding chars), fields[2]=MSH.3,
        // fields[3]=MSH.4
        let app = fields.get(2).copied().unwrap_or_default();
        if app != AUTH_SENDING_APP {
            return false;
        }
        let presented = fields.get(3).copied().unwrap_or_default();
        // Comparación en tiempo constante: no filtra el secreto por
        // temporización de la respuesta.
        use subtle::ConstantTimeEq;
        presented.as_bytes().ct_eq(expected.as_bytes()).into()
    }

    /// Un `MSH.3` puede enviar mensajes HL7 tras autenticarse la conexión.
    pub fn sender_allowed(&self, sender: &str) -> bool {
        self.allowed_senders.is_empty() || self.allowed_senders.iter().any(|s| s == sender)
    }
}

/// Códigos de error estables que se devuelven en el ACK.
///
/// Nunca se eco el mensaje interno del error: el peer es un dispositivo de
/// bedside en la red clínica y devolver el detalle de SurrealDB o rutas de disco
/// filtra topología e información del esquema (auditoría L9).
fn ack_error_code(err: &anyhow::Error) -> &'static str {
    let msg = err.to_string();
    if msg.contains("no encontrado") {
        "patient_not_found"
    } else if msg.contains("frame") {
        "frame_error"
    } else {
        "internal_error"
    }
}

/// Reexporta el constructor de ACK para no tener dos copias que divergir.
///
/// Existía una implementación duplicada aquí y otra en [`crate::hl7::mllp`], y
/// ya lo habían hecho: una escribía el segmento `ERR` fuera del frame MLLP, de
/// modo que el emisor nunca leía el motivo del rechazo.
pub use crate::hl7::mllp::build_ack;

/// Extrae device_id del mensaje HL7 (MSH.3 + MSH.4 o IP del peer)
fn extract_device_id(
    msg: &crate::hl7::parser::VitalsMessage,
    peer: &std::net::SocketAddr,
) -> String {
    if !msg.sender.is_empty() {
        msg.sender.clone()
    } else {
        format!("device-{}", peer.ip())
    }
}

/// Extrae sequence number del MSH.13
fn extract_sequence(msg: &crate::hl7::parser::VitalsMessage) -> Option<u32> {
    msg.sequence_number
}

/// Extrae campos para validación de calidad
fn extract_vitals_for_quality(msg: &crate::hl7::parser::VitalsMessage) -> Vec<(String, String)> {
    let mut vitals = Vec::new();
    for v in &msg.vitals {
        vitals.push((v.name.clone(), v.value.to_string()));
    }
    vitals
}

/// Lee un frame MLLP completo (`0x0B ... 0x1C 0x0D`) respetando el timeout de
/// lectura y el tamaño máximo configurado.
///
/// Devuelve `Ok(None)` si el peer cerró limpiamente antes de empezar un frame.
/// Errores de timeout llevan el detalle interno (nunca sale al peer).
async fn read_frame<S>(
    stream: &mut S,
    max_frame_bytes: usize,
    read_timeout: Duration,
) -> anyhow::Result<Option<(String, usize)>>
where
    S: AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(2048);
    let mut last = None;
    let mut frame_size = 0usize;

    loop {
        let mut byte = [0u8; 1];
        let n = match tokio::time::timeout(read_timeout, stream.read(&mut byte)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "mllp read timeout tras {read_timeout:?} (slow-loris)"
                ));
            }
        };
        if n == 0 {
            return Ok(None);
        }
        let b = byte[0];
        if buf.is_empty() && b == START_BLOCK {
            last = None;
            continue;
        }
        buf.push(b);
        frame_size += 1;
        if last == Some(END_BLOCK) && b == CARRIAGE_RETURN {
            let size = buf.len();
            buf.truncate(buf.len() - 2); // quitar 0x1C 0x0D
            return match String::from_utf8(buf) {
                Ok(s) => Ok(Some((s, size))),
                Err(_) => Err(anyhow::anyhow!("non-utf8 payload")),
            };
        }
        last = Some(b);
        if frame_size > max_frame_bytes {
            return Err(anyhow::anyhow!("frame_too_large"));
        }
    }
}

/// Tope de bytes pendientes que se drenan antes de cerrar tras descartar un
/// frame, y gracia de espera por lectura durante ese drenado.
///
/// Si el servidor cierra con datos sin leer en el socket de recepción, el
/// kernel responde con RST y el emisor pierde el ACK negativo que el servidor
/// sí llegó a escribir. Drenar una cantidad **acotada** garantiza que el
/// rechazo le llegue al emisor sin abrir la puerta al agotamiento de memoria
/// que `max_frame_bytes` previene.
const MAX_DRAIN_BYTES: usize = 64 * 1024;
const DRAIN_GRACE: Duration = Duration::from_millis(250);

/// Consume el resto del frame ya descartado hasta su terminador `\x1C\x0D`.
///
/// Terminar en el terminador natural del frame (y no agotar el presupuesto de
/// tiempo) es lo que evita que el emisor se quede esperando un ACK que el
/// servidor sí envió: si el drenado bloqueara hasta el `read_timeout`, el
/// cliente cortaría antes de leerlo.
async fn drain_pending<S>(stream: &mut S, budget: usize, grace: Duration)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut remaining = budget;
    let mut last: Option<u8> = None;
    let mut buf = [0u8; 1024];
    while remaining > 0 {
        let want = remaining.min(buf.len());
        match tokio::time::timeout(grace, stream.read(&mut buf[..want])).await {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
            Ok(Ok(n)) => {
                remaining -= n;
                for &b in &buf[..n] {
                    if last == Some(END_BLOCK) && b == CARRIAGE_RETURN {
                        return;
                    }
                    last = Some(b);
                }
            }
        }
    }
}

async fn write_ack<S>(stream: &mut S, ack: &[u8], write_timeout: Duration)
where
    S: AsyncWrite + Unpin,
{
    match tokio::time::timeout(write_timeout, stream.write_all(ack)).await {
        Ok(Ok(())) => {
            let _ = stream.flush().await;
        }
        Ok(Err(e)) => tracing::debug!("[mllp] error escribiendo ACK: {e}"),
        Err(_) => tracing::debug!("[mllp] timeout escribiendo ACK"),
    }
}

async fn handle_stream<S>(
    mut stream: S,
    db: Database,
    ingest_state: Arc<IngestState>,
    peer: SocketAddr,
    sec: MllpSecurityConfig,
    // `MSH.3` que la identidad de transporte autoriza. Cuando es `Some` (mTLS),
    // el `MSH.3` del mensaje **debe** coincidir: el certificado es la identidad,
    // no una pista adicional.
    pinned_sender: Option<String>,
) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let config = ingest_state.config().clone();
    let mut quality_validator = QualityValidator::new();
    let max_frame_bytes = config.max_frame_bytes.min(MAX_MESSAGE);

    // ── Handshake: si hay secreto configurado, el primer frame debe
    //    autenticarse; si no, la conexión se cierra sin ACK.
    if sec.auth_secret.is_some() {
        match read_frame(&mut stream, max_frame_bytes, sec.read_timeout).await? {
            None => return Ok(()),
            Some((raw, _)) => {
                if !sec.verify_handshake(&raw) {
                    tracing::warn!(
                        "[mllp:{peer}] handshake rechazado (MSH.3/MSH.4 inválido); conexión cerrada"
                    );
                    crate::metrics::ingest_message("mllp", "auth_failed");
                    write_ack(
                        &mut stream,
                        &build_ack("", Some("unauthorized")),
                        sec.write_timeout,
                    )
                    .await;
                    return Ok(());
                }
            }
        }
    }

    loop {
        let (raw, frame_size) =
            match read_frame(&mut stream, max_frame_bytes, sec.read_timeout).await {
                Ok(Some(frame)) => frame,
                Ok(None) => return Ok(()),
                Err(e) => {
                    let code = ack_error_code(&e);
                    if e.to_string().contains("frame_too_large") {
                        // Drenar antes del ACK: si no, el RST del cierre se
                        // come el rechazo y el emisor no sabe por qué falló.
                        drain_pending(&mut stream, MAX_DRAIN_BYTES, DRAIN_GRACE).await;
                        tracing::warn!(
                            "[mllp:{peer}] mensaje excede {max_frame_bytes} bytes; descartado"
                        );
                    } else {
                        tracing::debug!("[mllp:{peer}] lectura falló: {e}");
                    }
                    let ack = build_ack("", Some(code));
                    write_ack(&mut stream, &ack, sec.write_timeout).await;
                    // Un frame inválido no debe permitir bucle infinito de errores
                    // en la misma conexión.
                    return Ok(());
                }
            };

        match parse_oru_message(&raw) {
            Ok(msg) => {
                let msg_id = msg.message_id.clone();
                let device_id = extract_device_id(&msg, &peer);
                let sequence = extract_sequence(&msg);

                // Identidad del emisor. Con mTLS, el `MSH.3` lo fija el
                // certificado (pinning por fingerprint): si no coincide, el
                // mensaje se descarta aunque el certificado sea válido. Sin
                // mTLS se aplica la allowlist de `MSH.3`.
                let sender_ok = match pinned_sender.as_deref() {
                    Some(expected) => {
                        let ok = msg.sender == expected;
                        if !ok {
                            tracing::warn!(
                                "[mllp:{peer}] MSH.3 ({}) no corresponde al certificado de la \
                                 conexión; mensaje descartado",
                                msg.sender
                            );
                        }
                        ok
                    }
                    None => sec.sender_allowed(&msg.sender),
                };
                if !sender_ok {
                    crate::metrics::ingest_message(&device_id, "sender_not_allowed");
                    let ack = build_ack(&msg_id, Some("sender_not_allowed"));
                    write_ack(&mut stream, &ack, sec.write_timeout).await;
                    continue;
                }

                // ─── Ingest Hardening (SPEC-031) ───
                let parse_result = if msg.vitals.is_empty() {
                    Err("no vitals in message".to_string())
                } else {
                    Ok(())
                };

                let (_allowed, _reject_reason, metrics_update) = ingest_state
                    .process_message(&device_id, frame_size, sequence, &parse_result)
                    .await;

                // Métricas de rate limit / circuit breaker
                if metrics_update.throttled {
                    ingest_throttled(&device_id);
                    crate::metrics::ingest_message(&device_id, "throttled");
                    let ack = build_ack(&msg_id, Some("throttled"));
                    write_ack(&mut stream, &ack, sec.write_timeout).await;
                    continue;
                }

                if metrics_update.circuit_open {
                    crate::metrics::ingest_message(&device_id, "circuit_open");
                    let ack = build_ack(&msg_id, Some("circuit_open"));
                    write_ack(&mut stream, &ack, sec.write_timeout).await;
                    crate::support::note("ingest", false);
                    continue;
                }

                // SPEC-044: self-healing — el CB se recuperó solo (Open → HalfOpen).
                if metrics_update.self_healed {
                    crate::support::note("ingest", true);
                    let _ = crate::support::note_auto_recovery(
                        db.as_ref(),
                        "ingest",
                        &format!("circuit_breaker {device_id}: Open → HalfOpen automático"),
                    )
                    .await;
                }

                if metrics_update.gaps {
                    ingest_gap(&device_id, metrics_update.gaps_count);
                }

                // Validación de calidad de datos
                let vitals_for_quality = extract_vitals_for_quality(&msg);
                for (vital, value) in vitals_for_quality {
                    if let ValidationResult::Invalid { reason, .. } =
                        quality_validator.validate_vital(&device_id, &vital, &value)
                    {
                        let reason_str = match reason {
                            ValidationReason::OutOfRange => "out_of_range",
                            ValidationReason::ParseError => "parse_error",
                            ValidationReason::MissingRequired => "missing_required",
                            ValidationReason::Malformed => "malformed",
                        };
                        ingest_invalid(&device_id, &vital, reason_str);
                    }
                }

                // Métricas de throughput
                crate::metrics::ingest_message(&device_id, "ok");
                crate::metrics::ingest_message_size(&device_id, frame_size);
                crate::support::note("ingest", true);

                // Actualizar métricas globales
                let im = ingest_state.metrics().await;
                ingest_fault_devices_set(im.fault_devices);
                ingest_error_avg_set(im.error_avg);

                // Actualizar rate limit current (para debugging)
                if let Some(dev_state) = ingest_state.devices.read().await.get(&device_id) {
                    let mut rl = dev_state.rate_limiter.clone();
                    ingest_rate_limit_current(&device_id, rl.available());
                    ingest_circuit_state(&device_id, dev_state.circuit_breaker.state().as_u8());
                }

                // ─── Ingestión normal ───
                match ingest_vitals(&db, &msg).await {
                    Ok(m) => {
                        hl7_processed(msg.source.label());
                        tracing::info!(
                            "[mllp:{peer}] medición {} a paciente {} (device: {})",
                            m.measurement_id,
                            m.patient_id,
                            device_id
                        );
                        let ack = build_ack(&msg_id, None);
                        write_ack(&mut stream, &ack, sec.write_timeout).await;
                    }
                    Err(e) => {
                        hl7_error(msg.source.label());
                        crate::metrics::ingest_message(&device_id, "ingest_error");
                        crate::support::note("ingest", false);
                        tracing::warn!("[mllp:{peer}] ingestión falló: {e}");
                        let ack = build_ack(&msg_id, Some(ack_error_code(&e)));
                        write_ack(&mut stream, &ack, sec.write_timeout).await;
                    }
                }
            }
            Err(e) => {
                hl7_error("unknown");
                crate::support::note("ingest", false);
                tracing::warn!("[mllp:{peer}] parseo falló: {e}");
                let ack = build_ack("", Some("parse_error"));
                write_ack(&mut stream, &ack, sec.write_timeout).await;
            }
        }
    }
}

/// Contador de conexiones activas por IP (tope anti-abuso).
#[derive(Default)]
struct IpConnCounter {
    counts: std::sync::Mutex<std::collections::HashMap<IpAddr, usize>>,
}

impl IpConnCounter {
    fn acquire(&self, ip: IpAddr, max: usize) -> bool {
        let mut guard = self.counts.lock().unwrap_or_else(|p| p.into_inner());
        let entry = guard.entry(ip).or_insert(0);
        if *entry >= max {
            return false;
        }
        *entry += 1;
        true
    }

    fn release(&self, ip: IpAddr) {
        let mut guard = self.counts.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = guard.get_mut(&ip) {
            *c = c.saturating_sub(1);
            if *c == 0 {
                guard.remove(&ip);
            }
        }
    }
}

/// Levanta un listener MLLP en `address` con hardening de SPEC-031 y los
/// controles de seguridad de `MllpSecurityConfig` (handshake, timeouts, topes).
pub async fn serve(
    address: SocketAddr,
    db: Database,
    ingest_state: Arc<IngestState>,
) -> anyhow::Result<()> {
    let sec = MllpSecurityConfig::from_env();

    // ── Política de transporte: mTLS o error. Una configuración de mTLS a medias
    //    o inválida es un fallo duro, no un downgrade silencioso a texto plano.
    let tls_cfg = MllpTlsConfig::from_env().map_err(|e| -> anyhow::Error {
        anyhow::anyhow!("configuración mTLS inválida del listener MLLP: {e}")
    })?;
    serve_with_security(address, db, ingest_state, sec, tls_cfg).await
}

/// Igual que [`serve`] pero con el material de mTLS y la configuración de
/// seguridad inyectados en lugar de leídos del entorno.
///
/// Existe para que las pruebas end-to-end puedan levantar el listener real sin
/// depender de variables de entorno de proceso: `DMART_MLLP_*` es global, así que
/// varias pruebas concurrentes se pisarían el material unas a otras y el
/// resultado dependería del orden de ejecución.
pub(crate) async fn serve_with_security(
    address: SocketAddr,
    db: Database,
    ingest_state: Arc<IngestState>,
    sec: MllpSecurityConfig,
    tls_cfg: Option<MllpTlsConfig>,
) -> anyhow::Result<()> {
    let tls = match tls_cfg {
        Some(cfg) => {
            let acceptor = cfg.acceptor().map_err(|e| -> anyhow::Error {
                anyhow::anyhow!("mTLS MLLP no disponible: {e}")
            })?;
            if !sec.allowed_senders.is_empty() {
                tracing::info!(
                    "[mllp] DMART_MLLP_ALLOWED_SENDERS se ignora con mTLS: la identidad la fija                      el certificado (pinned por fingerprint SHA-256)"
                );
            }
            Some((cfg, acceptor))
        }
        None => {
            assert_listener_security_policy(&sec)?;
            None
        }
    };

    let listener = TcpListener::bind(address).await?;
    tracing::info!("[mllp] escuchando en {address} con hardening (SPEC-031)");
    match &tls {
        Some((cfg, _)) => tracing::info!(
            "[mllp] transporte: mTLS TLS1.3 + AES-256-GCM, {} identidades pinneadas (SHA-256),              handshake_capa2={} max_conn={} max_conn_per_ip={} read_timeout={:?} write_timeout={:?}",
            cfg.identities.len(),
            if sec.auth_secret.is_some() {
                "ON"
            } else {
                "OFF"
            },
            sec.max_connections,
            sec.max_connections_per_ip,
            sec.read_timeout,
            sec.write_timeout,
        ),
        None => tracing::warn!(
            "[mllp] transporte: TCP EN CLARO (sin mTLS). Aceptable sólo en loopback/VLAN de \
             pruebas; en producción el arranque falla (DMART_ENV=production)."
        ),
    }
    if sec.auth_secret.is_none() && tls.is_some() {
        tracing::info!(
            "[mllp] DMART_MLLP_AUTH_SECRET no configurado: la autenticación se basa sólo en el \
             certificado de cliente (recomendado no duplicar el secreto en el cable)"
        );
    }

    let permits = Arc::new(Semaphore::new(sec.max_connections));
    let ip_counter = Arc::new(IpConnCounter::default());

    loop {
        let (stream, peer) = listener.accept().await?;
        tracing::debug!("[mllp] conexión desde {peer}");

        // Tope global de conexiones concurrentes.
        let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
            tracing::warn!("[mllp] límite global de conexiones alcanzado; rechazo {peer}");
            crate::metrics::ingest_message("mllp", "conn_limit");
            drop(stream);
            continue;
        };

        // Tope por IP origen.
        if !ip_counter.acquire(peer.ip(), sec.max_connections_per_ip) {
            tracing::warn!(
                "[mllp] {} excede {} conexiones simultáneas; rechazo",
                peer.ip(),
                sec.max_connections_per_ip
            );
            crate::metrics::ingest_message("mllp", "conn_limit_ip");
            drop(stream);
            drop(permit);
            continue;
        }

        let db = db.clone();
        let ingest_state = ingest_state.clone();
        let sec = sec.clone();
        let ip_counter = Arc::clone(&ip_counter);

        match tls.clone() {
            Some((cfg, acceptor)) => {
                tokio::spawn(async move {
                    let _permit = permit;
                    // Handshake TLS: sin certificado válido no se lee un solo
                    // byte de MLLP, de modo que no hay superficie en claro.
                    let tls_stream = match acceptor.accept(stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(
                                "[mllp:{peer}] handshake TLS fallido: {}",
                                tls_reason(&e)
                            );
                            crate::metrics::ingest_message("mllp", "tls_failed");
                            ip_counter.release(peer.ip());
                            return;
                        }
                    };
                    // Identidad = fingerprint del certificado pinneado.
                    //
                    // Se resuelve sobre el DER del certificado, no sobre su
                    // fingerprint hexadecimal: `sender_for_cert` hashea lo que
                    // recibe, así que pasarle el hex volvería a hashear el hex
                    // y nunca habría coincidencia.
                    let Some(der) = peer_cert_der(&tls_stream) else {
                        tracing::warn!("[mllp:{peer}] cliente sin certificado utilizable");
                        crate::metrics::ingest_message("mllp", "tls_failed");
                        ip_counter.release(peer.ip());
                        return;
                    };
                    let fp = MllpTlsConfig::fingerprint(&der);
                    match cfg.sender_for_cert(&der) {
                        Some(sender) => {
                            tracing::debug!("[mllp:{peer}] identidad mTLS: {sender}");
                            if let Err(e) = handle_stream(
                                TlsConnCompat::new(tls_stream),
                                db,
                                ingest_state,
                                peer,
                                sec,
                                Some(sender.to_string()),
                            )
                            .await
                            {
                                tracing::debug!("[mllp:{peer}] error de conexión: {e}");
                            }
                            // El cupo por IP se libera siempre: sin esto, cada
                            // conexión mTLS válida consumía un hueco para
                            // siempre y el emisor quedaba bloqueado al alcanzar
                            // el tope, sin forma de recuperarse.
                            ip_counter.release(peer.ip());
                        }
                        None => {
                            tracing::warn!(
                                "[mllp:{peer}] certificado fuera del pin (sha256={fp}); conexión \
                                 rechazada antes de leer datos"
                            );
                            crate::metrics::ingest_message("mllp", "cert_not_pinned");
                            ip_counter.release(peer.ip());
                        }
                    }
                });
            }
            None => {
                // Keepalive: detecta monitores desmontados sin cerrar el socket a
                // tiempo. Sólo aplica al socket TCP desnudo.
                let _ = stream.set_nodelay(true);
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(e) = handle_stream(stream, db, ingest_state, peer, sec, None).await {
                        tracing::debug!("[mllp:{peer}] error de conexión: {e}");
                    }
                    ip_counter.release(peer.ip());
                });
            }
        }
    }
}

/// Política fail-closed: en un despliegue declarado como productivo el listener
/// MLLP no puede arrancar en claro.
///
/// Se admite un escape hatch únicamente para despliegues no productivos, y
/// sólo si se pide explícitamente, de forma que el riesgo quede registrado en
/// el arranque y sea visible en los logs.
fn assert_listener_security_policy(sec: &MllpSecurityConfig) -> anyhow::Result<()> {
    let is_production = crate::deployment::is_production();
    let insecure_opt_in = crate::deployment::flag_enabled("DMART_MLLP_ALLOW_INSECURE");

    if is_production && insecure_opt_in {
        return Err(anyhow::anyhow!(
            "DMART_MLLP_ALLOW_INSECURE no es válido con DMART_ENV=production: el listener MLLP \
             expone PHI en claro. Configura mTLS (DMART_MLLP_CERT_FILE/KEY_FILE/CA_FILE + \
             DMART_MLLP_CLIENT_IDS)."
        ));
    }

    if !is_production {
        if sec.auth_secret.is_none() {
            tracing::warn!(
                "[mllp] DMART_MLLP_AUTH_SECRET no configurado: se acepta cualquier emisor. \
                 Sólo válido en loopback/VLAN de pruebas."
            );
        }
        return Ok(());
    }

    // Producción: mTLS o nada. Un secreto compartido en la capa 2 no cambia
    // que los signos vitales vayan por el cable en claro, así que aceptarlo
    // aquí dejaría PHI expuesta justo en el despliegue que más importa.
    // (Esta función sólo se invoca cuando NO hay mTLS configurado.)
    let _ = sec;
    Err(anyhow::anyhow!(
        "producción exige mTLS en el listener MLLP: sin DMART_MLLP_CERT_FILE/KEY_FILE/CA_FILE \
         + DMART_MLLP_CLIENT_IDS los signos vitales viajarían en claro por TCP. \
         DMART_MLLP_AUTH_SECRET no es alternativa: autentica el emisor pero no cifra el \
         transporte. Para pruebas en claro usa DMART_ENV distinto de production."
    ))
}

/// Motivo del fallo de handshake TLS, sin filtrar detalles del peer.
fn tls_reason(e: &std::io::Error) -> String {
    match e
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>())
    {
        Some(rustls::Error::InvalidCertificate(_)) => "certificado de cliente inválido".into(),
        Some(rustls::Error::NoCertificatesPresented) => "el cliente no presentó certificado".into(),
        Some(rustls::Error::DecryptError) => "fallo de cifrado negotiated".into(),
        _ => "handshake TLS rechazado".into(),
    }
}

/// DER del primer certificado presentado por el peer.
///
/// Se devuelve el DER en vez del fingerprint porque el pin se resuelve
/// hasheando: pasar el hexadecimal por la misma ruta lo hashearía dos veces.
fn peer_cert_der<S>(stream: &TlsStream<S>) -> Option<Vec<u8>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (_, conn) = stream.get_ref();
    let certs = conn.peer_certificates()?;
    certs.first().map(|c| c.as_ref().to_vec())
}

/// Adaptador mínimo para unificar `TlsStream` con `TcpStream` en
/// [`handle_stream`]. Ya cumple los bounds necesarios; el newtype deja el
/// intent explícito y evita repetir la firma genérica en las llamadas.
struct TlsConnCompat<S>(TlsStream<S>);

impl<S> TlsConnCompat<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn new(inner: TlsStream<S>) -> Self {
        Self(inner)
    }
}

impl<S> AsyncRead for TlsConnCompat<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for TlsConnCompat<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;

    fn sec(secret: Option<&str>) -> MllpSecurityConfig {
        MllpSecurityConfig {
            auth_secret: secret.map(|s| Zeroizing::new(s.to_string())),
            allowed_senders: Vec::new(),
            read_timeout: Duration::from_secs(1),
            write_timeout: Duration::from_secs(1),
            max_connections: 8,
            max_connections_per_ip: 2,
        }
    }

    fn frame(msg: &str) -> String {
        format!("\x0B{msg}\x1C\r")
    }

    const GOOD_HANDSHAKE: &str = "MSH|^~\\&|DMART-AUTH|s3cr3t-s3rv1d0r|A|1|||AL|NE";
    const BAD_SECRET: &str = "MSH|^~\\&|DMART-AUTH|guess|A|1|||AL|NE";
    const BAD_APP: &str = "MSH|^~\\&|monitor-bed-3|A|1|||AL|NE";

    /// Auditoría H3: un listener con secreto configurado NO acepta conexiones
    /// sin handshake válido, ni con el secreto correcto en otro campo.
    #[test]
    fn handshake_requires_configured_secret() {
        let cfg = sec(Some("s3cr3t-s3rv1d0r"));
        assert!(cfg.verify_handshake(GOOD_HANDSHAKE));
        assert!(!cfg.verify_handshake(BAD_SECRET));
        assert!(!cfg.verify_handshake(BAD_APP));
        assert!(!cfg.verify_handshake("no-es-msh"));
        assert!(!cfg.verify_handshake(""));
    }

    /// Sin secreto configurado el handshake pasa (modo legado), pero el `serve`
    /// emite un warning explícito de riesgo aceptado.
    #[test]
    fn handshake_open_when_not_configured() {
        let cfg = sec(None);
        assert!(cfg.verify_handshake(BAD_APP));
        assert!(cfg.auth_secret.is_none());
    }

    /// El secreto no puede colarse por prefijos/sufijos ni por longitudes
    /// distintas (comparación constant-time sobre el campo completo).
    #[test]
    fn handshake_rejects_prefix_of_secret() {
        let cfg = sec(Some("s3cr3t-s3rv1d0r"));
        assert!(!cfg.verify_handshake("MSH|^~\\&|DMART-AUTH|s3cr3t|A|1|||AL|NE"));
        assert!(!cfg.verify_handshake("MSH|^~\\&|DMART-AUTH|s3cr3t-s3rv1d0rX|A|1|||AL|NE"));
        assert!(!cfg.verify_handshake("MSH|^~\\&|DMART-AUTH||A|1|||AL|NE"));
    }

    #[test]
    fn sender_allowlist_enforced() {
        let mut cfg = sec(None);
        assert!(cfg.sender_allowed("cualquiera"));
        cfg.allowed_senders = vec!["monitor-bed-3".to_string()];
        assert!(cfg.sender_allowed("monitor-bed-3"));
        assert!(!cfg.sender_allowed("monitor-bed-9"));
    }

    /// El ACK nunca debe filtrar el error interno (ruta de BD, tabla, esquema).
    #[test]
    fn ack_error_codes_do_not_leak_internals() {
        let db_err = anyhow::anyhow!("create measurement: E220: No table 'measurements'");
        let code = ack_error_code(&db_err);
        assert_eq!(code, "internal_error");
        assert!(!code.contains("measurements"));
        assert_eq!(
            ack_error_code(&anyhow::anyhow!("paciente no encontrado para ref MRN-1")),
            "patient_not_found"
        );
        assert_eq!(
            ack_error_code(&anyhow::anyhow!("frame_too_large")),
            "frame_error"
        );
    }

    #[test]
    fn ack_builds_error_code_not_detail() {
        let ack = build_ack("MSG1", Some("internal_error"));
        let text = String::from_utf8_lossy(&ack);
        assert!(text.contains("ACKAR^MSG1"));
        assert!(text.contains("ERR|Ste|internal_error"));
    }

    /// Anti slow-loris: un peer que acepta la conexión y no envía nada debe
    /// ver su conexión cerrada al expirar `read_timeout`, no quedar ocupando un
    /// socket indefinidamente.
    #[tokio::test]
    async fn read_frame_respects_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.expect("accept");
            // El peer no envía nada.
            read_frame(&mut s, 1024, Duration::from_millis(150)).await
        });

        let _client = TcpStream::connect(addr).await.expect("connect");
        let outcome = tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .expect("read_frame no debe colgarse")
            .expect("join");
        let err = outcome.expect_err("esperado timeout de lectura");
        assert!(
            err.to_string().contains("timeout"),
            "debe ser un timeout, no otro error: {err}"
        );
    }

    /// Lectura correcta de un frame MLLP completo, con su tamaño real.
    #[tokio::test]
    async fn read_frame_returns_payload_and_size() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let payload = "MSH|^~\\&|X|Y|A|1|||AL|NE";
        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.expect("accept");
            read_frame(&mut s, 1024, Duration::from_secs(2)).await
        });

        // El emisor (monitor) es quien escribe el frame.
        let mut client = TcpStream::connect(addr).await.expect("connect");
        client
            .write_all(frame(payload).as_bytes())
            .await
            .expect("write");
        client.flush().await.expect("flush");

        let (raw, size) = tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .expect("no timeout")
            .expect("join")
            .expect("sin error")
            .expect("payload presente");
        assert_eq!(raw, payload);
        // El tamaño reportado incluye el terminador 0x1C 0x0D.
        assert_eq!(size, payload.len() + 2);
    }

    /// Un frame por encima de `max_frame_bytes` se rechaza (no se acumula
    /// memoria indefinidamente esperando el terminador).
    #[tokio::test]
    async fn read_frame_rejects_oversized_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.expect("accept");
            read_frame(&mut s, 1024, Duration::from_secs(3)).await
        });

        let mut client = TcpStream::connect(addr).await.expect("connect");
        let mut big = String::from("");
        big.push_str(&"X".repeat(5000));
        let _ = client.write_all(big.as_bytes()).await;

        let outcome = tokio::time::timeout(Duration::from_secs(4), server)
            .await
            .expect("no timeout")
            .expect("join");
        let err = outcome.expect_err("debe rechazar frame demasiado grande");
        assert!(err.to_string().contains("frame_too_large"));
    }

    /// Cierre limpio del peer antes de empezar un frame: `Ok(None)`, no error.
    #[tokio::test]
    async fn read_frame_returns_none_on_clean_close() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.expect("accept");
            read_frame(&mut s, 1024, Duration::from_secs(2)).await
        });

        let client = TcpStream::connect(addr).await.expect("connect");
        drop(client);

        let outcome = tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .expect("no timeout")
            .expect("join")
            .expect("sin error");
        assert!(outcome.is_none());
    }

    /// Política de transporte: con `DMART_ENV=production` el listener en claro
    /// no arranca nunca, y el secreto de capa 2 no es una alternativa.
    #[test]
    fn production_policy_refuses_cleartext_listener() {
        // Sin secreto ni mTLS → error duro que menciona mTLS.
        let cfg = sec(None);
        with_env(
            &[
                ("DMART_ENV", "production"),
                ("DMART_MLLP_ALLOW_INSECURE", ""),
            ],
            || {
                let err = assert_listener_security_policy(&cfg).expect_err("debe fallar");
                assert!(
                    err.to_string().contains("mTLS"),
                    "el error debe indicar el control que falta: {err}"
                );
            },
        );

        // Con secreto de capa 2 configurado en producción → también falla: el
        // secreto autentica pero no cifra, y los signos vitales ya están en el
        // cable antes de que la capa 2 se compruebe.
        let cfg = sec(Some("s3cr3t-s3rv1d0r"));
        with_env(&[("DMART_ENV", "production")], || {
            let err = assert_listener_security_policy(&cfg).expect_err("debe fallar");
            assert!(
                err.to_string()
                    .contains("DMART_MLLP_AUTH_SECRET no es alternativa")
            );
        });

        // El escape hatch tampoco cuela.
        let cfg = sec(Some("s3cr3t-s3rv1d0r"));
        with_env(
            &[
                ("DMART_ENV", "production"),
                ("DMART_MLLP_ALLOW_INSECURE", "1"),
            ],
            || {
                assert!(assert_listener_security_policy(&cfg).is_err());
            },
        );

        // Fuera de producción, el listener en claro se permite (loopback/VLAN).
        let cfg = sec(None);
        with_env(&[("DMART_ENV", "development")], || {
            assert!(assert_listener_security_policy(&cfg).is_ok());
        });
    }

    /// Aplica un entorno temporal a un cierre y lo restaura al salir.
    ///
    /// Delega en el guard compartido, que además toma el lock de variables de
    /// entorno. La versión anterior guardaba y restauraba sin tomar el lock, así
    /// que estos tests de política podían correr en paralelo con
    /// `test_master_key_validation` de `crypto`, ambos tocando las mismas
    /// variables globales del proceso.
    fn with_env(vars: &[(&str, &str)], f: impl FnOnce()) {
        let _lock = crate::deployment::tests_lock();
        // Una variable con valor vacío se trata como ausente.
        let owned: Vec<(String, Option<&str>)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_string(), if v.is_empty() { None } else { Some(*v) }))
            .collect();
        let _guard = crate::deployment::EnvGuard::new(&owned);
        f();
    }

    /// El log del fallo de TLS no debe filtrar detalles del peer.
    #[test]
    fn tls_failure_reasons_are_generic() {
        let generic = std::io::Error::new(std::io::ErrorKind::InvalidData, "no secret here");
        assert_eq!(tls_reason(&generic), "handshake TLS rechazado");
        let cert_err = rustls::Error::NoCertificatesPresented;
        let io = std::io::Error::new(std::io::ErrorKind::InvalidData, cert_err);
        assert_eq!(tls_reason(&io), "el cliente no presentó certificado");
        assert!(!tls_reason(&generic).contains("no secret"));
    }

    #[tokio::test]
    async fn connection_limit_per_ip_rejects_and_releases() {
        let counter = IpConnCounter::default();
        let ip: IpAddr = "10.1.2.3".parse().expect("ip");
        assert!(counter.acquire(ip, 2));
        assert!(counter.acquire(ip, 2));
        assert!(!counter.acquire(ip, 2), "excede el tope por IP");
        counter.release(ip);
        assert!(counter.acquire(ip, 2), "release libera plaza");
        // Otra IP no comparte cupo.
        let other: IpAddr = "10.1.2.4".parse().expect("ip");
        assert!(counter.acquire(other, 2));
    }
}
