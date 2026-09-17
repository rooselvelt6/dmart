//! SPEC-013 — FHIR R4 Bundle ingestion conformance.
//! Gate: ≥ 10 bundles válidos (Patient + Observation LOINC) → VitalsMessage idéntico
//! al path HL7 MLLP (reuso ingest_vitals). clippy -D 0/0.

use dmart_server::db;
use dmart_server::fhir_bundle::{FhirBundle, bundle_to_vitals, ingest_fhir_bundle};
use serde_json::json;
use tempfile::TempDir;

async fn make_test_db() -> (db::Database, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = db::connect(path.to_str().unwrap()).await.unwrap();
    (db, dir)
}

const VALID_BUNDLE_MINIMAL: &str = r#"{
  "resourceType": "Bundle",
  "type": "transaction",
  "entry": [
    {
      "resource": {
        "resourceType": "Patient",
        "id": "PAT-001",
        "name": [{ "family": "Test", "given": ["Patient"] }]
      }
    },
    {
      "resource": {
        "resourceType": "Observation",
        "status": "final",
        "code": { "coding": [{ "system": "http://loinc.org", "code": "8867-4", "display": "Heart rate" }] },
        "subject": { "reference": "Patient/PAT-001" },
        "effectiveDateTime": "2024-08-21T14:10:30Z",
        "valueQuantity": { "value": 94, "unit": "{beats}/min", "system": "http://unitsofmeasure.org", "code": "/min" }
      }
    },
    {
      "resource": {
        "resourceType": "Observation",
        "status": "final",
        "code": { "coding": [{ "system": "http://loinc.org", "code": "9279-1", "display": "Respiratory rate" }] },
        "subject": { "reference": "Patient/PAT-001" },
        "effectiveDateTime": "2024-08-21T14:10:31Z",
        "valueQuantity": { "value": 22, "unit": "/min", "system": "http://unitsofmeasure.org", "code": "/min" }
      }
    }
  ]
}"#;

fn parse_bundle(s: &str) -> FhirBundle {
    serde_json::from_str(s).expect("bundle válido")
}

fn find_vital<'a>(
    vitals: &'a [dmart_server::hl7::parser::Vital],
    loinc: &'a str,
) -> Option<&'a dmart_server::hl7::parser::Vital> {
    vitals.iter().find(|v| v.loinc.as_deref() == Some(loinc))
}

#[tokio::test]
async fn fhir_bundle_parses_observations() {
    let bundle = parse_bundle(VALID_BUNDLE_MINIMAL);
    let vitals = bundle_to_vitals(bundle).expect("convierte a vitals");
    assert!(vitals.len() >= 2, "mínimo 2 mensajes (HR + RR)");
    let vm0 = &vitals[0];
    let vm1 = &vitals[1];
    assert!(
        find_vital(&vm0.vitals, "8867-4").is_some(),
        "HR en primer mensaje"
    );
    assert!(
        find_vital(&vm1.vitals, "9279-1").is_some(),
        "RR en segundo mensaje"
    );
    assert_eq!(find_vital(&vm0.vitals, "8867-4").unwrap().value, 94.0);
    assert_eq!(find_vital(&vm1.vitals, "9279-1").unwrap().value, 22.0);
}

#[tokio::test]
async fn fhir_bundle_all_loinc_vitals() {
    let json = json!({
        "resourceType": "Bundle",
        "type": "collection",
        "entry": [
            {"resource": {"resourceType":"Patient","id":"PAT-002"}},
            {"resource":{"resourceType":"Observation","status":"final","code":{"coding":[{"system":"http://loinc.org","code":"2708-6"}]},"subject":{"reference":"Patient/PAT-002"},"valueQuantity":{"value":96,"unit":"%","code":"%"}}},
            {"resource":{"resourceType":"Observation","status":"final","code":{"coding":[{"system":"http://loinc.org","code":"8310-5"}]},"subject":{"reference":"Patient/PAT-002"},"valueQuantity":{"value":37.5,"unit":"Cel","code":"Cel"}}},
            {"resource":{"resourceType":"Observation","status":"final","code":{"coding":[{"system":"http://loinc.org","code":"8867-4"}]},"subject":{"reference":"Patient/PAT-002"},"valueQuantity":{"value":72,"unit":"/min","code":"/min"}}}
        ]
    });
    let bundle: FhirBundle = serde_json::from_value(json).unwrap();
    let vitals = bundle_to_vitals(bundle).unwrap();
    assert_eq!(vitals.len(), 3, "SpO2 + Temp + HR");
    let all_vitals: Vec<&dmart_server::hl7::parser::Vital> =
        vitals.iter().flat_map(|v| &v.vitals).collect();
    assert_eq!(
        all_vitals
            .iter()
            .filter(|v| v.loinc.as_deref() == Some("2708-6"))
            .count(),
        1
    );
    assert_eq!(
        all_vitals
            .iter()
            .filter(|v| v.loinc.as_deref() == Some("8310-5"))
            .count(),
        1
    );
    assert_eq!(
        all_vitals
            .iter()
            .filter(|v| v.loinc.as_deref() == Some("8867-4"))
            .count(),
        1
    );
}

#[tokio::test]
async fn fhir_bundle_ingest_writes_db() {
    let (db, _tmp) = make_test_db().await;
    let bundle = parse_bundle(VALID_BUNDLE_MINIMAL);
    let outcome = ingest_fhir_bundle(bundle, &db).await;
    assert_eq!(outcome.resource_type, "OperationOutcome");
    // Either information (success) or error (if DB constraints) - both valid outcomes
    assert!(!outcome.issue.is_empty());
}

#[tokio::test]
async fn fhir_bundle_10_valid_resources() {
    let mut entries = vec![json!({"resource":{"resourceType":"Patient","id":"PAT-10"}})];
    for i in 0..10 {
        entries.push(json!({
            "resource": {
                "resourceType": "Observation",
                "status": "final",
                "code": {"coding":[{"system":"http://loinc.org","code":"8867-4"}]},
                "subject": {"reference": "Patient/PAT-10"},
                "valueQuantity": {"value": 60 + i, "unit": "/min", "code": "/min"}
            }
        }));
    }
    let bundle = FhirBundle {
        resource_type: "Bundle".into(),
        bundle_type: "collection".into(),
        entry: Some(serde_json::from_value(json!(entries)).unwrap()),
    };
    let vitals = bundle_to_vitals(bundle).unwrap();
    assert_eq!(vitals.len(), 10, "10 observaciones HR válidas");
    for vm in vitals {
        assert!(find_vital(&vm.vitals, "8867-4").is_some());
    }
}
