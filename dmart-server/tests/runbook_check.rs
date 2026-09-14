//! SPEC-011 — Runbook / On-Call (heredero de SPEC-008 alertas + SPEC-009 backup).
//! Gate: los 5 incidentes existen en docs/runbook/incidents/ y **cada uno** contiene
//! las 4 secciones obligatorias: Síntoma, Comando, Verificar, Escalar.

use std::fs;

const INCIDENTS_DIR: &str = "../docs/runbook/incidents";
const REQUIRED_SECTIONS: [&str; 4] = ["## Síntoma", "## Comando", "## Verificar", "## Escalar"];
const INCIDENTS: [&str; 5] = [
    "mllp-down.md",
    "backpressure.md",
    "disk-backup.md",
    "dr-restore.md",
    "auth-lockout.md",
];

#[test]
fn runbook_incidents_all_exist_and_complete() {
    for inc in INCIDENTS {
        let path = format!("{INCIDENTS_DIR}/{inc}");
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("SPEC-011: incidente {inc} NO existe en {path}"));
        for sec in REQUIRED_SECTIONS {
            assert!(
                content.contains(sec),
                "SPEC-011: incidente {inc} le falta sección '{sec}'"
            );
        }
        assert!(
            content.len() > 200,
            "SPEC-011: incidente {inc} parece vacío (len={})",
            content.len()
        );
    }
}

#[test]
fn runbook_index_readme_exists_and_links_all() {
    let readme = fs::read_to_string("../docs/runbook/README.md")
        .expect("SPEC-011: docs/runbook/README.md debe existir");
    for inc in INCIDENTS {
        assert!(
            readme.contains(&format!("incidents/{inc}")),
            "SPEC-011: README no enlaza {inc}"
        );
    }
    assert!(readme.contains("SPEC-011"));
    assert!(readme.contains("15 min"));
}
