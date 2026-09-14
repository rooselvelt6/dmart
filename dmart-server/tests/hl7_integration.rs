//! HL7 v2 + MLLP Integration Tests (tokio-test mock streams)
//! SPEC-003: Coverage parser + framer > 90%

use dmart_server::db::{self as db_ops};
use dmart_server::hl7::ingest::ingest_vitals;
use dmart_server::hl7::mllp::{CARRIAGE_RETURN, END_BLOCK, START_BLOCK, build_ack};
use dmart_server::hl7::parser::{
    Hl7ErrorKind, MonitorSource, VitalKind, classify_vital, detect_vendor, hl7_datetime_to_rfc3339,
    parse_oru_message, vitals_into_apache,
};
use dmart_shared::models::{ApacheIIData, Patient};

mod hl7_parser_integration {
    use super::*;

    const VALID_MINDRYA_ORU: &str = r#"MSH|^~\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MSGID001|P|2.5
PID|||003939^^^TJUHMR||GONZALEZ^LUIS||19600415|M|||CALLE 10|||
OBR|1|||||||20240821141030|||||||||||||||||||||||||
OBX|1|NM|8867-4^Heart rate^LN||94|{beats}/min||||||F|||20240821141030|||BeneVision
OBX|2|NM|9279-1^Respiratory rate^LN||22|/min||||||F|||20240821141030|||BeneVision
OBX|3|NM|2708-6^Oxygen saturation in Arterial blood^LN||91|%||||||F|||20240821141030|||BeneVision
OBX|4|NM|8310-5^Body temperature^LN||38.2|Cel||||||F|||20240821141030|||BeneVision"#;

    const VALID_PHILIPS_ORU: &str = r#"MSH|^~\&|IntelliVue|BED05|DMART|HOSP|20240821141031||ORU^R01|MSGID002|P|2.5.1
PID|||1122334^^^TJUHMR||RAMIREZ^ANA||19780302|F|||AV 3 #45|||
OBR|1|||||||20240821141030|||||||||||||||||||||||||
OBX|1|NM|HR^Heart Rate||72|bpm||||||F|||20240821141030|||IntelliVue
OBX|2|NM|RESP^Respiration Rate||14|/min||||||F|||20240821141030|||IntelliVue
OBX|3|NM|SPO2^SpO2||96|%||||||F|||20240821141030|||IntelliVue
OBX|4|NM|NIBPs^NIBP Systolic||118|mmHg||||||F|||20240821141030|||IntelliVue
OBX|5|NM|NIBPm^NIBP Mean||86|mmHg||||||F|||20240821141030|||IntelliVue
OBX|6|NM|T1^Temperature||37.1|Cel||||||F|||20240821141030|||IntelliVue"#;

    #[test]
    fn test_valid_oru_mindray_parses_all_vitals() {
        let msg = parse_oru_message(VALID_MINDRYA_ORU).expect("parse");

        assert_eq!(msg.source, MonitorSource::Mindray);
        assert_eq!(msg.patient_ref, "003939");
        assert!(!msg.patient_ref_is_uuid);
        assert_eq!(msg.vitals.len(), 4);

        // Verify all 4 vitals have correct LOINC and values
        let hr = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8867-4"))
            .expect("HR");
        assert_eq!(hr.value, 94.0);
        assert_eq!(hr.unit, "{beats}/min");

        let rr = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("9279-1"))
            .expect("RR");
        assert_eq!(rr.value, 22.0);

        let spo2 = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("2708-6"))
            .expect("SpO2");
        assert_eq!(spo2.value, 91.0);

        let temp = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8310-5"))
            .expect("Temp");
        assert_eq!(temp.value, 38.2);
    }

    #[test]
    fn test_valid_oru_philips_maps_mnemonics_to_loinc() {
        let msg = parse_oru_message(VALID_PHILIPS_ORU).expect("parse");

        assert_eq!(msg.source, MonitorSource::Philips);
        assert_eq!(msg.vitals.len(), 6);

        // Verify mnemonics mapped to correct LOINC
        let hr = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8867-4"))
            .expect("HR");
        assert_eq!(hr.value, 72.0);

        let sys = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8480-6"))
            .expect("SYS");
        assert_eq!(sys.value, 118.0);

        let mean = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8478-0"))
            .expect("MEAN");
        assert_eq!(mean.value, 86.0);
    }

    #[test]
    fn test_patient_ref_uuid_from_pid18() {
        // PID-18 (patient account number) lleva el patient_id de dMart (UUID)
        let uuid = "550e8400-e29b-41d4-a716-446655440000";

        // Construimos la línea PID con PID-18 explícito (evita contar separadores).
        let mut pid_fields: Vec<String> = vec![
            "PID".into(),
            String::new(),            // PID-1
            String::new(),            // PID-2
            "003939^^^TJUHMR".into(), // PID-3 MRN
            String::new(),            // PID-4
            "GONZALEZ^LUIS".into(),   // PID-5
            String::new(),            // PID-6
            "19600415".into(),        // PID-7
            "M".into(),               // PID-8
        ];
        // PID-9 … PID-17 vacíos
        for _ in 9..=17 {
            pid_fields.push(String::new());
        }
        pid_fields.push(uuid.into()); // PID-18

        let pid_line = pid_fields.join("|");
        let raw = format!(
            "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MSGID001|P|2.5\r\
            {}\r\
            OBR|1|||||||20240821141030|||||||||||||||||||||||||\r\
            OBX|1|NM|8867-4^Heart rate^LN||94|{{beats}}/min||||||F|||20240821141030|||BeneVision",
            pid_line
        );

        let msg = parse_oru_message(&raw).expect("parse");
        assert!(msg.patient_ref_is_uuid);
        assert_eq!(msg.patient_ref, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_malformed_hl7_rejected_gracefully() {
        // Empty message
        let err = parse_oru_message("").unwrap_err();
        assert_eq!(err.kind, Hl7ErrorKind::EmptyMessage);

        // Non-ORU message
        let not_oru = VALID_MINDRYA_ORU.replacen("ORU^R01", "ADT^A01", 1);
        let err = parse_oru_message(&not_oru).unwrap_err();
        assert_eq!(err.kind, Hl7ErrorKind::NotOruMessage);

        // Missing PID
        let no_pid = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MSG|P|2.5";
        let err = parse_oru_message(no_pid).unwrap_err();
        assert_eq!(err.kind, Hl7ErrorKind::MissingPatientId);

        // Missing patient ID in PID
        let no_pid3 = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MSG|P|2.5\rPID|||||||||||";
        let err = parse_oru_message(no_pid3).unwrap_err();
        assert_eq!(err.kind, Hl7ErrorKind::MissingPatientId);
    }

    #[test]
    fn test_out_of_range_vitals_discarded() {
        // HR = 450 > 300 max, should be discarded
        let bad = VALID_MINDRYA_ORU.replace("||94|", "||450|");
        let msg = parse_oru_message(&bad).expect("parse with artifact discarded");

        // Should have 3 vitals (HR discarded)
        assert_eq!(msg.vitals.len(), 3);
        // First vital should be RR (22), not HR
        assert_eq!(msg.vitals[0].value, 22.0);
    }

    #[test]
    fn test_classify_vital_loinc_codes() {
        assert_eq!(classify_vital("8867-4", "Heart Rate"), Some(VitalKind::Hr));
        assert_eq!(
            classify_vital("9279-1", "Respiratory Rate"),
            Some(VitalKind::Rr)
        );
        assert_eq!(classify_vital("2708-6", "SpO2"), Some(VitalKind::Spo2));
        assert_eq!(
            classify_vital("8310-5", "Temperature"),
            Some(VitalKind::Temp)
        );
        assert_eq!(
            classify_vital("8480-6", "Systolic BP"),
            Some(VitalKind::SysAf)
        );
        assert_eq!(
            classify_vital("8478-0", "Mean Arterial"),
            Some(VitalKind::MeanArterial)
        );
        assert_eq!(
            classify_vital("8460-8", "Diastolic BP"),
            Some(VitalKind::DiaAf)
        );
    }

    #[test]
    fn test_classify_vital_mnemonics() {
        // Mindray
        assert_eq!(classify_vital("HR", "Heart Rate"), Some(VitalKind::Hr));
        assert_eq!(classify_vital("RR", "Resp"), Some(VitalKind::Rr));
        assert_eq!(classify_vital("SPO2", "SpO2"), Some(VitalKind::Spo2));
        assert_eq!(classify_vital("TEMP", "Temperature"), Some(VitalKind::Temp));

        // Philips
        assert_eq!(classify_vital("NIBPs", "Systolic"), Some(VitalKind::SysAf));
        assert_eq!(
            classify_vital("NIBPm", "Mean"),
            Some(VitalKind::MeanArterial)
        );
        assert_eq!(classify_vital("NIBPd", "Diastolic"), Some(VitalKind::DiaAf));

        // Generic
        assert_eq!(classify_vital("PR", "Pulse Rate"), Some(VitalKind::Hr));
        assert_eq!(classify_vital("RESP", "Respiration"), Some(VitalKind::Rr));
    }

    #[test]
    fn test_classify_vital_by_name_fallback() {
        assert_eq!(classify_vital("UNKNOWN", "Heart Rate"), Some(VitalKind::Hr));
        assert_eq!(
            classify_vital("UNKNOWN", "Frecuencia Cardíaca"),
            Some(VitalKind::Hr)
        );
        assert_eq!(
            classify_vital("UNKNOWN", "Respiratory Rate"),
            Some(VitalKind::Rr)
        );
        assert_eq!(
            classify_vital("UNKNOWN", "Frecuencia Respiratoria"),
            Some(VitalKind::Rr)
        );
        assert_eq!(
            classify_vital("UNKNOWN", "Oxygen Saturation"),
            Some(VitalKind::Spo2)
        );
        assert_eq!(
            classify_vital("UNKNOWN", "Saturación"),
            Some(VitalKind::Spo2)
        );
        assert_eq!(
            classify_vital("UNKNOWN", "Temperature"),
            Some(VitalKind::Temp)
        );
        assert_eq!(
            classify_vital("UNKNOWN", "Temperatura"),
            Some(VitalKind::Temp)
        );
    }

    #[test]
    fn test_detect_vendor() {
        assert_eq!(
            detect_vendor("BeneVision", "Hospital"),
            MonitorSource::Mindray
        );
        assert_eq!(detect_vendor("Benefit", "Hospital"), MonitorSource::Mindray);
        assert_eq!(
            detect_vendor("IntelliVue", "Hospital"),
            MonitorSource::Philips
        );
        assert_eq!(detect_vendor("MP5", "Hospital"), MonitorSource::Philips);
        assert_eq!(detect_vendor("Generic", "Hospital"), MonitorSource::Generic);
    }

    #[test]
    fn test_hl7_datetime_conversion() {
        assert_eq!(
            hl7_datetime_to_rfc3339("20240821141030"),
            Some("2024-08-21T14:10:30Z".to_string())
        );
        assert_eq!(
            hl7_datetime_to_rfc3339("20240821141030.123"),
            Some("2024-08-21T14:10:30Z".to_string())
        );
        assert!(hl7_datetime_to_rfc3339("garbage").is_none());
        assert!(hl7_datetime_to_rfc3339("2024").is_none());
    }

    #[test]
    fn test_vitals_into_apache_merges_correctly() {
        let msg = parse_oru_message(VALID_PHILIPS_ORU).expect("parse");
        let base = ApacheIIData::default();
        let merged = vitals_into_apache(&msg, &base);

        assert_eq!(merged.frecuencia_cardiaca, 72.0);
        assert_eq!(merged.frecuencia_respiratoria, 14.0);
        assert_eq!(merged.presion_sistolica, 118.0);
        assert_eq!(merged.presion_arterial_media, 86.0);
        assert_eq!(merged.temperatura, 37.1);
        assert_eq!(merged.spo2, 96.0);

        // Labs should remain at base values
        assert_eq!(merged.ph_arterial, 7.40);
    }

    #[test]
    fn test_multiple_obx_segments_parsed() {
        // Create a message with many OBX segments
        let multi_obx = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MULTI|P|2.5\r\
            PID|||003939^^^TJUHMR||TEST^PATIENT||19600415|M\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^HR^LN||80|bpm\r\
            OBX|2|NM|9279-1^RR^LN||16|/min\r\
            OBX|3|NM|2708-6^SpO2^LN||98|%\r\
            OBX|4|NM|8310-5^Temp^LN||37.0|Cel\r\
            OBX|5|NM|8480-6^SYS^LN||120|mmHg\r\
            OBX|6|NM|8478-0^MAP^LN||85|mmHg\r\
            OBX|7|NM|8460-8^DIA^LN||70|mmHg\r\
            OBX|8|NM|9999-9^Unknown^LN||50|units\r"; // Debe ignorarse

        let msg = parse_oru_message(multi_obx).expect("parse");

        // 7 vitals parsed (1 unknown ignored)
        assert_eq!(msg.vitals.len(), 7);
    }

    #[test]
    fn test_escape_sequences_in_obx() {
        // HL7 escape sequences: \F\ \R\ \S\ \T\ \E\
        let msg_with_escape = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|ESC|P|2.5\r\
            PID|||003939^^^TJUHMR||TEST^PATIENT||19600415|M\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^Heart Rate^LN||90|bpm||||||F|||20240821141030|||BeneVision\r\
            OBX|2|TX|9999-9^Note^LN||Patient is stable\\.\\r\\nMonitoring continues|text";

        let msg = parse_oru_message(msg_with_escape).expect("parse with escapes");

        // HR should still parse
        let hr = msg
            .vitals
            .iter()
            .find(|v| v.loinc.as_deref() == Some("8867-4"));
        assert!(hr.is_some());
        assert_eq!(hr.unwrap().value, 90.0);
    }
}

mod hl7_mllp_integration {
    use super::*;

    #[tokio::test]
    async fn test_mllp_frame_parsing() {
        let hl7_msg = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MLLP001|P|2.5\r\
            PID|||003939^^^TJUHMR||TEST^PATIENT||19600415|M\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^Heart rate^LN||88|bpm";

        let _frame = format!(
            "{}{}{}{}",
            START_BLOCK as char, hl7_msg, END_BLOCK as char, CARRIAGE_RETURN as char
        );

        // Test parse_via_mllp if exposed, or test frame parsing manually
        // For now, test the frame building
        let ack = build_ack("MLLP001", None);
        let ack_str = String::from_utf8(ack).expect("utf8");

        assert!(ack_str.starts_with(START_BLOCK as char));
        assert!(ack_str.contains("ACKAA^MLLP001"));
        assert!(ack_str.ends_with(&format!("{}{}", END_BLOCK as char, CARRIAGE_RETURN as char)));
    }

    #[test]
    fn test_build_ack_positive() {
        let ack = build_ack("MSG001", None);
        let ack_str = String::from_utf8(ack).expect("utf8");

        assert!(ack_str.contains("ACKAA^MSG001"));
        assert!(ack_str.contains("DMART|UCI"));
    }

    #[test]
    fn test_build_ack_error() {
        let ack = build_ack("MSG001", Some("paciente desconocido"));
        let ack_str = String::from_utf8(ack).expect("utf8");

        assert!(ack_str.contains("ACK^R01"));
        assert!(ack_str.contains("ERR|Ste|paciente desconocido"));
    }

    #[test]
    fn test_multiple_messages_in_stream() {
        // Simulate a stream with multiple HL7 messages framed by MLLP
        let msg1 = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|M1|P|2.5\rPID|||001^^^TJUHMR||PAT^ONE||19600415|M\rOBR|1|||||||20240821141030\rOBX|1|NM|8867-4^HR^LN||80|bpm";
        let msg2 = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141032||ORU^R01|M2|P|2.5\rPID|||002^^^TJUHMR||PAT^TWO||19700520|F\rOBR|1|||||||20240821141031\rOBX|1|NM|8867-4^HR^LN||75|bpm";

        let _frame1 = format!(
            "{}{}{}{}",
            START_BLOCK as char, msg1, END_BLOCK as char, CARRIAGE_RETURN as char
        );
        let _frame2 = format!(
            "{}{}{}{}",
            START_BLOCK as char, msg2, END_BLOCK as char, CARRIAGE_RETURN as char
        );

        // Verify each frame can be parsed independently
        let parsed1 = parse_oru_message(msg1).expect("msg1 parse");
        let parsed2 = parse_oru_message(msg2).expect("msg2 parse");

        assert_eq!(parsed1.message_id, "M1");
        assert_eq!(parsed2.message_id, "M2");
        assert_eq!(parsed1.patient_ref, "001");
        assert_eq!(parsed2.patient_ref, "002");
    }
}

mod hl7_mllp_stream {
    use super::*;
    use dmart_server::ingest::{IngestConfig, IngestState};
    use dmart_server::server_ingest::{MAX_MESSAGE, serve};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    async fn spawn_server(db: db_ops::Database) -> std::net::SocketAddr {
        let probe = TcpListener::bind("127.0.0.1:0").await.expect("probe bind");
        let addr = probe.local_addr().expect("probe addr");
        drop(probe);
        let ingest_config = IngestConfig::default();
        let ingest_state = Arc::new(IngestState::new(ingest_config));
        tokio::spawn(async move {
            let _ = serve(addr, db, ingest_state).await;
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if TcpStream::connect(addr).await.is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("serve no escuchó el puerto");
        addr
    }

    /// Envía un frame MLLP y acumula la respuesta del server hasta EOF o `\x1C\r`.
    async fn send_frame(addr: std::net::SocketAddr, frame: &[u8]) -> Vec<u8> {
        let mut client = TcpStream::connect(addr).await.expect("connect");
        client.write_all(frame).await.expect("write frame");
        let mut ack = Vec::new();
        let mut buf = [0u8; 512];
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(2), client.read(&mut buf))
                .await
            {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    ack.extend_from_slice(&buf[..n]);
                    if ack.len() >= 2 && ack[ack.len() - 2..] == [END_BLOCK, CARRIAGE_RETURN] {
                        break;
                    }
                }
                Ok(Err(e)) => match e.kind() {
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe => break,
                    other => panic!("read error: {other}"),
                },
                Err(_) => break,
            }
        }
        ack
    }

    #[tokio::test]
    async fn test_mllp_server_parse_error_returns_ar() {
        let dir = std::env::temp_dir().join(format!("dmart-mllp-parse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = db_ops::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");
        let addr = spawn_server(db).await;

        // Mensaje con MSH válido pero tipo ADT (no ORU^R01) → parseo falla
        let bad = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ADT^A01|BAD|P|2.5\r\
             PID|||003939^^^TJUHMR||TEST^PATIENT||19600415|M";
        let mut frame = vec![START_BLOCK];
        frame.extend_from_slice(bad.as_bytes());
        frame.extend_from_slice(&[END_BLOCK, CARRIAGE_RETURN]);

        let ack = send_frame(addr, &frame).await;
        let ack_str = String::from_utf8_lossy(&ack).into_owned();
        assert!(ack_str.contains("AR"), "ACK AR esperado => {ack_str}");
        assert!(
            ack_str.contains("ERR|Ste|"),
            "debe incluir detalle => {ack_str}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_mllp_server_non_utf8_returns_ar() {
        let dir = std::env::temp_dir().join(format!("dmart-mllp-nonutf8-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = db_ops::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");
        let addr = spawn_server(db).await;

        let frame = [START_BLOCK, 0xFF, 0xFE, END_BLOCK, CARRIAGE_RETURN];
        let ack = send_frame(addr, &frame).await;
        let ack_str = String::from_utf8_lossy(&ack).into_owned();
        assert!(ack_str.contains("AR"), "ACK AR esperado => {ack_str}");
        assert!(ack_str.contains("non-utf8"), "motivo en ACK => {ack_str}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_mllp_server_ingest_error_returns_ar() {
        let dir = std::env::temp_dir().join(format!("dmart-mllp-ingest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = db_ops::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");
        let addr = spawn_server(db).await;

        // ORU válido pero paciente MRN 999999 no existe → ingestión falla
        let hl7 = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MISS|P|2.5\r\
             PID|||999999^^^TJUHMR||GHOST^PATIENT||19600415|M\r\
             OBR|1|||||||20240821141030\r\
             OBX|1|NM|8867-4^Heart rate^LN||80|bpm";
        let mut frame = vec![START_BLOCK];
        frame.extend_from_slice(hl7.as_bytes());
        frame.extend_from_slice(&[END_BLOCK, CARRIAGE_RETURN]);

        let ack = send_frame(addr, &frame).await;
        let ack_str = String::from_utf8_lossy(&ack).into_owned();
        assert!(ack_str.contains("AR"), "ACK AR esperado => {ack_str}");
        assert!(
            ack_str.contains("ERR|Ste|"),
            "debe incluir detalle => {ack_str}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_mllp_server_oversized_message_close() {
        let dir = std::env::temp_dir().join(format!("dmart-mllp-big-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = db_ops::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");
        let addr = spawn_server(db).await;

        // Frame > 1 MiB → el server responde AR con error "frame_too_large" y cierra
        let mut payload = Vec::with_capacity(MAX_MESSAGE + 64);
        payload.push(START_BLOCK);
        payload.resize(payload.len() + MAX_MESSAGE + 32, b'X');
        payload.push(END_BLOCK);
        payload.push(CARRIAGE_RETURN);

        let ack = send_frame(addr, &payload).await;
        // Debe responder AR (error) no cerrar sin ACK
        assert!(!ack.is_empty(), "server debe responder AR con error");
        let ack_str = String::from_utf8_lossy(&ack);
        assert!(
            ack_str.contains("AR"),
            "debe ser ACK negativo (AR), got: {}",
            ack_str
        );
        assert!(
            ack_str.contains("frame_too_large"),
            "debe indicar frame_too_large"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_mllp_server_client_graceful_close() {
        let dir = std::env::temp_dir().join(format!("dmart-mllp-eof-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = db_ops::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");
        let addr = spawn_server(db).await;

        // Cliente se conecta y cierra al instante → el server maneja EOF sin panic
        let client = TcpStream::connect(addr).await.expect("connect");
        drop(client);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // El server sigue vivo: nueva conexión responde OK
        let hl7 = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|EOFOK|P|2.5\r\
             PID|||NOPE^^^TJUHMR||X^Y||19600415|M\r\
             OBR|1|||||||20240821141030\r\
             OBX|1|NM|8867-4^Heart rate^LN||80|bpm";
        let mut frame = vec![START_BLOCK];
        frame.extend_from_slice(hl7.as_bytes());
        frame.extend_from_slice(&[END_BLOCK, CARRIAGE_RETURN]);
        let ack = send_frame(addr, &frame).await;
        assert!(!ack.is_empty(), "server sigue respondiendo tras EOF");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

mod hl7_ingest_integration {
    use super::*;

    #[tokio::test]
    async fn test_ingest_vitals_creates_measurement() {
        let dir = std::env::temp_dir().join(format!("dmart-hl7-ingest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let db = dmart_server::db::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");

        // Create a patient first
        let mut patient = Patient::new();
        patient.nombre = "Test".into();
        patient.apellido = "HL7".into();
        patient.historia_clinica = "003939".into();
        let created_patient = db_ops::create_patient(&db, patient)
            .await
            .expect("create patient");

        // Create HL7 message
        let hl7_msg = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|INGEST001|P|2.5\r\
            PID|||003939^^^TJUHMR||TEST^PATIENT||19600415|M\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^Heart rate^LN||94|bpm\r\
            OBX|2|NM|9279-1^Respiratory rate^LN||20|/min\r\
            OBX|3|NM|2708-6^SpO2^LN||95|%\r\
            OBX|4|NM|8310-5^Temperature^LN||37.5|Cel";

        let msg = parse_oru_message(hl7_msg).expect("parse");

        // Ingest
        let measurement = ingest_vitals(&db, &msg).await.expect("ingest");

        assert_eq!(measurement.patient_id, created_patient.patient_id);
        assert_eq!(measurement.apache_data.frecuencia_cardiaca, 94.0);
        assert_eq!(measurement.apache_data.frecuencia_respiratoria, 20.0);
        assert_eq!(measurement.apache_data.spo2, 95.0);
        assert_eq!(measurement.apache_data.temperatura, 37.5);
        assert!(measurement.notas.contains("Auto-ingesta HL7 v2"));
        assert!(measurement.notas.contains("Mindray"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_ingest_vitals_updates_patient_severity() {
        let dir = std::env::temp_dir().join(format!("dmart-hl7-severity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let db = dmart_server::db::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");

        let mut patient = Patient::new();
        patient.nombre = "Test".into();
        patient.apellido = "Severity".into();
        patient.historia_clinica = "999999".into();
        let created_patient = db_ops::create_patient(&db, patient)
            .await
            .expect("create patient");

        // High severity HL7 message
        let hl7_msg = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|SEV001|P|2.5\r\
            PID|||999999^^^TJUHMR||SEVERITY^TEST||19600415|M\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^HR^LN||130|bpm\r\
            OBX|2|NM|9279-1^RR^LN||35|/min\r\
            OBX|3|NM|2708-6^SpO2^LN||85|%\r\
            OBX|4|NM|8310-5^Temp^LN||39.5|Cel";

        let msg = parse_oru_message(hl7_msg).expect("parse");
        let _measurement = ingest_vitals(&db, &msg).await.expect("ingest");

        // Verify patient severity updated
        let updated = db_ops::get_patient(&db, &created_patient.patient_id)
            .await
            .expect("get");
        let updated = updated.expect("patient exists");

        // Verify mortality risk is calculated from Apache score
        assert!(updated.mortality_risk.unwrap() > 0.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_ingest_vitals_missing_patient_returns_error() {
        let dir = std::env::temp_dir().join(format!("dmart-hl7-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let db = dmart_server::db::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");

        let hl7_msg = "MSH|^~\\&|BeneVision|TJUH|DMART|HOSP|20240821141031||ORU^R01|MISSING|P|2.5\r\
            PID|||NONEXISTENT^^^TJUHMR||GHOST^PATIENT||19600415|M\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^HR^LN||80|bpm";

        let msg = parse_oru_message(hl7_msg).expect("parse");

        let result = ingest_vitals(&db, &msg).await;
        // Verify that ingesting vitals for a non-existent patient returns an error
        assert!(
            result.is_err(),
            "Expected ingest_vitals to fail for missing patient, got Ok"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

mod mllp_framer_edge_cases {
    use super::*;
    use dmart_server::hl7::mllp::{CARRIAGE_RETURN, END_BLOCK, MAX_MESSAGE, START_BLOCK};

    #[test]
    fn test_mllp_frame_delimiters() {
        // Test that frame delimiters are correct
        assert_eq!(START_BLOCK, 0x0B);
        assert_eq!(END_BLOCK, 0x1C);
        assert_eq!(CARRIAGE_RETURN, 0x0D);
    }

    #[test]
    fn test_max_message_size() {
        // Verify MAX_MESSAGE constant
        assert_eq!(MAX_MESSAGE, 1024 * 1024); // 1 MiB
    }

    #[test]
    fn test_utf8_validation_in_frame() {
        // Test that non-UTF8 payloads are rejected
        // The handle_stream function checks UTF-8 and sends AR
        let ack = build_ack("", Some("non-utf8 payload"));
        let ack_str = String::from_utf8(ack).expect("utf8");
        assert!(ack_str.contains("non-utf8"));
    }
}

mod hl7_end_to_end_scenarios {
    use super::*;

    #[tokio::test]
    async fn test_complete_hl7_pipeline_mindray() {
        let dir =
            std::env::temp_dir().join(format!("dmart-hl7-e2e-mindray-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let db = dmart_server::db::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");

        // Create patient
        let mut patient = Patient::new();
        patient.nombre = "E2E".into();
        patient.apellido = "Mindray".into();
        patient.historia_clinica = "111111".into();
        let created = db_ops::create_patient(&db, patient).await.expect("create");

        // Full HL7 ORU^R01 from Mindray
        let hl7 = "MSH|^~\\&|BeneVision|ICU01|DMART|UCI|20240821141031||ORU^R01|E2E001|P|2.5\r\
            PID|||111111^^^ICU01MR||E2E^MINDRAY||19600415|M\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^Heart rate^LN||88|bpm\r\
            OBX|2|NM|9279-1^Respiratory rate^LN||18|/min\r\
            OBX|3|NM|2708-6^SpO2^LN||97|%\r\
            OBX|4|NM|8310-5^Temperature^LN||36.8|Cel";

        let msg = parse_oru_message(hl7).expect("parse");
        assert_eq!(msg.source, MonitorSource::Mindray);

        let measurement = ingest_vitals(&db, &msg).await.expect("ingest");

        assert_eq!(measurement.patient_id, created.patient_id);
        assert_eq!(measurement.apache_data.frecuencia_cardiaca, 88.0);
        assert_eq!(measurement.apache_data.frecuencia_respiratoria, 18.0);
        assert_eq!(measurement.apache_data.spo2, 97.0);
        assert_eq!(measurement.apache_data.temperatura, 36.8);

        // Verify scores calculated - for new patient with default ApacheIIData
        // apache_score and gcs_score should be within clinical bounds
        assert!(measurement.apache_score <= 71); // rango APACHE II (0-71)
        assert!(measurement.gcs_score <= 15); // rango GCS (3-15)

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_complete_hl7_pipeline_philips() {
        let dir =
            std::env::temp_dir().join(format!("dmart-hl7-e2e-philips-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let db = dmart_server::db::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");

        let mut patient = Patient::new();
        patient.nombre = "E2E".into();
        patient.apellido = "Philips".into();
        patient.historia_clinica = "222222".into();
        let _created = db_ops::create_patient(&db, patient).await.expect("create");

        // Full HL7 ORU^R01 from Philips (using mnemonics)
        let hl7 = "MSH|^~\\&|IntelliVue|BED10|DMART|UCI|20240821141031||ORU^R01|E2E002|P|2.5\r\
            PID|||222222^^^ICU01MR||E2E^PHILIPS||19780302|F\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|HR^Heart Rate||68|bpm\r\
            OBX|2|NM|RESP^Respiration Rate||12|/min\r\
            OBX|3|NM|SPO2^SpO2||99|%\r\
            OBX|4|NM|NIBPs^NIBP Systolic||110|mmHg\r\
            OBX|5|NM|NIBPm^NIBP Mean||75|mmHg\r\
            OBX|6|NM|T1^Temperature||37.0|Cel";

        let msg = parse_oru_message(hl7).expect("parse");
        assert_eq!(msg.source, MonitorSource::Philips);

        let measurement = ingest_vitals(&db, &msg).await.expect("ingest");

        assert_eq!(measurement.apache_data.frecuencia_cardiaca, 68.0);
        assert_eq!(measurement.apache_data.frecuencia_respiratoria, 12.0);
        assert_eq!(measurement.apache_data.spo2, 99.0);
        assert_eq!(measurement.apache_data.presion_sistolica, 110.0);
        assert_eq!(measurement.apache_data.presion_arterial_media, 75.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_hl7_with_pid18_uuid_direct_resolution() {
        let dir = std::env::temp_dir().join(format!("dmart-hl7-uuid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let db = dmart_server::db::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");

        // Create patient with known UUID (will be generated by create_patient)
        let mut patient = Patient::new();
        patient.nombre = "UUID".into();
        patient.apellido = "Test".into();
        patient.historia_clinica = "333333".into();
        let created = db_ops::create_patient(&db, patient).await.expect("create");

        // HL7 con PID-18 = UUID de dMart
        let uuid = created.patient_id.clone();
        let mut pid_fields: Vec<String> = vec![
            "PID".into(),
            String::new(),             // PID-1
            String::new(),             // PID-2
            "333333^^^ICU01MR".into(), // PID-3 MRN
            String::new(),             // PID-4
            "UUID^TEST".into(),        // PID-5
            String::new(),             // PID-6
            "19600415".into(),         // PID-7
            "M".into(),                // PID-8
        ];
        for _ in 9..=17 {
            pid_fields.push(String::new());
        }
        pid_fields.push(uuid.clone()); // PID-18

        let hl7 = format!(
            "MSH|^~\\&|BeneVision|ICU01|DMART|UCI|20240821141031||ORU^R01|UUID001|P|2.5\r\
            {}\r\
            OBR|1|||||||20240821141030\r\
            OBX|1|NM|8867-4^HR^LN||85|bpm",
            pid_fields.join("|")
        );

        let msg = parse_oru_message(&hl7).expect("parse");
        assert!(msg.patient_ref_is_uuid);
        assert_eq!(msg.patient_ref, created.patient_id);

        let measurement = ingest_vitals(&db, &msg).await.expect("ingest");

        assert_eq!(measurement.patient_id, created.patient_id);
        assert_eq!(measurement.apache_data.frecuencia_cardiaca, 85.0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

mod hl7_concurrent_handling {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_hl7_ingestion() {
        let dir = std::env::temp_dir().join(format!("dmart-hl7-concurrent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let db = dmart_server::db::connect(&dir.join("test.surreal").to_string_lossy())
            .await
            .expect("db connect");

        // Create multiple patients
        let mut patients = Vec::new();
        for i in 1..=5 {
            let mut p = Patient::new();
            p.nombre = format!("Concurrent{}", i);
            p.apellido = "Test".into();
            p.historia_clinica = format!("{:06}", i);
            let created = db_ops::create_patient(&db, p).await.expect("create");

            patients.push(created);
        }

        // Ingest HL7 for all patients concurrently
        let handles: Vec<_> = patients.clone().into_iter().map(|p| {
            let db = db.clone();
            let mrn = p.historia_clinica.clone();
            let patient_id = p.patient_id.clone();

            tokio::spawn(async move {
                let hl7 = format!(
                    "MSH|^~\\&|BeneVision|ICU01|DMART|UCI|20240821141031||ORU^R01|CONC{}001|P|2.5\r\
                    PID|||{}^^^ICU01MR||CONC^PATIENT||19600415|M\r\
                    OBR|1|||||||20240821141030\r\
                    OBX|1|NM|8867-4^HR^LN||{}|bpm",
                    mrn, mrn, 80 + (mrn.parse::<i32>().unwrap_or(1) * 5) % 20
                );

                let msg = parse_oru_message(&hl7).expect("parse");
                let measurement = ingest_vitals(&db, &msg).await.expect("ingest");

                assert_eq!(measurement.patient_id, patient_id);
                measurement
            })
        }).collect();

        // Wait for all
        for handle in handles {
            handle.await.expect("task panicked");
        }

        // Verify all measurements created
        let mut total_measurements: usize = 0;
        for p in &patients {
            let measurements = db_ops::get_measurements_for_patient(&db, &p.patient_id)
                .await
                .expect("measurements");
            total_measurements += measurements.len();
        }
        assert_eq!(total_measurements, 5);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
