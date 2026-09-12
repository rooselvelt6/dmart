//! Integración de monitores de cama: HL7 v2 (ORU^R01) + transporte MLLP.
//!
//! ```text
//! Monitor (Mindray/Philips)
//!    │  MLLP (0x0B … 0x1C 0x0D)      │   HTTP POST /api/monitores/hl7
//!    ▼                               ▼
//! [mllp::serve]              [api::monitores::hl7_ingest]
//!    └──► parser::parse_oru_message ──► ingest::ingest_vitals ──► DB + SSE
//! ```
//!
//! Los fabricantes emiten `OBX` con códigos LOINC o mnemónicos; el parser los
//! normaliza a LOINC y los fusiona en `ApacheIIData`. Para MQTT, un broker
//! puede reenviar el mismo payload HL7 a `ingest_vitals` (el mapeo es
//! idéntico); la documentación está en `../docs/hl7-integracion.md`.

pub mod ingest;
pub mod mllp;
pub mod parser;
pub mod proptests;
