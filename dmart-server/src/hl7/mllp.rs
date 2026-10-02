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

/// Construye el ACK MLLP de un mensaje.
///
/// El formato del segmento MSH es contrato de interoperabilidad con los
/// monitores ya desplegados (`ACKAA^<id>` / `ACKAR^<id>`), así que no se
/// cambia sin coordinación.
///
/// Cuando hay error se añade un segmento `ERR` **dentro** del frame, antes del
/// terminador. Antes iba después de `\x1C\r`, y como el peer corta ahí la
/// lectura, el emisor nunca recibía el motivo del rechazo.
pub fn build_ack(message_id: &str, err: Option<&str>) -> Vec<u8> {
    let ack_code = if err.is_some() { "AR" } else { "AA" };
    let now = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let line = format!(
        "MSH|^~\\&|DMART|UCI|||{now}||ACK{ack_code}^{}|||P|2.5",
        message_id
    );
    let mut body = vec![START_BLOCK];
    body.extend_from_slice(line.as_bytes());
    if let Some(err_full) = err {
        body.push(b'\r');
        body.extend_from_slice(format!("ERR|Ste|{err_full}").as_bytes());
    }
    body.push(END_BLOCK);
    body.push(CARRIAGE_RETURN);
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

        // El ACK de error lleva un único MSH y el motivo dentro del frame.
        // Antes se emitía un segundo MSH^R01 **después** de `\x1C\r`: el peer
        // cortaba ahí, así que nunca veía el motivo, y el tramo posterior era
        // basura para cualquier parser MLLP estricto.
        let err =
            String::from_utf8(build_ack("MSGID001", Some("paciente desconocido"))).expect("utf8");
        assert!(err.contains("ACKAR^MSGID001"));
        assert!(err.contains("ERR|Ste|paciente desconocido"));
        assert_eq!(
            err.matches("MSH|").count(),
            1,
            "un ACK lleva un solo MSH: {err:?}"
        );
        assert!(err.ends_with("\u{001C}\r"));
        // El motivo va antes del terminador, no después.
        assert!(
            err.find("ERR|Ste|").unwrap() < err.find('\u{001C}').unwrap(),
            "el ERR debe ir dentro del frame: {err:?}"
        );
    }
}
