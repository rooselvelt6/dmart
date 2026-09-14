use crate::models::{ApacheIIData, GcsData};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Loader de fixtures de conformidad clínica (SPEC-028).
///
/// Los vectores de referencia se versionan en `testdata/scales/<scale>/<NNN>-<nombre>.json`
/// y cada fixture cita la fuente bibliográfica de la que deriva sus valores esperados
/// (Knaus et al. 1985 para APACHE II, Teasdale & Jennett 1974 para GCS, etc.).
///
/// El formato canónico usa las claves internas de los DTO (ApacheIIData / GcsData)
/// para deserializar directamente con serde y evitar errores de traducción de unidades.

#[derive(Debug, Clone, Deserialize)]
pub struct ApacheIiFixture {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub comment: Option<String>,
    pub inputs: ApacheIIData,
    pub expected: ApacheIiExpected,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApacheIiExpected {
    pub apache_ii: u32,
    #[serde(default)]
    pub subscores: Option<ApacheIiSubscores>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApacheIiSubscores {
    pub temperatura: u32,
    pub pam: u32,
    pub fc: u32,
    pub fr: u32,
    pub oxigenacion: u32,
    pub ph: u32,
    pub sodio: u32,
    pub potasio: u32,
    pub creatinina: u32,
    pub hematocrito: u32,
    pub leucocitos: u32,
    pub gcs_pts: u32,
    pub aps_total: u32,
    pub edad_pts: u32,
    pub cronicas_pts: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GcsFixture {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub comment: Option<String>,
    pub inputs: GcsData,
    pub expected: GcsExpected,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GcsExpected {
    pub gcs_total: u8,
    #[serde(default)]
    pub interpretation: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FixtureError {
    DirectoryNotFound(PathBuf),
    ReadError(PathBuf, String),
    ParseError(PathBuf, String),
    InvalidInvariant(String),
    MissingFixtures(String),
}

impl std::fmt::Display for FixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixtureError::DirectoryNotFound(p) => {
                write!(f, "Directorio de fixtures no encontrado: {}", p.display())
            }
            FixtureError::ReadError(p, e) => {
                write!(f, "No se pudo leer fixtures {}: {}", p.display(), e)
            }
            FixtureError::ParseError(p, e) => {
                write!(f, "Fixture JSON inválido {}: {}", p.display(), e)
            }
            FixtureError::InvalidInvariant(msg) => write!(f, "Invariante violado: {}", msg),
            FixtureError::MissingFixtures(msg) => write!(f, "Fixtures insuficientes: {}", msg),
        }
    }
}

impl std::error::Error for FixtureError {}

/// Directorio base de testdata, derivado del manifest del crate.
fn testdata_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

fn load_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, FixtureError> {
    let bytes = fs::read(path).map_err(|e| FixtureError::ReadError(path.clone(), e.to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| FixtureError::ParseError(path.clone(), e.to_string()))
}

fn list_json_files(dir: &PathBuf) -> Result<Vec<PathBuf>, FixtureError> {
    let entries = fs::read_dir(dir)
        .map_err(|_| FixtureError::DirectoryNotFound(dir.clone()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    Ok(entries)
}

/// Carga todos los fixtures APACHE II y valida sus invariantes estructuralees.
pub fn load_apache_ii_fixtures() -> Result<Vec<ApacheIiFixture>, FixtureError> {
    let dir = testdata_dir().join("scales").join("apache_ii");
    let paths = list_json_files(&dir)?;
    let mut fixtures = Vec::new();
    for path in &paths {
        let fixture: ApacheIiFixture = load_json(path)?;
        validate_apache_ii_invariants(&fixture)?;
        fixtures.push(fixture);
    }
    if fixtures.is_empty() {
        return Err(FixtureError::MissingFixtures(
            "APACHE II: no hay fixtures en testdata".into(),
        ));
    }
    Ok(fixtures)
}

/// Carga todos los fixtures GCS y valida sus invariantes estructurales.
pub fn load_gcs_fixtures() -> Result<Vec<GcsFixture>, FixtureError> {
    let dir = testdata_dir().join("scales").join("gcs");
    let paths = list_json_files(&dir)?;
    let mut fixtures = Vec::new();
    for path in &paths {
        let fixture: GcsFixture = load_json(path)?;
        validate_gcs_invariants(&fixture)?;
        fixtures.push(fixture);
    }
    if fixtures.is_empty() {
        return Err(FixtureError::MissingFixtures(
            "GCS: no hay fixtures en testdata".into(),
        ));
    }
    Ok(fixtures)
}

fn validate_apache_ii_invariants(fixture: &ApacheIiFixture) -> Result<(), FixtureError> {
    let data = &fixture.inputs;
    if data.gcs_total != data.gcs_ojos + data.gcs_verbal + data.gcs_motor {
        return Err(FixtureError::InvalidInvariant(format!(
            "{}: gcs_total ({}) != gcs_ojos+verbal+motor ({}), fixture debe ser consistente",
            fixture.id,
            data.gcs_total,
            data.gcs_ojos + data.gcs_verbal + data.gcs_motor
        )));
    }
    if !(3..=15).contains(&data.gcs_total) {
        return Err(FixtureError::InvalidInvariant(format!(
            "{}: gcs_total fuera de rango 3-15: {}",
            fixture.id, data.gcs_total
        )));
    }
    if let Some(sub) = &fixture.expected.subscores {
        let maxes: [(&str, u32, u32); 12] = [
            ("temperatura", sub.temperatura, 4),
            ("pam", sub.pam, 4),
            ("fc", sub.fc, 4),
            ("fr", sub.fr, 4),
            ("oxigenacion", sub.oxigenacion, 4),
            ("ph", sub.ph, 4),
            ("sodio", sub.sodio, 4),
            ("potasio", sub.potasio, 4),
            ("creatinina", sub.creatinina, 8),
            ("hematocrito", sub.hematocrito, 4),
            ("leucocitos", sub.leucocitos, 4),
            ("gcs_pts", sub.gcs_pts, 12),
        ];
        for (name, val, max) in maxes {
            if val > max {
                return Err(FixtureError::InvalidInvariant(format!(
                    "{}: sub-score {} = {} excede el máximo {} de la escala",
                    fixture.id, name, val, max
                )));
            }
        }
        if sub.edad_pts > 6 {
            return Err(FixtureError::InvalidInvariant(format!(
                "{}: edad_pts {} excede 6",
                fixture.id, sub.edad_pts
            )));
        }
        if sub.cronicas_pts > 5 {
            return Err(FixtureError::InvalidInvariant(format!(
                "{}: cronicas_pts {} excede 5",
                fixture.id, sub.cronicas_pts
            )));
        }
        let recomputed_total = sub.temperatura
            + sub.pam
            + sub.fc
            + sub.fr
            + sub.oxigenacion
            + sub.ph
            + sub.sodio
            + sub.potasio
            + sub.creatinina
            + sub.hematocrito
            + sub.leucocitos
            + sub.gcs_pts
            + sub.edad_pts
            + sub.cronicas_pts;
        if recomputed_total != sub.total {
            return Err(FixtureError::InvalidInvariant(format!(
                "{}: la suma de sub-scores ({}) no coincide con total declarado ({})",
                fixture.id, recomputed_total, sub.total
            )));
        }
        if sub.total != fixture.expected.apache_ii {
            return Err(FixtureError::InvalidInvariant(format!(
                "{}: total del breakdown ({}) no coincide con apache_ii esperado ({})",
                fixture.id, sub.total, fixture.expected.apache_ii
            )));
        }
    }
    Ok(())
}

fn validate_gcs_invariants(fixture: &GcsFixture) -> Result<(), FixtureError> {
    let data = &fixture.inputs;
    if !(1..=4).contains(&data.apertura_ocular) {
        return Err(FixtureError::InvalidInvariant(format!(
            "{}: apertura_ocular fuera de 1-4: {}",
            fixture.id, data.apertura_ocular
        )));
    }
    if !(1..=5).contains(&data.respuesta_verbal) {
        return Err(FixtureError::InvalidInvariant(format!(
            "{}: respuesta_verbal fuera de 1-5: {}",
            fixture.id, data.respuesta_verbal
        )));
    }
    if !(1..=6).contains(&data.respuesta_motora) {
        return Err(FixtureError::InvalidInvariant(format!(
            "{}: respuesta_motora fuera de 1-6: {}",
            fixture.id, data.respuesta_motora
        )));
    }
    let total = data.apertura_ocular + data.respuesta_verbal + data.respuesta_motora;
    if total != fixture.expected.gcs_total {
        return Err(FixtureError::InvalidInvariant(format!(
            "{}: los componentes suman {}, pero gcs_total esperado es {}",
            fixture.id, total, fixture.expected.gcs_total
        )));
    }
    if !(3..=15).contains(&total) {
        return Err(FixtureError::InvalidInvariant(format!(
            "{}: gcs_total fuera de 3-15: {}",
            fixture.id, total
        )));
    }
    Ok(())
}

/// Genera un diff legible entre el breakdown calculado y el esperado,
/// para que un fallo de conformidad nombre escala, fixture y sub-score divergente.
pub fn diff_apache_ii_subscores(
    fixture: &ApacheIiFixture,
    computed: &crate::scales::ApacheIIBreakdown,
) -> Vec<String> {
    let Some(exp) = &fixture.expected.subscores else {
        return Vec::new();
    };
    let pairs: [(&str, u32, u32); 16] = [
        ("temperatura", computed.temperatura, exp.temperatura),
        ("pam", computed.pam, exp.pam),
        ("fc", computed.fc, exp.fc),
        ("fr", computed.fr, exp.fr),
        ("oxigenacion", computed.oxigenacion, exp.oxigenacion),
        ("ph", computed.ph, exp.ph),
        ("sodio", computed.sodio, exp.sodio),
        ("potasio", computed.potasio, exp.potasio),
        ("creatinina", computed.creatinina, exp.creatinina),
        ("hematocrito", computed.hematocrito, exp.hematocrito),
        ("leucocitos", computed.leucocitos, exp.leucocitos),
        ("gcs_pts", computed.gcs_pts, exp.gcs_pts),
        ("aps_total", computed.aps_total, exp.aps_total),
        ("edad_pts", computed.edad_pts, exp.edad_pts),
        ("cronicas_pts", computed.cronicas_pts, exp.cronicas_pts),
        ("total", computed.total, exp.total),
    ];
    pairs
        .into_iter()
        .filter(|(_, c, e)| c != e)
        .map(|(name, c, e)| format!("{name}: calculado={c}, esperado={e}"))
        .collect()
}
