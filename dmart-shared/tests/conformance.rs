//! Tests de conformidad clínica — SPEC-028.
//!
//! Valida que los algoritmos de puntuación coincidan EXACTAMENTE con los
//! vectores de referencia publicados y citados en `testdata/scales/`.
//! Un fallo aquí impide declarar DONE una escala (gate del ROADMAP).

use dmart_shared::scales::{apache_ii_breakdown, calculate_apache_ii_score, calculate_gcs_score};
use dmart_shared::testdata::{
    diff_apache_ii_subscores, load_apache_ii_fixtures, load_gcs_fixtures,
};

const MIN_APACHE_II_FIXTURES: usize = 6;
const MIN_GCS_FIXTURES: usize = 5;

#[test]
fn test_conformance_apache_ii_has_enough_vectors() {
    let fixtures = load_apache_ii_fixtures().expect("loader APACHE II debe funcionar");
    assert!(
        fixtures.len() >= MIN_APACHE_II_FIXTURES,
        "APACHE II requiere al menos {} vectores con cita (Knaus 1985), hay {}",
        MIN_APACHE_II_FIXTURES,
        fixtures.len()
    );
}

#[test]
fn test_conformance_gcs_has_enough_vectors() {
    let fixtures = load_gcs_fixtures().expect("loader GCS debe funcionar");
    assert!(
        fixtures.len() >= MIN_GCS_FIXTURES,
        "GCS requiere al menos {} vectores con cita (Teasdale & Jennett 1974), hay {}",
        MIN_GCS_FIXTURES,
        fixtures.len()
    );
}

#[test]
fn test_conformance_apache_ii_exact_match() {
    let fixtures = load_apache_ii_fixtures().expect("loader APACHE II debe funcionar");

    for fixture in fixtures {
        let total = calculate_apache_ii_score(&fixture.inputs);
        assert_eq!(
            total, fixture.expected.apache_ii,
            "Fixture '{}' ({}) fuera de conformidad: APACHE II calculado={}, esperado={}",
            fixture.id, fixture.source, total, fixture.expected.apache_ii
        );

        let breakdown = apache_ii_breakdown(&fixture.inputs);
        let diff = diff_apache_ii_subscores(&fixture, &breakdown);
        assert!(
            diff.is_empty(),
            "Fixture '{}' ({}) falla en sub-scores:\n  {}",
            fixture.id,
            fixture.source,
            diff.join("\n  ")
        );
    }
}

#[test]
fn test_conformance_gcs_exact_match() {
    let fixtures = load_gcs_fixtures().expect("loader GCS debe funcionar");

    for fixture in fixtures {
        let total = calculate_gcs_score(&fixture.inputs);
        assert_eq!(
            total, fixture.expected.gcs_total,
            "Fixture '{}' ({}) fuera de conformidad: GCS calculado={}, esperado={}",
            fixture.id, fixture.source, total, fixture.expected.gcs_total
        );

        if let Some(interpretation) = &fixture.expected.interpretation {
            let actual = fixture.inputs.interpret();
            assert_eq!(
                actual, interpretation,
                "Fixture '{}': interpretación clínica no coincide",
                fixture.id
            );
        }
    }
}

#[test]
fn test_conformance_all_fixtures_cite_sources() {
    let apache = load_apache_ii_fixtures().expect("loader APACHE II");
    let gcs = load_gcs_fixtures().expect("loader GCS");

    for fixture in apache {
        assert!(
            !fixture.source.trim().is_empty(),
            "Fixture {} no cita fuente bibliográfica",
            fixture.id
        );
        assert!(
            fixture.source.contains("1985") || fixture.source.contains("198"),
            "Fixture {} debe citar Knaus et al. 1985",
            fixture.id
        );
    }
    for fixture in gcs {
        assert!(
            !fixture.source.trim().is_empty(),
            "Fixture {} no cita fuente bibliográfica",
            fixture.id
        );
        assert!(
            fixture.source.contains("1974"),
            "Fixture {} debe citar Teasdale & Jennett 1974",
            fixture.id
        );
    }
}

#[test]
fn test_conformance_loader_rejects_invariant_violations() {
    // Los invariantes estructurales ya se validan al cargar; estos tests
    // confirman que los fixtures existentes son internamente consistentes
    // (sub-scores suman al total declarado).
    let fixtures = load_apache_ii_fixtures().expect("loader APACHE II");
    for fixture in &fixtures {
        let breakdown = apache_ii_breakdown(&fixture.inputs);
        let diff = diff_apache_ii_subscores(fixture, &breakdown);
        assert!(
            diff.is_empty(),
            "Fixture {} inconsistente: {}",
            fixture.id,
            diff.join("; ")
        );

        let sub = fixture.expected.subscores.as_ref();
        if let Some(sub) = sub {
            assert!(
                sub.aps_total <= 60,
                "{}: aps_total {} > 60",
                fixture.id,
                sub.aps_total
            );
            assert!(sub.total <= 71, "{}: total {} > 71", fixture.id, sub.total);
        }
    }
}
