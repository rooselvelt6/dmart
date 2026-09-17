//! dmart-server — plataforma clínica UCI (SPEC-031). Superficie compartida: la lib declara
//! TODOS los módulos de integración; el bin usa `dmart_server::` y NO re-declara `mod`
//! (0 dead-code en bin, SPEC-031 cerrado).
//!
//! Cobertura llvm-cov (SPEC-027): parser 95 / mllp 100 / ingest 94.7 / scales 89.1 /
//! validation 100 / ml 95.9 / global 65.0 (umbral de producción 60). Los 38 tests de
//! integración/conformance (SPEC-028) ejercitan la superficie `hl7` (por eso su allow).

pub mod api;
pub mod audit;
pub mod auth;
pub mod cache;
#[allow(dead_code)] // SPEC-016: Clinical Decision Support rules (ejercitado por test cds_rules)
pub mod cds_rules;
pub mod crypto;
#[allow(dead_code)] // SPEC-018: Data Quality (ejercitado por test data_quality)
pub mod data_quality;
pub mod db;
#[allow(dead_code)] // SPEC-017: Device Registry (ejercitado por test device_registry)
pub mod device_registry;
#[allow(dead_code)] // SPEC-019: Alert Escalation (ejercitado por test escalation)
pub mod escalation;
#[allow(dead_code)] // SPEC-014: Early-Warning Streaming EWS (ejercitado por test ews_streaming)
pub mod ews_stream;
#[allow(dead_code)] // SPEC-013: FHIR R4 Bundle ingestion (ejercitado por test fhir_conformance)
pub mod fhir_bundle;
#[allow(dead_code)] // SPEC-031/028: superficie HL7 legacy ejercitada por los 38 tests de
// integración/conformance (SPEC-028, 32+6) y por la API externa del wire-format MLLP/ORU.
// NO es código muerto: es la superficie de referencia de conformance HL7 (SPEC-028).
pub mod hl7;
pub mod ingest;
pub mod metrics;
pub mod mfa;
pub mod middleware;
pub mod migrations;
#[allow(dead_code)] // SPEC-032: ML Serving (ejercitado por test ml_serving)
pub mod ml_serving;
#[allow(dead_code)] // SPEC-045: TimesFM forecasting (sidecar opcional + fallback naive)
pub mod forecasting;
pub mod observability;
#[allow(dead_code)] // SPEC-015: Patient Timeline API (ejercitado por test patient_timeline)
pub mod patient_timeline;
pub mod rbac;
pub mod realtime;
#[allow(dead_code)] // SPEC-030: Retención/downsampling (job + API; ejercitado por test retention)
pub mod retention;
pub mod security;
pub mod server_ingest;
#[allow(dead_code)] // SPEC-033: Patient Similarity Engine (ejercitado por test ml_similarity)
pub mod similarity;
#[allow(dead_code)] // SPEC-020: Tele-ICU (ejercitado por test teleicu)
pub mod teleicu;
#[allow(dead_code)] // SPEC-025: Multi-tenancy (ejercitado por test multi_tenant)
pub mod tenant;
