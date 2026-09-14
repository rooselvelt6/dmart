// SPEC-031: superficie HL7/MLLP legacy ejercitada por los 38 tests de conformance/integración (SPEC-028); el pipeline ACTIVO es server_ingest + ingest/ (SPEC-031).
//! Listener MLLP (Minimal Lower Layer Protocol) para recibir mensajes HL7
//! desde monitores de cama en la LAN.
//!
//! Frame: `0x0B <mensaje HL7> 0x1C 0x0D`. Responde ACK (o AR) HL7.

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
}
