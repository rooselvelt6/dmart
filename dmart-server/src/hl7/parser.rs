// SPEC-031: superficie HL7/MLLP legacy ejercitada por los 38 tests de conformance/integración (SPEC-028); el pipeline ACTIVO es server_ingest + ingest/ (SPEC-031).
//! Parser HL7 v2 para mensajes `ORU^R01` de monitores de cama (Mindray, Philips).
//!
//! Soporta terminadores `\r` y `\n`, encabezados de observación con códigos
//! LOINC o mnemónicos propietarios de fabricante, y convierte los signos
//! vitales a `ApacheIIData` usando valores neutros para lo que no viene.

use chrono::NaiveDateTime;
use dmart_shared::models::ApacheIIData;

/// Separador de campos HL7 (MSH-2 permite [custom], se respeta el del mensaje).
pub const FIELD_SEP: char = '|';
/// Separador de componentes.
pub const COMPONENT_SEP: char = '^';

/// Nivel de severidad de un fallo de parseo.
#[derive(Debug, Clone, PartialEq)]
pub enum Hl7ErrorKind {
    NotOruMessage,
    MissingPatientId,
    EmptyMessage,
    MalformedSegment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hl7Error {
    pub kind: Hl7ErrorKind,
    pub detail: String,
}

impl std::fmt::Display for Hl7Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

/// Fabricante del monitor, detectado por el emisor en MSH-3/MSH-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum MonitorSource {
    Mindray,
    Philips,
    Generic,
}

impl MonitorSource {
    pub fn label(self) -> &'static str {
        match self {
            MonitorSource::Mindray => "Mindray",
            MonitorSource::Philips => "Philips",
            MonitorSource::Generic => "Genérico",
        }
    }
}

/// Un signo vital con su código LOINC (cuando es mapeable).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Vital {
    pub loinc: Option<String>,
    pub name: String,
    pub value: f32,
    pub unit: String,
}

/// Tipo de signo vital reconocido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VitalKind {
    Hr,
    Rr,
    Spo2,
    Temp,
    SysAf,
    MeanArterial,
    DiaAf,
}

/// Mensaje ORU^R01 parseado.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct VitalsMessage {
    pub message_id: String,
    pub sender: String,
    /// Identificador de paciente: UUID de dMart si viene en PID-18, si no el MRN (PID-3.1).
    pub patient_ref: String,
    /// true si `patient_ref` es un `patient_id` de dMart (UUID).
    pub patient_ref_is_uuid: bool,
    pub timestamp: String,
    pub vitals: Vec<Vital>,
    pub source: MonitorSource,
    /// MSH.13 — sequence number (opcional, para gap detection)
    pub sequence_number: Option<u32>,
}

/// Segmentos de un mensaje: cada campo es un vector de componentes.
fn split_segments(msg: &str) -> Vec<(String, Vec<Vec<String>>)> {
    let field_sep = msg
        .chars()
        .nth(3)
        .filter(|c| matches!(c, '|' | '&'))
        .unwrap_or(FIELD_SEP);

    msg.split(['\r', '\n'])
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let parts = line.split(field_sep);
            let cat: Vec<&str> = line.splitn(2, field_sep).collect();
            let header = cat.first().map(|s| s.to_string()).unwrap_or_default();
            let fields: Vec<Vec<String>> = parts
                .skip(1)
                .map(|f| f.split(COMPONENT_SEP).map(str::to_string).collect())
                .collect();
            (header, fields)
        })
        .collect()
}

/// Convierte una fecha/hora HL7 (`YYYYMMDDHHMMSS[.fff]`) a RFC 3339 UTC.
/// Si no trae zona, se interpreta como UTC.
pub fn hl7_datetime_to_rfc3339(raw: &str) -> Option<String> {
    let core = raw.get(..14)?;
    let dt = NaiveDateTime::parse_from_str(core, "%Y%m%d%H%M%S").ok()?;
    Some(format!("{}Z", dt.format("%Y-%m-%dT%H:%M:%S")))
}

pub fn detect_vendor(sending_app: &str, sending_facility: &str) -> MonitorSource {
    let hay = format!("{} {}", sending_app, sending_facility).to_lowercase();
    if hay.contains("mindray") || hay.contains("benefit") || hay.contains("bene") {
        MonitorSource::Mindray
    } else if hay.contains("philips")
        || hay.contains("intellivue")
        || hay.contains("mp5")
        || hay.contains("mp6")
    {
        MonitorSource::Philips
    } else {
        MonitorSource::Generic
    }
}

/// Clasifica un OBX por identificador (LOINC o mnemónico) + nombre.
pub fn classify_vital(ident: &str, name: &str) -> Option<VitalKind> {
    let code_upper = ident.to_uppercase();
    let name_lower = name.to_lowercase();

    // 1) Códigos LOINC estándar
    match code_upper.as_str() {
        "8867-4" => return Some(VitalKind::Hr),
        "9279-1" => return Some(VitalKind::Rr),
        "2708-6" => return Some(VitalKind::Spo2),
        "8310-5" => return Some(VitalKind::Temp),
        "8480-6" => return Some(VitalKind::SysAf),
        "8478-0" => return Some(VitalKind::MeanArterial),
        "8460-8" => return Some(VitalKind::DiaAf),
        _ => {}
    }

    // 2) Mnemónicos de fabricante (Mindray / Philips / genéricos)
    let compact: String = code_upper.chars().filter(|c| !c.is_whitespace()).collect();
    match compact.as_str() {
        "HR" | "PR" | "PEAKHR" => return Some(VitalKind::Hr),
        "RR" | "RESP" | "RESPRATE" | "BR" => return Some(VitalKind::Rr),
        "SPO2" | "SPO2_2" | "OXY" => return Some(VitalKind::Spo2),
        "T1" | "T2" | "P1" | "TEMPERATURE" | "TEMP" => return Some(VitalKind::Temp),
        "NIBP_S" | "NIBPS" | "ARTS" | "SYSA" | "SYS" => return Some(VitalKind::SysAf),
        "NIBP_M" | "NIBPM" | "ARTM" | "MBP" | "MED" => return Some(VitalKind::MeanArterial),
        "NIBP_D" | "NIBPD" | "ARTD" | "DIA" => return Some(VitalKind::DiaAf),
        _ => {}
    }

    // 3) Fallback por nombre en español/inglés (normaliza acentos).
    let name_normalized: String = name_lower
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            c => c,
        })
        .collect();
    if name_normalized.contains("frecuencia") && name_normalized.contains("card")
        || name_normalized.contains("heart rate")
    {
        return Some(VitalKind::Hr);
    }
    if name_normalized.contains("respirator")
        || name_normalized.contains("frecuencia respiratoria")
        || name_normalized.replace(' ', "").contains("frecresp")
    {
        return Some(VitalKind::Rr);
    }
    if name_normalized.contains("saturacion")
        || name_normalized.contains("oxygen saturation")
        || name_normalized.contains("spo2")
    {
        return Some(VitalKind::Spo2);
    }
    if name_normalized.contains("temperatura") || name_normalized.contains("temperature") {
        return Some(VitalKind::Temp);
    }
    None
}

/// Parsea un mensaje HL7 v2 `ORU^R01` con segmentos MSH/PID/OBR/OBX.
pub fn parse_oru_message(raw: &str) -> Result<VitalsMessage, Hl7Error> {
    let segs = split_segments(raw);
    if segs.is_empty() {
        return Err(Hl7Error {
            kind: Hl7ErrorKind::EmptyMessage,
            detail: "no segments".to_string(),
        });
    }

    // ── MSH ──
    let msh = segs
        .iter()
        .find(|(h, _)| h == "MSH")
        .ok_or_else(|| Hl7Error {
            kind: Hl7ErrorKind::MalformedSegment,
            detail: "MSH missing".to_string(),
        })?;

    // MSH: después del encabezado, fields[0]=MSH-1, [1]=MSH-2, …
    let msg_type_full = msh
        .1
        .get(7)
        .map(|f| f.join(&COMPONENT_SEP.to_string()))
        .unwrap_or_default();
    if !msg_type_full.starts_with("ORU^R01") && !msg_type_full.is_empty() {
        return Err(Hl7Error {
            kind: Hl7ErrorKind::NotOruMessage,
            detail: format!("message type {msg_type_full}"),
        });
    }

    let sender = msh
        .1
        .get(1)
        .and_then(|f| f.first().cloned())
        .unwrap_or_default();
    let facility = msh
        .1
        .get(2)
        .and_then(|f| f.first().cloned())
        .unwrap_or_default();
    let source = detect_vendor(&sender, &facility);
    let message_id = msh
        .1
        .get(8)
        .and_then(|f| f.first().cloned())
        .unwrap_or_default();
    // MSH.13 — sequence number (opcional, para gap detection SPEC-031)
    let sequence_number = msh
        .1
        .get(12)
        .and_then(|f| f.first().cloned())
        .and_then(|s| s.parse::<u32>().ok());
    let msh_time = msh
        .1
        .get(5)
        .and_then(|f| f.first().cloned())
        .and_then(|s| hl7_datetime_to_rfc3339(&s));

    // ── PID ──
    let pid = segs
        .iter()
        .find(|(h, _)| h == "PID")
        .ok_or_else(|| Hl7Error {
            kind: Hl7ErrorKind::MissingPatientId,
            detail: "PID missing".to_string(),
        })?;

    // PID-18 (patient_id dMart) si parece UUID; si no, del MRN en PID-3.1
    let pid18 = pid
        .1
        .get(17)
        .and_then(|f| f.first().cloned())
        .unwrap_or_default();
    let pid3 = pid
        .1
        .get(2)
        .and_then(|f| f.first().cloned())
        .unwrap_or_default();

    let (patient_ref, patient_ref_is_uuid) = if pid18.contains('-') && pid18.len() == 36 {
        (pid18, true)
    } else {
        (pid3, false)
    };
    if patient_ref.is_empty() {
        return Err(Hl7Error {
            kind: Hl7ErrorKind::MissingPatientId,
            detail: "PID-3 empty and PID-18 empty".to_string(),
        });
    }

    // ── OBR → timestamp de la observación (OBR-8 si OBR-7 vacío) ──
    let obr_time = segs
        .iter()
        .find(|(h, _)| h == "OBR")
        .and_then(|(_, f)| {
            let o7 = f.get(6).and_then(|c| c.first().cloned());
            match o7 {
                Some(s) if !s.is_empty() => Some(s),
                _ => f.get(7).and_then(|c| c.first().cloned()),
            }
        })
        .and_then(|s| hl7_datetime_to_rfc3339(&s));

    // ── OBX ──
    let mut vitals: Vec<Vital> = Vec::new();
    for (_, fields) in segs.iter().filter(|(h, _)| h == "OBX") {
        let value_type = fields
            .get(1)
            .and_then(|f| f.first().cloned())
            .unwrap_or_default();
        if value_type != "NM" && value_type != "SN" && value_type != "TX" {
            continue;
        }
        let identifier = fields
            .get(2)
            .and_then(|f| f.first().cloned())
            .unwrap_or_default();
        if identifier.is_empty() {
            continue;
        }
        let name = fields
            .get(2)
            .and_then(|f| f.get(1).cloned())
            .unwrap_or_default();
        let value_str = fields
            .get(4)
            .and_then(|f| f.first().cloned())
            .unwrap_or_default();
        let unit = fields
            .get(5)
            .and_then(|c| c.first().cloned())
            .unwrap_or_default();
        let obx_time = fields
            .get(13)
            .and_then(|f| f.first().cloned())
            .and_then(|s| hl7_datetime_to_rfc3339(&s));

        let kind = match classify_vital(&identifier, &name) {
            Some(k) => k,
            None => continue, // Observación no vital (lab, etc.)
        };

        let Ok(value) = value_str.replace(',', ".").parse::<f32>() else {
            continue;
        };

        // Validación clínica del rango: descarta artefactos evidentes.
        let valid = match kind {
            VitalKind::Hr | VitalKind::Rr => (0.0..=300.0).contains(&value),
            VitalKind::Spo2 => (0.0..=100.0).contains(&value),
            VitalKind::Temp => (30.0..=44.0).contains(&value),
            VitalKind::SysAf => (0.0..=300.0).contains(&value),
            VitalKind::MeanArterial => (0.0..=250.0).contains(&value),
            VitalKind::DiaAf => (0.0..=250.0).contains(&value),
        };
        if !valid {
            continue;
        }

        vitals.push(Vital {
            loinc: None, // se rellena abajo
            name,
            value,
            unit,
        });
        let last = vitals.last_mut().expect("just pushed");
        last.loinc = Some(match kind {
            VitalKind::Hr => "8867-4".to_string(),
            VitalKind::Rr => "9279-1".to_string(),
            VitalKind::Spo2 => "2708-6".to_string(),
            VitalKind::Temp => "8310-5".to_string(),
            VitalKind::SysAf => "8480-6".to_string(),
            VitalKind::MeanArterial => "8478-0".to_string(),
            VitalKind::DiaAf => "8460-8".to_string(),
        });
        let _ = obx_time;
    }

    if vitals.is_empty() {
        return Err(Hl7Error {
            kind: Hl7ErrorKind::MalformedSegment,
            detail: "no vital OBX found".to_string(),
        });
    }

    let timestamp = obr_time
        .or(msh_time)
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));

    Ok(VitalsMessage {
        message_id,
        sender,
        patient_ref,
        patient_ref_is_uuid,
        timestamp,
        vitals,
        source,
        sequence_number,
    })
}

/// Aplica los signos vitales del mensaje sobre un registro base (los valores
/// que no vienen en el HL7 conservan el `base`).
pub fn vitals_into_apache(msg: &VitalsMessage, base: &ApacheIIData) -> ApacheIIData {
    let mut out = base.clone();
    for v in &msg.vitals {
        match v.loinc.as_deref() {
            Some("8867-4") => out.frecuencia_cardiaca = v.value,
            Some("9279-1") => out.frecuencia_respiratoria = v.value,
            Some("2708-6") => out.spo2 = v.value,
            Some("8310-5") => out.temperatura = v.value,
            Some("8480-6") => out.presion_sistolica = v.value,
            Some("8478-0") => out.presion_arterial_media = v.value,
            Some("8460-8")
                if out.presion_arterial_media <= 0.0 || out.presion_arterial_media > 250.0 =>
            {
                out.presion_arterial_media = v.value;
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINDRYA_ORU: &str = r#"MSH|^~\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MSGID001|P|2.5
PID|||003939^^^TJUHMR||GONZALEZ^LUIS||19600415|M|||CALLE 10|||
OBR|1|||||||20240821141030|||||||||||||||||||||||||
OBX|1|NM|8867-4^Heart rate^LN||94|{beats}/min||||||F|||20240821141030|||BeneVision
OBX|2|NM|9279-1^Respiratory rate^LN||22|/min||||||F|||20240821141030|||BeneVision
OBX|3|NM|2708-6^Oxygen saturation in Arterial blood^LN||91|%||||||F|||20240821141030|||BeneVision
OBX|4|NM|8310-5^Body temperature^LN||38.2|Cel||||||F|||20240821141030|||BeneVision"#;

    const PHILIPS_ORU: &str = r#"MSH|^~\&|IntelliVue|BED05|DMART|HOSP|20240821141031||ORU^R01|MSGID002|P|2.5.1
PID|||1122334^^^TJUHMR||RAMIREZ^ANA||19780302|F|||AV 3 #45|||
OBR|1|||||||20240821141030|||||||||||||||||||||||||
OBX|1|NM|HR^Heart Rate||72|bpm||||||F|||20240821141030|||IntelliVue
OBX|2|NM|RESP^Respiration Rate||14|/min||||||F|||20240821141030|||IntelliVue
OBX|3|NM|SPO2^SpO2||96|%||||||F|||20240821141030|||IntelliVue
OBX|4|NM|NIBPs^NIBP Systolic||118|mmHg||||||F|||20240821141030|||IntelliVue
OBX|5|NM|NIBPm^NIBP Mean||86|mmHg||||||F|||20240821141030|||IntelliVue
OBX|6|NM|T1^Temperature||37.1|Cel||||||F|||20240821141030|||IntelliVue"#;

    #[test]
    fn detects_mindray_and_parses_vitals() {
        let msg = parse_oru_message(MINDRYA_ORU).expect("parse");
        assert_eq!(msg.source, MonitorSource::Mindray);
        assert_eq!(msg.patient_ref, "003939");
        assert!(!msg.patient_ref_is_uuid);
        assert_eq!(msg.timestamp, "2024-08-21T14:10:30Z");
        assert_eq!(msg.vitals.len(), 4);
        assert_eq!(msg.vitals[0].loinc.as_deref(), Some("8867-4"));
        assert_eq!(msg.vitals[0].value, 94.0);
        assert_eq!(msg.vitals[3].loinc.as_deref(), Some("8310-5"));
        assert_eq!(msg.vitals[3].value, 38.2);
    }

    #[test]
    fn detects_philips_and_maps_loinc() {
        let msg = parse_oru_message(PHILIPS_ORU).expect("parse");
        assert_eq!(msg.source, MonitorSource::Philips);
        assert_eq!(msg.patient_ref, "1122334");
        assert_eq!(msg.vitals.len(), 6);
        let sys = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8480-6"));
        assert!(sys.is_some());
        assert_eq!(sys.unwrap().value, 118.0);
        let mean = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8478-0"));
        assert_eq!(mean.unwrap().value, 86.0);
    }

    #[test]
    fn patient_ref_is_uuid_when_pid18_present() {
        // PID-18 = account number; si parece UUID, se usa como patient_id directo.
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let mut fields: Vec<String> = vec![
            "PID".into(),
            "1".into(),               // PID-1 set id
            String::new(),            // PID-2
            "003939^^^TJUHMR".into(), // PID-3 MRN
            String::new(),            // PID-4
            "DIAZ^JUAN".into(),       // PID-5
        ];
        // PID-6 … PID-17 vacíos
        for _ in 6..=17 {
            fields.push(String::new());
        }
        fields.push(uuid.into()); // PID-18
        fields.push(String::new());
        let pid_line = fields.join("|");
        let raw = MINDRYA_ORU.replacen(
            "PID|||003939^^^TJUHMR||GONZALEZ^LUIS||19600415|M|||CALLE 10|||",
            &pid_line,
            1,
        );
        let msg = parse_oru_message(&raw).expect("parse");
        assert!(msg.patient_ref_is_uuid, "PID-18 con UUID se usa directo");
        assert_eq!(msg.patient_ref, uuid);
    }

    #[test]
    fn rejects_non_oru_and_empty() {
        let not_oru = MINDRYA_ORU.replacen("ORU^R01", "ADT^A01", 1);
        let err = parse_oru_message(&not_oru).unwrap_err();
        assert_eq!(err.kind, Hl7ErrorKind::NotOruMessage);

        let empty = parse_oru_message("").unwrap_err();
        assert_eq!(empty.kind, Hl7ErrorKind::EmptyMessage);
    }

    #[test]
    fn rejects_out_of_range_vitals() {
        let bad = MINDRYA_ORU.replace("||94|", "||450|");
        // 450 lpm > 300 → se descarta; los demás siguen presentes
        let msg = parse_oru_message(&bad).expect("parse con artefacto descartado");
        assert_eq!(msg.vitals[0].value, 22.0, "el artefacto HR se descarta");
    }

    #[test]
    fn date_conversion_handles_precision() {
        assert_eq!(
            hl7_datetime_to_rfc3339("20240821141030").as_deref(),
            Some("2024-08-21T14:10:30Z")
        );
        assert_eq!(
            hl7_datetime_to_rfc3339("20240821141030.123").as_deref(),
            Some("2024-08-21T14:10:30Z")
        );
        assert!(hl7_datetime_to_rfc3339("garbage").is_none());
    }

    #[test]
    fn vitals_merge_uses_base_for_missing() {
        let msg = parse_oru_message(PHILIPS_ORU).expect("parse");
        let base = ApacheIIData::default();
        let merged = vitals_into_apache(&msg, &base);
        assert_eq!(merged.frecuencia_cardiaca, 72.0);
        assert_eq!(merged.frecuencia_respiratoria, 14.0);
        assert_eq!(merged.presion_sistolica, 118.0);
        assert_eq!(merged.presion_arterial_media, 86.0);
        assert_eq!(merged.temperatura, 37.1);
        assert_eq!(merged.spo2, 96.0);
        // Los labs no vienen en HL7 y conservan el valor neutro del base
        assert_eq!(merged.ph_arterial, 7.40);
    }
}
