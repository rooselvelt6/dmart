//! SPEC-010 — Restore / DR (heredero de SPEC-009 backup).
//! Gate: el plan DR (`tests/fixtures/dr_plan.json`) contiene 6 pasos de runbook,
//! hash de integridad con fingerprint SPEC-029, RTO ≤ 15 min y RPO 0.
//! Bin NO expone API nueva (SPEC-031: sin dead-code bin).

use std::fs;

const DR_PLAN: &str = "tests/fixtures/dr_plan.json";
const RUNBOOK_DR: &str = "../docs/runbook/incidents/dr-restore.md";

#[test]
fn dr_plan_has_6_recovery_steps() {
    let plan = fs::read_to_string(DR_PLAN).expect("SPEC-010: fixture dr_plan.json debe existir");
    let v: serde_json::Value = serde_json::from_str(&plan).expect("dr_plan.json válido JSON");
    let steps = v["steps"].as_array().expect("steps array").len();
    assert_eq!(
        steps, 6,
        "SPEC-010: el plan debe tener 6 pasos de restauración"
    );
    assert_eq!(v["rto_minutes"].as_u64(), Some(15), "RTO objetivo = 15 min");
    assert_eq!(v["rpo_minutes"].as_u64(), Some(0), "RPO 0 declarado");
}

#[test]
fn dr_plan_fingerprint_gate_matches_spec029() {
    let plan = fs::read_to_string(DR_PLAN).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan).unwrap();
    let gate = v["integrity"]["gate"].as_str().unwrap();
    let expected = v["integrity"]["expected"].as_str().unwrap();
    assert_eq!(gate, "fingerprint_dump_matches");
    assert!(expected.contains("100% match con fingerprint SPEC-029"));
}

#[test]
fn runbook_dr_has_4_sections() {
    let rb = fs::read_to_string(RUNBOOK_DR).unwrap();
    for sec in ["## Síntoma", "## Comando", "## Verificar", "## Escalar"] {
        assert!(rb.contains(sec), "SPEC-010: runbook DR debe tener '{sec}'");
    }
    assert!(
        rb.contains("SPEC-031"),
        "SPEC-010: el restore debe re-anclar MLLP (SPEC-031)"
    );
}
