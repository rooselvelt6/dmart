//! PropTest: Property-based testing para parser HL7

#[cfg(test)]
mod tests {
    use crate::hl7::parser::*;
    use proptest::prelude::*;
    use proptest::test_runner::{Config, TestRunner};

    // Helper para rangos f32
    fn f32_range(min: f32, max: f32) -> impl Strategy<Value = f32> {
        (min..max).prop_map(move |v| v)
    }

    // Genera un MSH segment válido
    fn arb_msh() -> impl Strategy<Value = String> {
        (
            "[A-Z]{3,10}",    // sending_app
            "[A-Z]{3,10}",    // sending_facility
            "[A-Z]{3,10}",    // receiving_app
            "[A-Z]{3,10}",    // receiving_facility
            "[0-9]{14}",      // timestamp
            "[A-Z0-9]{5,15}", // message_id
        )
            .prop_map(
                |(
                    sending_app,
                    sending_facility,
                    receiving_app,
                    receiving_facility,
                    timestamp,
                    message_id,
                )| {
                    format!(
                        "MSH|^~\\&|{}|{}|{}|{}|{}||ORU^R01|{}|P|2.5",
                        sending_app,
                        sending_facility,
                        receiving_app,
                        receiving_facility,
                        timestamp,
                        message_id
                    )
                },
            )
    }

    // Genera un PID segment válido
    fn arb_pid() -> impl Strategy<Value = String> {
        (
            "[A-Z0-9]{5,15}", // MRN
            "[A-Z]{3,10}",    // apellido
            "[A-Z]{3,10}",    // nombre
            "[0-9]{8}",       // fecha_nacimiento
            "[MF]",           // sexo
        )
            .prop_map(|(mrn, apellido, nombre, fecha_nac, sexo)| {
                format!(
                    "PID|||{}^^^HOSP||{} ^{}||{}|{}",
                    mrn, apellido, nombre, fecha_nac, sexo
                )
            })
    }

    // Genera un OBX segment con LOINC válido
    fn arb_obx_loinc() -> impl Strategy<Value = String> {
        prop_oneof![
            f32_range(0.0, 300.0).prop_map(move |val| format!("OBX|1|NM|8867-4^Heart rate^LN||{}|{{beats}}/min||||||F|||20240821141030", val)),
            f32_range(0.0, 60.0).prop_map(move |val| format!("OBX|1|NM|9279-1^Respiratory rate^LN||{}|/min||||||F|||20240821141030", val)),
            f32_range(0.0, 100.0).prop_map(move |val| format!("OBX|1|NM|2708-6^Oxygen saturation in Arterial blood^LN||{}|%||||||F|||20240821141030", val)),
            f32_range(30.0, 44.0).prop_map(move |val| format!("OBX|1|NM|8310-5^Body temperature^LN||{}|Cel||||||F|||20240821141030", val)),
            f32_range(0.0, 300.0).prop_map(move |val| format!("OBX|1|NM|8480-6^Systolic BP^LN||{}|mmHg||||||F|||20240821141030", val)),
            f32_range(0.0, 250.0).prop_map(move |val| format!("OBX|1|NM|8478-0^Mean arterial pressure^LN||{}|mmHg||||||F|||20240821141030", val)),
            f32_range(0.0, 250.0).prop_map(move |val| format!("OBX|1|NM|8460-8^Diastolic BP^LN||{}|mmHg||||||F|||20240821141030", val)),
        ]
    }

    // Genera un OBX segment con mnemónico de fabricante
    fn arb_obx_mnemonic() -> impl Strategy<Value = String> {
        prop_oneof![
            f32_range(40.0, 180.0).prop_map(move |val| format!(
                "OBX|1|NM|HR^Heart Rate||{}|bpm||||||F|||20240821141030",
                val
            )),
            f32_range(5.0, 50.0).prop_map(move |val| format!(
                "OBX|1|NM|RESP^Respiration Rate||{}|/min||||||F|||20240821141030",
                val
            )),
            f32_range(70.0, 100.0).prop_map(move |val| format!(
                "OBX|1|NM|SPO2^SpO2||{}|%||||||F|||20240821141030",
                val
            )),
            f32_range(35.0, 42.0).prop_map(move |val| format!(
                "OBX|1|NM|T1^Temperature||{}|Cel||||||F|||20240821141030",
                val
            )),
            f32_range(80.0, 200.0).prop_map(move |val| format!(
                "OBX|1|NM|NIBPs^NIBP Systolic||{}|mmHg||||||F|||20240821141030",
                val
            )),
            f32_range(50.0, 130.0).prop_map(move |val| format!(
                "OBX|1|NM|NIBPm^NIBP Mean||{}|mmHg||||||F|||20240821141030",
                val
            )),
            f32_range(40.0, 120.0).prop_map(move |val| format!(
                "OBX|1|NM|NIBPd^NIBP Diastolic||{}|mmHg||||||F|||20240821141030",
                val
            )),
        ]
    }

    // Genera un mensaje ORU^R01 completo válido
    fn arb_oru_message() -> impl Strategy<Value = String> {
        (
            arb_msh(),
            arb_pid(),
            prop::collection::vec(prop_oneof![arb_obx_loinc(), arb_obx_mnemonic()], 1..=10),
        )
            .prop_map(|(msh, pid, obxs)| {
                let obr = "OBR|1|||||||20240821141030";
                let segments = vec![msh, pid, obr.to_string()];
                let all: Vec<String> = segments.into_iter().chain(obxs).collect();
                all.join("\r")
            })
    }

    // Genera mensajes HL7 malformados para test de robustez
    fn arb_malformed_hl7() -> impl Strategy<Value = String> {
        prop_oneof![
            // MSH faltante
            (arb_pid(), prop::collection::vec(arb_obx_loinc(), 1..=5))
                .prop_map(|(pid, obxs)| {
                    let obr = "OBR|1|||||||20240821141030";
                    vec![pid, obr.to_string()].into_iter().chain(obxs).collect::<Vec<_>>().join("\r")
                }),
            // PID faltante
            (arb_msh(), prop::collection::vec(arb_obx_loinc(), 1..=5))
                .prop_map(|(msh, obxs)| {
                    let obr = "OBR|1|||||||20240821141030";
                    vec![msh, obr.to_string()].into_iter().chain(obxs).collect::<Vec<_>>().join("\r")
                }),
            // OBX sin valor numérico
            (arb_msh(), arb_pid(), prop::collection::vec(
                Just("OBX|1|NM|8867-4^Heart rate^LN||not_a_number|{beats}/min||||||F|||20240821141030".to_string()), 1..=3))
                .prop_map(|(msh, pid, obxs)| {
                    let obr = "OBR|1|||||||20240821141030";
                    vec![msh, pid, obr.to_string()].into_iter().chain(obxs).collect::<Vec<_>>().join("\r")
                }),
            // Mensaje vacío
            Just("".to_string()),
            // Solo basura
            "[A-Z|^~&]{0,50}",
        ]
    }

    #[test]
    fn parse_valid_oru_message_does_not_panic() {
        let mut runner = TestRunner::new(Config::with_cases(100));
        let strategy = arb_oru_message();
        runner
            .run(&strategy, |msg| {
                let _ = parse_oru_message(&msg);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn parse_valid_oru_returns_vitals() {
        let mut runner = TestRunner::new(Config::with_cases(100));
        let strategy = arb_oru_message();
        runner
            .run(&strategy, |msg| {
                if let Ok(parsed) = parse_oru_message(&msg) {
                    prop_assert!(
                        !parsed.vitals.is_empty(),
                        "Parsed message should have vitals"
                    );
                    for v in &parsed.vitals {
                        prop_assert!(v.loinc.is_some(), "Vital should have LOINC code");
                        prop_assert!(
                            !v.loinc.as_ref().unwrap().is_empty(),
                            "LOINC should not be empty"
                        );
                        prop_assert!(v.value.is_finite(), "Vital value must be finite");
                    }
                    prop_assert!(
                        !parsed.patient_ref.is_empty(),
                        "Patient ref should not be empty"
                    );
                    prop_assert!(
                        parsed.timestamp.contains('T'),
                        "Timestamp should be RFC3339"
                    );
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn parse_malformed_hl7_does_not_panic() {
        let mut runner = TestRunner::new(Config::with_cases(100));
        let strategy = arb_malformed_hl7();
        runner
            .run(&strategy, |msg| {
                let _ = parse_oru_message(&msg);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn hl7_datetime_conversion_valid() {
        let mut runner = TestRunner::new(Config::with_cases(100));
        let strategy = (
            2000..2099u32,
            1..=12u32,
            1..=28u32,
            0..=23u32,
            0..=59u32,
            0..=59u32,
        );
        runner
            .run(&strategy, |(year, month, day, hour, min, sec)| {
                let dt = format!(
                    "{:04}{:02}{:02}{:02}{:02}{:02}",
                    year, month, day, hour, min, sec
                );
                let result = hl7_datetime_to_rfc3339(&dt);
                prop_assert!(result.is_some(), "Valid datetime should parse");
                let rfc = result.unwrap();
                prop_assert!(rfc.contains('T'), "Should be RFC3339 format");
                prop_assert!(rfc.ends_with('Z'), "Should be UTC (Z suffix)");
                Ok(())
            })
            .unwrap();
    }
}
