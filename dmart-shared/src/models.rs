use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Enums
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Sexo {
    #[default]
    Masculino,
    Femenino,
}

/// Escala Fitzpatrick de color de piel (con valores hex para UI)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ColorPiel {
    #[default]
    Tipo1, // Muy clara / pálida
    Tipo2, // Clara / blanca
    Tipo3, // Intermedia / beige
    Tipo4, // Oliva / marrón claro
    Tipo5, // Marrón oscuro
    Tipo6, // Muy oscura / negra
}

impl ColorPiel {
    pub fn label(&self) -> &'static str {
        match self {
            ColorPiel::Tipo1 => "Tipo I — Muy Clara",
            ColorPiel::Tipo2 => "Tipo II — Clara",
            ColorPiel::Tipo3 => "Tipo III — Intermedia",
            ColorPiel::Tipo4 => "Tipo IV — Oliva",
            ColorPiel::Tipo5 => "Tipo V — Marrón Oscuro",
            ColorPiel::Tipo6 => "Tipo VI — Muy Oscura",
        }
    }

    pub fn hex_color(&self) -> &'static str {
        match self {
            ColorPiel::Tipo1 => "#FDDBB4",
            ColorPiel::Tipo2 => "#F5C99E",
            ColorPiel::Tipo3 => "#E8B88A",
            ColorPiel::Tipo4 => "#C68642",
            ColorPiel::Tipo5 => "#8D5524",
            ColorPiel::Tipo6 => "#4A2912",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Nacionalidad {
    #[default]
    Venezolano,
    Extranjero,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum TipoAdmision {
    #[default]
    Urgente,
    Electiva,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SeverityLevel {
    #[default]
    Bajo, // 0-9
    Moderado, // 10-19
    Severo,   // 20-29
    Critico,  // ≥30
}

impl SeverityLevel {
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=9 => SeverityLevel::Bajo,
            10..=19 => SeverityLevel::Moderado,
            20..=29 => SeverityLevel::Severo,
            _ => SeverityLevel::Critico,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SeverityLevel::Bajo => "Bajo",
            SeverityLevel::Moderado => "Moderado",
            SeverityLevel::Severo => "Severo",
            SeverityLevel::Critico => "Crítico",
        }
    }

    pub fn color_class(&self) -> &'static str {
        match self {
            SeverityLevel::Bajo => "severity-low",
            SeverityLevel::Moderado => "severity-moderate",
            SeverityLevel::Severo => "severity-severe",
            SeverityLevel::Critico => "severity-critical",
        }
    }

    pub fn mortality_estimate(&self) -> &'static str {
        match self {
            SeverityLevel::Bajo => "< 10%",
            SeverityLevel::Moderado => "10–25%",
            SeverityLevel::Severo => "25–50%",
            SeverityLevel::Critico => "> 50%",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Patient
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    #[serde(skip_serializing, skip_deserializing, default)]
    pub id: Option<String>,

    // ID público del paciente
    #[serde(default)]
    pub patient_id: String,

    // Identificación
    #[serde(default)]
    pub nombre: String,
    #[serde(default)]
    pub apellido: String,
    #[serde(default)]
    pub sexo: Sexo,
    #[serde(default)]
    pub cedula: String,
    #[serde(default)]
    pub color_piel: ColorPiel,
    #[serde(default)]
    pub historia_clinica: String,

    // Nacionalidad y origen
    #[serde(default)]
    pub nacionalidad: Nacionalidad,
    #[serde(default)]
    pub pais: String,
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub ciudad: String,
    #[serde(default)]
    pub lugar_nacimiento: String,
    #[serde(default)]
    pub direccion: String,

    // Datos personales
    #[serde(default)]
    pub fecha_nacimiento: String, // ISO date string YYYY-MM-DD
    #[serde(default)]
    pub familiar_encargado: String,

    // Ingreso hospitalario
    #[serde(default)]
    pub fecha_ingreso_hospital: String, // ISO datetime
    #[serde(default)]
    pub fecha_ingreso_uci: String, // ISO datetime
    #[serde(default)]
    pub descripcion_ingreso: String,
    #[serde(default)]
    pub antecedentes: String,
    #[serde(default)]
    pub resumen_ingreso: String,
    #[serde(default)]
    pub diagnostico_hospital: String,
    #[serde(default)]
    pub diagnostico_uci: String,
    #[serde(default)]
    pub examen_fisico_hospital: String,
    #[serde(default)]
    pub examen_fisico_uci: String,
    #[serde(default)]
    pub tipo_admision: TipoAdmision,
    #[serde(default)]
    pub migracion_otro_centro: bool,
    #[serde(default)]
    pub centro_origen: Option<String>,

    // Soporte vital y procesos
    #[serde(default)]
    pub ventilacion_mecanica: bool,
    #[serde(default)]
    pub procesos_invasivos: Vec<String>,

    // Estado calculado
    #[serde(default)]
    pub estado_gravedad: SeverityLevel,
    #[serde(default)]
    pub ultimo_apache_score: Option<u32>,
    #[serde(default)]
    pub ultimo_gcs_score: Option<u8>,

    #[serde(default)]
    pub created_at: String, // ISO datetime
    #[serde(default)]
    pub updated_at: String,
}

impl Patient {
    pub fn new() -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: None,
            patient_id: Uuid::new_v4().to_string(),
            nombre: String::new(),
            apellido: String::new(),
            sexo: Sexo::Masculino,
            cedula: String::new(),
            color_piel: ColorPiel::Tipo3,
            historia_clinica: String::new(),
            nacionalidad: Nacionalidad::Venezolano,
            pais: "Venezuela".to_string(),
            estado: String::new(),
            ciudad: String::new(),
            lugar_nacimiento: String::new(),
            direccion: String::new(),
            fecha_nacimiento: String::new(),
            familiar_encargado: String::new(),
            fecha_ingreso_hospital: now.clone(),
            fecha_ingreso_uci: now.clone(),
            descripcion_ingreso: String::new(),
            antecedentes: String::new(),
            resumen_ingreso: String::new(),
            diagnostico_hospital: String::new(),
            diagnostico_uci: String::new(),
            examen_fisico_hospital: String::new(),
            examen_fisico_uci: String::new(),
            tipo_admision: TipoAdmision::Urgente,
            migracion_otro_centro: false,
            centro_origen: None,
            ventilacion_mecanica: false,
            procesos_invasivos: Vec::new(),
            estado_gravedad: SeverityLevel::Bajo,
            ultimo_apache_score: None,
            ultimo_gcs_score: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn nombre_completo(&self) -> String {
        format!("{} {}", self.nombre, self.apellido)
    }
}

impl Default for Patient {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Apache II Measurement
// ─────────────────────────────────────────────────────────────────────────────

/// Variables fisiológicas para el cálculo del Score Apache II
/// Todos los valores son los valores REALES (no puntos); el cálculo de puntos
/// se realiza en scales.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApacheIIData {
    // Signos vitales
    pub temperatura: f32,             // °C rectal/central (30.0-44.0)
    pub presion_arterial_media: f32,  // mmHg (0-200)
    pub frecuencia_cardiaca: f32,     // lpm (0-200)
    pub frecuencia_respiratoria: f32, // rpm (0-60)

    // Oxigenación
    pub fio2: f32,           // fracción inspirada O2 (0.21-1.0)
    pub pao2: Option<f32>,   // mmHg — usado si FiO2 < 0.5
    pub a_ado2: Option<f32>, // mmHg — usado si FiO2 >= 0.5

    // Laboratorios
    pub ph_arterial: f32,        // 7.00-7.70
    pub sodio_serico: f32,       // mEq/L (100-200)
    pub potasio_serico: f32,     // mEq/L (1.0-8.0)
    pub creatinina: f32,         // mg/dL (0.1-10.0)
    pub falla_renal_aguda: bool, // multiplica creatinina x2 si true

    pub hematocrito: f32, // % (10-70)
    pub leucocitos: f32,  // x10^3/mm³ (0.5-60.0)

    // Glasgow Coma Scale (para el APS)
    pub gcs_total: u8, // 3-15

    // Parámetros de edad y enfermedades crónicas
    pub edad: u8,

    // Enfermedades crónicas severas (para puntos adicionales Apache II)
    pub insuficiencia_hepatica: bool,
    pub cardiovascular_severa: bool,
    pub insuficiencia_respiratoria: bool,
    pub insuficiencia_renal: bool,
    pub inmunocomprometido: bool,

    // Tipo de hospitalización (afecta puntos por enfermedades crónicas)
    pub cirugia_no_operado: bool, // no quirúrgico o cirugía de emergencia
}

impl Default for ApacheIIData {
    fn default() -> Self {
        Self {
            temperatura: 37.0,
            presion_arterial_media: 93.0,
            frecuencia_cardiaca: 80.0,
            frecuencia_respiratoria: 16.0,
            fio2: 0.21,
            pao2: Some(80.0),
            a_ado2: None,
            ph_arterial: 7.40,
            sodio_serico: 140.0,
            potasio_serico: 4.0,
            creatinina: 1.0,
            falla_renal_aguda: false,
            hematocrito: 42.0,
            leucocitos: 8.0,
            gcs_total: 15,
            edad: 50,
            insuficiencia_hepatica: false,
            cardiovascular_severa: false,
            insuficiencia_respiratoria: false,
            insuficiencia_renal: false,
            inmunocomprometido: false,
            cirugia_no_operado: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Glasgow Coma Scale
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GcsData {
    pub apertura_ocular: u8,  // 1=Ninguna, 2=Al dolor, 3=A la voz, 4=Espontánea
    pub respuesta_verbal: u8, // 1=Ninguna, 2=Sonidos, 3=Palabras, 4=Confuso, 5=Orientado
    pub respuesta_motora: u8, // 1=Ninguna, 2=Extensión, 3=Flexión anormal, 4=Retirada, 5=Localiza, 6=Obedece
}

impl GcsData {
    pub fn total(&self) -> u8 {
        self.apertura_ocular + self.respuesta_verbal + self.respuesta_motora
    }

    pub fn apertura_label(v: u8) -> &'static str {
        match v {
            1 => "Ninguna",
            2 => "Al dolor",
            3 => "A la voz",
            4 => "Espontánea",
            _ => "—",
        }
    }

    pub fn verbal_label(v: u8) -> &'static str {
        match v {
            1 => "Sin respuesta",
            2 => "Sonidos incomprensibles",
            3 => "Palabras inapropiadas",
            4 => "Confuso",
            5 => "Orientado",
            _ => "—",
        }
    }

    pub fn motora_label(v: u8) -> &'static str {
        match v {
            1 => "Sin respuesta",
            2 => "Extensión anormal (descerebración)",
            3 => "Flexión anormal (decorticación)",
            4 => "Retirada al dolor",
            5 => "Localiza el dolor",
            6 => "Obedece órdenes",
            _ => "—",
        }
    }

    pub fn interpret(&self) -> &'static str {
        match self.total() {
            15 => "Consciente / Normal",
            13..=14 => "Lesión leve",
            9..=12 => "Lesión moderada",
            3..=8 => "Lesión grave / Coma",
            _ => "—",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Measurement (medición diaria)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    #[serde(skip_serializing, skip_deserializing, default)]
    pub id: Option<String>,

    // ID público de la medición
    #[serde(default)]
    pub measurement_id: String,
    #[serde(default)]
    pub patient_id: String,
    #[serde(default)]
    pub timestamp: String, // ISO datetime UTC
    #[serde(default)]
    pub apache_data: ApacheIIData,
    #[serde(default)]
    pub gcs_data: GcsData,
    #[serde(default)]
    pub apache_score: u32, // calculado
    #[serde(default)]
    pub gcs_score: u8, // calculado
    #[serde(default)]
    pub severity: SeverityLevel, // calculado
    #[serde(default)]
    pub mortality_risk: f32, // % estimado
    #[serde(default)]
    pub notas: String,
}

impl Measurement {
    pub fn new(patient_id: &str, apache: ApacheIIData, gcs: GcsData) -> Self {
        use crate::scales::{calculate_apache_ii_score, mortality_risk};
        let apache_score = calculate_apache_ii_score(&apache);
        let gcs_score = gcs.total();
        let severity = SeverityLevel::from_score(apache_score);
        let mort = mortality_risk(apache_score);

        Self {
            id: None,
            measurement_id: Uuid::new_v4().to_string(),
            patient_id: patient_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            apache_data: apache,
            gcs_data: gcs,
            apache_score,
            gcs_score,
            severity,
            mortality_risk: mort,
            notas: String::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// API DTOs (shared between frontend fetches and backend handlers)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientListItem {
    pub id: String,
    pub nombre_completo: String,
    pub cedula: String,
    pub historia_clinica: String,
    pub edad: u8,
    pub sexo: Sexo,
    pub fecha_ingreso_uci: String,
    pub estado_gravedad: SeverityLevel,
    pub ultimo_apache_score: Option<u32>,
    pub ultimo_gcs_score: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}
