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
pub mod db;
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
pub mod observability;
#[allow(dead_code)] // SPEC-015: Patient Timeline API (ejercitado por test patient_timeline)
pub mod patient_timeline;
pub mod rbac;
pub mod realtime;
pub mod security;
pub mod server_ingest;
