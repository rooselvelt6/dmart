use crate::hl7::ingest::ingest_vitals;
use crate::hl7::parser::{MonitorSource, Vital, VitalsMessage};
use dmart_shared::models::Patient;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct FhirBundle {
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    #[serde(rename = "type")]
    pub bundle_type: String,
    pub entry: Option<Vec<BundleEntry>>,
}

#[derive(Debug, Deserialize)]
pub struct BundleEntry {
    pub resource: Option<FhirResource>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "resourceType")]
#[allow(clippy::large_enum_variant)]
pub enum FhirResource {
    Patient(Patient),
    Observation(FhirObservation),
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FhirObservation {
    pub id: Option<String>,
    pub status: String,
    pub code: FhirCodeableConcept,
    pub subject: Option<FhirReference>,
    #[serde(rename = "effectiveDateTime")]
    pub effective_date_time: Option<String>,
    #[serde(rename = "valueQuantity")]
    pub value_quantity: Option<FhirQuantity>,
    pub component: Option<Vec<FhirObservation>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FhirCodeableConcept {
    pub coding: Option<Vec<FhirCoding>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FhirCoding {
    pub system: Option<String>,
    pub code: Option<String>,
    pub display: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FhirReference {
    pub reference: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FhirQuantity {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub system: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OperationOutcome {
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    pub issue: Vec<OperationIssue>,
}

#[derive(Debug, Serialize)]
pub struct OperationIssue {
    pub severity: String,
    pub code: String,
    pub details: Option<OperationDetails>,
    pub diagnostics: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OperationDetails {
    pub coding: Vec<FhirCoding>,
}

pub fn bundle_to_vitals(bundle: FhirBundle) -> Result<Vec<VitalsMessage>, String> {
    let mut vitals_msgs = Vec::new();
    let mut patient_ref = String::new();

    for entry in bundle.entry.unwrap_or_default() {
        if let Some(resource) = entry.resource {
            match resource {
                FhirResource::Patient(p) => patient_ref = p.id.clone().unwrap_or_default(),
                FhirResource::Observation(obs) => {
                    if let Some(vm) = observation_to_vitals_message(obs, &patient_ref)? {
                        vitals_msgs.push(vm);
                    }
                }
                FhirResource::Other => {}
            }
        }
    }
    Ok(vitals_msgs)
}

fn observation_to_vitals_message(
    obs: FhirObservation,
    patient_ref: &str,
) -> Result<Option<VitalsMessage>, String> {
    let code = obs
        .code
        .coding
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.code.as_deref());
    let system = obs
        .code
        .coding
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.system.as_deref());
    let value = obs.value_quantity.as_ref().and_then(|v| v.value);

    if system != Some("http://loinc.org") || value.is_none() {
        return Ok(None);
    }

    let loinc = code.unwrap().to_string();
    let vital = Vital {
        loinc: Some(loinc.clone()),
        name: loinc.clone(),
        value: value.unwrap() as f32,
        unit: obs
            .value_quantity
            .as_ref()
            .and_then(|v| v.unit.clone())
            .unwrap_or_default(),
    };

    let vm = VitalsMessage {
        message_id: uuid::Uuid::new_v4().to_string(),
        sender: "FHIR".into(),
        patient_ref: patient_ref.to_string(),
        patient_ref_is_uuid: false,
        timestamp: obs
            .effective_date_time
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        vitals: vec![vital],
        source: MonitorSource::Generic,
        sequence_number: None,
    };
    Ok(Some(vm))
}

#[instrument(skip_all)]
pub async fn ingest_fhir_bundle(bundle: FhirBundle, db: &crate::db::Database) -> OperationOutcome {
    let vitals = bundle_to_vitals(bundle).unwrap_or_default();
    let mut issues = Vec::new();
    for vm in vitals {
        if let Err(e) = ingest_vitals(db, &vm).await {
            issues.push(OperationIssue {
                severity: "error".into(),
                code: "processing".into(),
                details: None,
                diagnostics: Some(e.to_string()),
            });
        }
    }
    OperationOutcome {
        resource_type: "OperationOutcome".into(),
        issue: if issues.is_empty() {
            vec![OperationIssue {
                severity: "information".into(),
                code: "informational".into(),
                details: None,
                diagnostics: Some("Bundle processed successfully".into()),
            }]
        } else {
            issues
        },
    }
}
