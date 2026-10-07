//! Tests de conformidad clínica — SPEC-028.
//!
//! Valida que los algoritmos de puntuación coincidan EXACTAMENTE con los
//! vectores de referencia publicados y citados en `testdata/scales/`.
//! Un fallo aquí impide declarar DONE una escala (gate del ROADMAP).

use dmart_shared::models::News2Level;
use dmart_shared::scales::{
    apache_ii_breakdown, calculate_apache_ii_score, calculate_gcs_score, calculate_news2_score,
    calculate_saps_iii_score, calculate_sofa_score, saps_iii_mortality_prediction,
    sofa_mortality_estimate,
};
use dmart_shared::testdata::{
    diff_apache_ii_subscores, load_apache_ii_fixtures, load_gcs_fixtures, load_news2_fixtures,
    load_saps3_fixtures, load_sofa_fixtures,
};

const MIN_APACHE_II_FIXTURES: usize = 12;
const MIN_GCS_FIXTURES: usize = 5;
const MIN_NEWS2_FIXTURES: usize = 5;
const MIN_SOFA_FIXTURES: usize = 4;
const MIN_SAPS3_FIXTURES: usize = 4;

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

#[test]
fn test_conformance_news2_has_enough_vectors() {
    let fixtures = load_news2_fixtures().expect("loader NEWS2 debe funcionar");
    assert!(
        fixtures.len() >= MIN_NEWS2_FIXTURES,
        "NEWS2 requiere al menos {} vectores con cita (RCP 2017), hay {}",
        MIN_NEWS2_FIXTURES,
        fixtures.len()
    );
}

#[test]
fn test_conformance_sofa_has_enough_vectors() {
    let fixtures = load_sofa_fixtures().expect("loader SOFA debe funcionar");
    assert!(
        fixtures.len() >= MIN_SOFA_FIXTURES,
        "SOFA requiere al menos {} vectores con cita (Vincent 1996), hay {}",
        MIN_SOFA_FIXTURES,
        fixtures.len()
    );
}

#[test]
fn test_conformance_news2_exact_match() {
    let fixtures = load_news2_fixtures().expect("loader NEWS2 debe funcionar");

    for fixture in fixtures {
        let total = calculate_news2_score(&fixture.inputs);
        assert_eq!(
            total, fixture.expected.news2,
            "Fixture '{}' ({}) fuera de conformidad: NEWS2 calculado={}, esperado={}",
            fixture.id, fixture.source, total, fixture.expected.news2
        );

        if let Some(level) = &fixture.expected.level {
            let actual = News2Level::from_score(total).label().to_string();
            assert_eq!(
                actual, *level,
                "Fixture '{}': nivel NEWS2 calculado='{}', esperado='{}'",
                fixture.id, actual, level
            );
        }
    }
}

#[test]
fn test_conformance_sofa_exact_match() {
    let fixtures = load_sofa_fixtures().expect("loader SOFA debe funcionar");

    for fixture in fixtures {
        let total = calculate_sofa_score(&fixture.inputs);
        assert_eq!(
            total, fixture.expected.sofa,
            "Fixture '{}' ({}) fuera de conformidad: SOFA calculado={}, esperado={}",
            fixture.id, fixture.source, total, fixture.expected.sofa
        );

        if let Some(mort) = fixture.expected.mortality {
            let actual = sofa_mortality_estimate(total);
            assert_eq!(
                actual, mort,
                "Fixture '{}': mortalidad SOFA calculada={}, esperada={}",
                fixture.id, actual, mort
            );
        }
    }
}

#[test]
fn test_conformance_news2_sofa_fixtures_cite_sources() {
    let news2 = load_news2_fixtures().expect("loader NEWS2");
    let sofa = load_sofa_fixtures().expect("loader SOFA");

    for fixture in news2 {
        assert!(
            !fixture.source.trim().is_empty(),
            "Fixture NEWS2 {} no cita fuente bibliográfica",
            fixture.id
        );
        assert!(
            fixture.source.contains("2017"),
            "Fixture NEWS2 {} debe citar RCP 2017",
            fixture.id
        );
        assert!(
            fixture.expected.news2 <= 20,
            "Fixture NEWS2 {} excede el maximo 20",
            fixture.id
        );
    }
    for fixture in sofa {
        assert!(
            !fixture.source.trim().is_empty(),
            "Fixture SOFA {} no cita fuente bibliográfica",
            fixture.id
        );
        assert!(
            fixture.source.contains("1996") || fixture.source.contains("Ferreira"),
            "Fixture SOFA {} debe citar Vincent et al. 1996 o Ferreira et al. 2001",
            fixture.id
        );
        assert!(
            fixture.expected.sofa <= 24,
            "Fixture SOFA {} excede el maximo 24",
            fixture.id
        );
    }
}

#[test]
fn test_conformance_saps3_has_enough_vectors() {
    let fixtures = load_saps3_fixtures().expect("loader SAPS3 debe funcionar");
    assert!(
        fixtures.len() >= MIN_SAPS3_FIXTURES,
        "SAPS3 (variante dMart) requiere al menos {} vectores con cita (Moreno 2005), hay {}",
        MIN_SAPS3_FIXTURES,
        fixtures.len()
    );
}

#[test]
fn test_conformance_saps3_exact_match() {
    let fixtures = load_saps3_fixtures().expect("loader SAPS3 debe funcionar");

    for fixture in fixtures {
        let total = calculate_saps_iii_score(&fixture.inputs);
        assert_eq!(
            total, fixture.expected.saps3,
            "Fixture '{}' ({}) fuera de conformidad: SAPS3 calculado={}, esperado={}",
            fixture.id, fixture.source, total, fixture.expected.saps3
        );

        if let Some(mort) = fixture.expected.mortality {
            let actual = saps_iii_mortality_prediction(total);
            assert_eq!(
                actual, mort,
                "Fixture '{}': mortalidad SAPS3 calculada={}, esperada={}",
                fixture.id, actual, mort
            );
        }
    }
}

#[test]
fn test_conformance_saps3_fixtures_cite_sources() {
    let saps3 = load_saps3_fixtures().expect("loader SAPS3");

    for fixture in saps3 {
        assert!(
            !fixture.source.trim().is_empty(),
            "Fixture SAPS3 {} no cita fuente bibliográfica",
            fixture.id
        );
        assert!(
            fixture.source.contains("2005"),
            "Fixture SAPS3 {} debe citar Moreno et al. 2005",
            fixture.id
        );
        assert!(
            fixture.expected.saps3 <= 145,
            "Fixture SAPS3 {} excede el maximo 145 de la variante dMart",
            fixture.id
        );
    }
}
