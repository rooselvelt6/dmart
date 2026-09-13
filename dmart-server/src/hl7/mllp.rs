//! Listener MLLP (Minimal Lower Layer Protocol) para recibir mensajes HL7
//! desde monitores de cama en la LAN.
//!
//! Frame: `0x0B <mensaje HL7> 0x1C 0x0D`. Responde ACK (o AR) HL7.

use crate::db::Database;
use crate::hl7::ingest::ingest_vitals;
use crate::hl7::parser::parse_oru_message;
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

async fn handle_stream(mut stream: TcpStream, db: Database, name: &str) -> anyhow::Result<()> {
    loop {
        // Leer hasta el par 0x1C 0x0D
        let mut buf = Vec::with_capacity(2048);
        let mut last = None;
        loop {
            let mut byte = [0u8; 1];
            let n = stream.read(&mut byte).await?;
            if n == 0 {
                // Conexión cerrada por el monitor
                tracing::debug!("[mllp:{name}] cliente cerró conexión");
                return Ok(());
            }
            let b = byte[0];
            if buf.is_empty() && b == START_BLOCK {
                last = None;
                continue;
            }
            buf.push(b);
            if last == Some(END_BLOCK) && b == CARRIAGE_RETURN {
                buf.truncate(buf.len() - 2); // quitar 0x1C 0x0D
                break;
            }
            last = Some(b);
            if buf.len() > MAX_MESSAGE {
                tracing::warn!("[mllp:{name}] mensaje excede 1 MiB; descartado");
                return Ok(());
            }
        }

        let raw = match String::from_utf8(buf) {
            Ok(s) => s,
            Err(_) => {
                stream
                    .write_all(&build_ack("", Some("non-utf8 payload")))
                    .await?;
                continue;
            }
        };

        match parse_oru_message(&raw) {
            Ok(msg) => {
                let msg_id = msg.message_id.clone();
                let source = msg.source.label();
                match ingest_vitals(&db, &msg).await {
                    Ok(m) => {
                        crate::metrics::hl7_processed(source);
                        tracing::info!(
                            "[mllp:{name}] medición {} a paciente {}",
                            m.measurement_id,
                            m.patient_id
                        );
                        stream.write_all(&build_ack(&msg_id, None)).await?;
                    }
                    Err(e) => {
                        crate::metrics::hl7_error(source);
                        tracing::warn!("[mllp:{name}] ingestión falló: {e}");
                        stream
                            .write_all(&build_ack(&msg_id, Some(&e.to_string())))
                            .await?;
                    }
                }
            }
            Err(e) => {
                crate::metrics::hl7_error("unknown");
                tracing::warn!("[mllp:{name}] parseo falló: {e}");
                stream
                    .write_all(&build_ack("", Some(&e.to_string())))
                    .await?;
            }
        }
    }
}

/// Levanta un listener MLLP en `addr`. No necesita auth (red interna de
/// hospitales); se recomienda aislar por VLAN.
pub async fn serve(address: std::net::SocketAddr, db: Database) -> anyhow::Result<()> {
    let listener = TcpListener::bind(address).await?;
    tracing::info!("[mllp] escuchando en {address}");
    loop {
        let (stream, peer) = listener.accept().await?;
        tracing::debug!("[mllp] conexión desde {peer}");
        let db = db.clone();
        tokio::spawn(async move {
            let name = format!("{peer}");
            if let Err(e) = handle_stream(stream, db, &name).await {
                tracing::debug!("[mllp:{name}] error de conexión: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_builds_positive_and_error() {
        let ok = String::from_utf8(build_ack("MSGID001", None)).expect("utf8");
        assert!(ok.starts_with("\u{000B}MSH|^~\\&|DMART|UCI"));
        assert!(ok.contains("ACKAA^MSGID001"));
        assert!(ok.ends_with("\u{001C}\r"));

        let err =
            String::from_utf8(build_ack("MSGID001", Some("paciente desconocido"))).expect("utf8");
        assert!(err.contains("ACK^R01"));
    }

    #[tokio::test]
    async fn mllp_roundtrip_ingests_over_tcp() {
        use crate::db::connect as db_connect;
        use dmart_shared::models::Patient;
        use std::time::Duration;

        let dir = std::env::temp_dir().join(format!("dmart-mllp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = db_connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db");

        let mut p = Patient::new();
        p.nombre = "Paciente".into();
        p.apellido = "HL7".into();
        p.historia_clinica = "003939".into();
        let created = crate::db::create_patient(&db, p).await.expect("create");

        let addr = "127.0.0.1:0";
        let listener = TcpListener::bind(addr).await.expect("bind");
        let listen_addr = listener.local_addr().unwrap();
        let db2 = db.clone();
        let server = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let db = db2.clone();
                tokio::spawn(async move {
                    let _ = handle_stream(stream, db, "test").await;
                });
            }
        });

        let msg = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MSGMLLP|P|2.5\r\
             PID|||003939^^^TJUHMR||TEST^HL7||19600415|M\r\
             OBR|1|||||||20240821141030\r\
             OBX|1|NM|8867-4^Heart rate^LN||96|bpm\r";
        let frame = format!("\u{000B}{msg}\u{001C}\r");

        let mut client = TcpStream::connect(listen_addr).await.unwrap();
        client.write_all(frame.as_bytes()).await.unwrap();
        let mut ack = [0u8; 256];
        let n = client.read(&mut ack).await.unwrap();
        let ack_str = String::from_utf8_lossy(&ack[..n]).to_string();
        assert!(ack_str.contains("ACKAA"), "ACK positivo => {ack_str}");

        // Esperar a que la ingestión termine (corre en el server task)
        tokio::time::sleep(Duration::from_millis(400)).await;

        let measurements = crate::db::get_measurements_for_patient(&db, &created.patient_id)
            .await
            .expect("list");
        assert_eq!(measurements.len(), 1, "se ingirió la medición");
        assert_eq!(measurements[0].apache_data.frecuencia_cardiaca, 96.0);
        assert!(measurements[0].notas.contains("HL7 v2"));

        server.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
