//! MLLP Server with Ingest Hardening (SPEC-031)
//! This module is part of the library and uses crate:: for internal references.

use crate::db::Database;
use crate::hl7::ingest::ingest_vitals;
use crate::hl7::parser::parse_oru_message;
use crate::ingest::quality::{QualityValidator, ValidationReason, ValidationResult};
use crate::ingest::IngestState;
use crate::metrics::{
    hl7_error, hl7_processed, ingest_circuit_state, ingest_error_avg_set, ingest_fault_devices_set,
    ingest_gap, ingest_invalid, ingest_rate_limit_current,
    ingest_throttled,
};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub const START_BLOCK: u8 = 0x0B;
pub const END_BLOCK: u8 = 0x1C;
pub const CARRIAGE_RETURN: u8 = 0x0D;

/// Máximo tamaño de un mensaje HL7 (1 MiB).
pub const MAX_MESSAGE: usize = 1024 * 1024;

pub fn build_ack(message_id: &str, err: Option<&str>) -> Vec<u8> {
    let ack_code = if err.is_some() { "AR" } else { "AA" };
    let now = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let line = format!(
        "MSH|^~\\&|DMART|UCI|||{now}||ACK{ack_code}^{}|||P|2.5",
        message_id
    );
    let mut body = vec![START_BLOCK];
    body.extend_from_slice(line.as_bytes());
    body.push(END_BLOCK);
    body.push(CARRIAGE_RETURN);
    if let Some(err_full) = err {
        let err_line =
            format!("\rMSH|^~\\&|DMART|UCI|||{now}||ACK^R01||ERR|P|2.5\rERR|Ste|{err_full}");
        body.extend_from_slice(err_line.as_bytes());
    }
    body
}

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

async fn handle_stream(
    mut stream: TcpStream,
    db: Database,
    ingest_state: Arc<IngestState>,
    peer: std::net::SocketAddr,
) -> anyhow::Result<()> {
    let config = ingest_state.config().clone();
    let mut quality_validator = QualityValidator::new();

    loop {
        // Leer hasta el par 0x1C 0x0D
        let mut buf = Vec::with_capacity(2048);
        let mut last = None;
        let mut frame_size = 0;

        loop {
            let mut byte = [0u8; 1];
            let n = stream.read(&mut byte).await?;
            if n == 0 {
                tracing::debug!("[mllp:{peer}] cliente cerró conexión");
                return Ok(());
            }
            let b = byte[0];
            if buf.is_empty() && b == START_BLOCK {
                last = None;
                continue;
            }
            buf.push(b);
            frame_size += 1;
            if last == Some(END_BLOCK) && b == CARRIAGE_RETURN {
                buf.truncate(buf.len() - 2); // quitar 0x1C 0x0D
                break;
            }
            last = Some(b);
            if frame_size > config.max_frame_bytes {
                tracing::warn!(
                    "[mllp:{peer}] mensaje excede {} bytes; descartado",
                    config.max_frame_bytes
                );
                let ack = build_ack("", Some("frame_too_large"));
                let _ = stream.write_all(&ack).await;
                return Ok(());
            }
        }

        let raw = match String::from_utf8(buf) {
            Ok(s) => s,
            Err(_) => {
                let ack = build_ack("", Some("non-utf8 payload"));
                let _ = stream.write_all(&ack).await;
                continue;
            }
        };

        match parse_oru_message(&raw) {
            Ok(msg) => {
                let msg_id = msg.message_id.clone();
                let device_id = extract_device_id(&msg, &peer);
                let sequence = extract_sequence(&msg);

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
                    let _ = stream.write_all(&ack).await;
                    continue;
                }

                if metrics_update.circuit_open {
                    crate::metrics::ingest_message(&device_id, "circuit_open");
                    let ack = build_ack(&msg_id, Some("circuit_open"));
                    let _ = stream.write_all(&ack).await;
                    continue;
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
                        let _ = stream.write_all(&ack).await;
                    }
                    Err(e) => {
                        hl7_error(msg.source.label());
                        crate::metrics::ingest_message(&device_id, "parse_error");
                        tracing::warn!("[mllp:{peer}] ingestión falló: {e}");
                        let ack = build_ack(&msg_id, Some(&e.to_string()));
                        let _ = stream.write_all(&ack).await;
                    }
                }
            }
            Err(e) => {
                hl7_error("unknown");
                tracing::warn!("[mllp:{peer}] parseo falló: {e}");
                let ack = build_ack("", Some(&e.to_string()));
                let _ = stream.write_all(&ack).await;
            }
        }
    }
}

/// Levanta un listener MLLP en `addr`. No necesita auth (red interna de
/// hospitales); se recomienda aislar por VLAN.
pub async fn serve(
    address: std::net::SocketAddr,
    db: Database,
    ingest_state: Arc<IngestState>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(address).await?;
    tracing::info!("[mllp] escuchando en {address} con hardening (SPEC-031)");
    loop {
        let (stream, peer) = listener.accept().await?;
        tracing::debug!("[mllp] conexión desde {peer}");
        let db = db.clone();
        let ingest_state = ingest_state.clone();
        tokio::spawn(async move {
            let name = format!("{peer}");
            if let Err(e) = handle_stream(stream, db, ingest_state, peer).await {
                tracing::debug!("[mllp:{name}] error de conexión: {e}");
            }
        });
    }
}
