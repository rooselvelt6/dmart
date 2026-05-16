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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum News2Level {
    #[default]
    Bajo,
    Medio,
    Alto,
    Emergent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SofaLevel {
    #[default]
    Normal,     // 0-1
    Disfuncion, // 2-6
    Falla,      // 7-9
    FallaMultiorganica, // >=10
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Saps3Level {
    #[default]
    Estable,
    RiesgoModerado,
    RiesgoAlto,
    RiesgoCritico,
}

impl News2Level {
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=4 => News2Level::Bajo,
            5..=6 => News2Level::Medio,
            7..=19 => News2Level::Alto,
            _ => News2Level::Emergent,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            News2Level::Bajo => "Bajo",
            News2Level::Medio => "Medio",
            News2Level::Alto => "Alto",
            News2Level::Emergent => "Emergencia",
        }
    }

    pub fn color_class(&self) -> &'static str {
        match self {
            News2Level::Bajo => "text-emerald-500",
            News2Level::Medio => "text-amber-500",
            News2Level::Alto => "text-orange-500",
            News2Level::Emergent => "text-rose-600",
        }
    }

    pub fn response(&self) -> &'static str {
        match self {
            News2Level::Bajo => "Monitoreo habitual",
            News2Level::Medio => "Revisión clínica en 1 hora",
            News2Level::Alto => "Revisión clínica inmediata",
            News2Level::Emergent => "Activación de código emergencia",
        }
    }
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
            SeverityLevel::Bajo => "text-emerald-500",
            SeverityLevel::Moderado => "text-blue-500",
            SeverityLevel::Severo => "text-amber-500",
            SeverityLevel::Critico => "text-rose-600",
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

impl SofaLevel {
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=1 => SofaLevel::Normal,
            2..=6 => SofaLevel::Disfuncion,
            7..=9 => SofaLevel::Falla,
            _ => SofaLevel::FallaMultiorganica,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SofaLevel::Normal => "Normal",
            SofaLevel::Disfuncion => "Disfunción Leve/Mod",
            SofaLevel::Falla => "Falla Orgánica",
            SofaLevel::FallaMultiorganica => "Falla Multiorgánica",
        }
    }

    pub fn color_class(&self) -> &'static str {
        match self {
            SofaLevel::Normal => "text-emerald-500",
            SofaLevel::Disfuncion => "text-amber-500",
            SofaLevel::Falla => "text-orange-500",
            SofaLevel::FallaMultiorganica => "text-rose-600",
        }
    }
}

impl Saps3Level {
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=30 => Saps3Level::Estable,
            31..=50 => Saps3Level::RiesgoModerado,
            51..=70 => Saps3Level::RiesgoAlto,
            _ => Saps3Level::RiesgoCritico,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Saps3Level::Estable => "Estable",
            Saps3Level::RiesgoModerado => "Riesgo Moderado",
            Saps3Level::RiesgoAlto => "Riesgo Alto",
            Saps3Level::RiesgoCritico => "Crítico / Muy Alto",
        }
    }

    pub fn color_class(&self) -> &'static str {
        match self {
            Saps3Level::Estable => "text-emerald-500",
            Saps3Level::RiesgoModerado => "text-amber-500",
            Saps3Level::RiesgoAlto => "text-orange-500",
            Saps3Level::RiesgoCritico => "text-rose-600",
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

    // Cama asignada
    #[serde(default)]
    pub cama_id: Option<String>,
    #[serde(default)]
    pub cama_numero: Option<u8>,

    // Estado calculado
    #[serde(default)]
    pub estado_gravedad: SeverityLevel,
    #[serde(default)]
    pub ultimo_apache_score: Option<u32>,
    #[serde(default)]
    pub ultimo_gcs_score: Option<u8>,
    #[serde(default)]
    pub ultimo_sofa_score: Option<u32>,
    #[serde(default)]
    pub ultimo_saps3_score: Option<u32>,
    #[serde(default)]
    pub ultimo_news2_score: Option<u32>,
    #[serde(default)]
    pub mortality_risk: Option<f32>,

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
            cama_id: None,
            cama_numero: None,
            estado_gravedad: SeverityLevel::Bajo,
            ultimo_apache_score: None,
            ultimo_gcs_score: None,
            ultimo_sofa_score: None,
            ultimo_saps3_score: None,
            ultimo_news2_score: None,
            mortality_risk: None,
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
    pub presion_sistolica: f32,       // mmHg (0-250) - para SAPS III y NEWS2
    pub frecuencia_cardiaca: f32,     // lpm (0-200)
    pub frecuencia_respiratoria: f32, // rpm (0-60)

    // Oxigenación
    pub fio2: f32,           // fracción inspirada O2 (0.21-1.0)
    pub pao2: Option<f32>,   // mmHg — usado si FiO2 < 0.5
    pub a_ado2: Option<f32>, // mmHg — usado si FiO2 >= 0.5
    pub spo2: f32,           // % saturación O2 (0-100) - para NEWS2

    // Laboratorios
    pub ph_arterial: f32,        // 7.00-7.70
    pub sodio_serico: f32,       // mEq/L (100-200)
    pub potasio_serico: f32,     // mEq/L (1.0-8.0)
    pub creatinina: f32,         // mg/dL (0.1-10.0)
    pub falla_renal_aguda: bool, // multiplica creatinina x2 si true
    pub bilirrubina: f32,        // mg/dL (0.1-30.0) - para SAPS III y SOFA

    pub hematocrito: f32, // % (10-70)
    pub leucocitos: f32,  // x10^3/mm³ (0.5-60.0)
    pub plaquetas: f32,   // x10^3/mm³ - para SAPS III y SOFA

    // Glasgow Coma Scale (para el APS)
    pub gcs_ojos: u8,
    pub gcs_verbal: u8,
    pub gcs_motor: u8,
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

    // Soporte vital
    pub ventilacion_mecanica: bool, // para SAPS III
    pub vasopresores: bool,         // para SAPS III y SOFA
    pub dosis_vasopresor: f32,      // µg/kg/min (dopamina o equivalente)
    pub diuresis_diaria: u32,       // mL/día para SOFA renal

    // NEWS2 específicos
    pub alerta: bool,             // estado de alerta del paciente
    pub o2_suplementario: bool,   // está recibiendo O2 suplementario
    pub nivel_conciencia: String, // para GCS/NEWS2

    // SAPS III específicos
    pub bicarbonate: f32, // mEq/L

    // Admisión (para SAPS III)
    pub tipo_admision: Option<String>, // "medical", "scheduled_surgical", "unscheduled_surgical"
    pub fuente_admision: Option<String>, // "emergency_room", "ward", "other_icu"
    pub dias_pre_uci: u8,              // días en hospital antes de UCI
    pub infeccion_admision: Option<String>, // "none", "respiratory", "nosocomial"
    pub sistema_anatomico: Option<String>, // razón de ingreso
}

impl Default for ApacheIIData {
    fn default() -> Self {
        Self {
            temperatura: 37.0,
            presion_arterial_media: 93.0,
            presion_sistolica: 120.0,
            frecuencia_cardiaca: 80.0,
            frecuencia_respiratoria: 16.0,
            fio2: 0.21,
            pao2: Some(80.0),
            a_ado2: None,
            spo2: 98.0,
            ph_arterial: 7.40,
            sodio_serico: 140.0,
            potasio_serico: 4.0,
            creatinina: 1.0,
            falla_renal_aguda: false,
            bilirrubina: 0.8,
            hematocrito: 42.0,
            leucocitos: 8.0,
            plaquetas: 250.0,
            gcs_ojos: 4,
            gcs_verbal: 5,
            gcs_motor: 6,
            gcs_total: 15,
            edad: 50,
            insuficiencia_hepatica: false,
            cardiovascular_severa: false,
            insuficiencia_respiratoria: false,
            insuficiencia_renal: false,
            inmunocomprometido: false,
            cirugia_no_operado: false,
            ventilacion_mecanica: false,
            vasopresores: false,
            dosis_vasopresor: 0.0,
            diuresis_diaria: 1500,
            alerta: true,
            o2_suplementario: false,
            nivel_conciencia: String::new(),
            bicarbonate: 24.0,
            tipo_admision: None,
            fuente_admision: None,
            dias_pre_uci: 0,
            infeccion_admision: None,
            sistema_anatomico: None,
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

    // Nuevos scores
    #[serde(default)]
    pub saps3_score: Option<u32>,
    #[serde(default)]
    pub saps3_mortality: Option<f32>,
    #[serde(default)]
    pub news2_score: Option<u32>,
    #[serde(default)]
    pub news2_level: News2Level,
    #[serde(default)]
    pub sofa_score: Option<u32>,
    #[serde(default)]
    pub sofa_mortality: Option<f32>,

    #[serde(default)]
    pub notas: String,
}

impl Measurement {
    pub fn new(patient_id: &str, apache: ApacheIIData, gcs: GcsData) -> Self {
        use crate::scales::{
            calculate_apache_ii_score, calculate_news2_score, calculate_saps_iii_score,
            calculate_sofa_score, mortality_risk, saps_iii_mortality_prediction,
            sofa_mortality_estimate,
        };
        let apache_score = calculate_apache_ii_score(&apache);
        let gcs_score = gcs.total();
        let severity = SeverityLevel::from_score(apache_score);
        let mort = mortality_risk(apache_score);

        let saps3_score = calculate_saps_iii_score(&apache);
        let saps3_mortality = saps_iii_mortality_prediction(saps3_score);

        let news2_score = calculate_news2_score(&apache);
        let news2_level = News2Level::from_score(news2_score);

        let sofa_score = calculate_sofa_score(&apache);
        let sofa_mortality = sofa_mortality_estimate(sofa_score);

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
            saps3_score: Some(saps3_score),
            saps3_mortality: Some(saps3_mortality),
            news2_score: Some(news2_score),
            news2_level,
            sofa_score: Some(sofa_score),
            sofa_mortality: Some(sofa_mortality),
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
    
    // Todas las escalas clínicas
    pub ultimo_apache_score: Option<u32>,
    pub ultimo_gcs_score: Option<u8>,
    pub ultimo_sofa_score: Option<u32>,
    pub ultimo_saps3_score: Option<u32>,
    pub ultimo_news2_score: Option<u32>,
    pub mortality_risk: Option<f32>,
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

// ─────────────────────────────────────────────────────────────────────────────
// User / Authentication
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum UserRole {
    #[default]
    Admin,
    Medico,
    Enfermero,
    Viewer,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "Admin"),
            UserRole::Medico => write!(f, "Medico"),
            UserRole::Enfermero => write!(f, "Enfermero"),
            UserRole::Viewer => write!(f, "Viewer"),
        }
    }
}

impl UserRole {
    pub fn label(&self) -> &'static str {
        match self {
            UserRole::Admin => "Admin",
            UserRole::Medico => "Medico",
            UserRole::Enfermero => "Enfermero",
            UserRole::Viewer => "Viewer",
        }
    }

    pub fn can_edit(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Medico)
    }
    pub fn can_measure(&self) -> bool {
        matches!(
            self,
            UserRole::Admin | UserRole::Medico | UserRole::Enfermero
        )
    }
    pub fn can_view(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub password_hash: String,  // Needed for DB storage
    pub rol: UserRole,
    pub nombre: String,
    pub activo: bool,
    pub created_at: String,
}

impl Default for User {
    fn default() -> Self {
        Self {
            user_id: Uuid::new_v4().to_string(),
            username: String::new(),
            password_hash: String::new(),
            rol: UserRole::Admin,
            nombre: String::new(),
            activo: true,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: String,
    pub username: String,
    pub rol: UserRole,
    pub nombre: String,
}

impl From<&User> for UserInfo {
    fn from(u: &User) -> Self {
        Self {
            user_id: u.user_id.clone(),
            username: u.username.clone(),
            rol: u.rol.clone(),
            nombre: u.nombre.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TipoCama - Tipos de Camas UCI
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum TipoCama {
    #[default]
    General,
    Aislamiento,
    Pediatrica,
    Coronaria,
    Quemados,
    Otro,
}

impl TipoCama {
    pub fn label(&self) -> &'static str {
        match self {
            TipoCama::General => "General",
            TipoCama::Aislamiento => "Aislamiento",
            TipoCama::Pediatrica => "Pediátrica",
            TipoCama::Coronaria => "Coronaria",
            TipoCama::Quemados => "Quemados",
            TipoCama::Otro => "Otro",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TipoCama::General => "fa-bed",
            TipoCama::Aislamiento => "fa-shield-virus",
            TipoCama::Pediatrica => "fa-child",
            TipoCama::Coronaria => "fa-heart",
            TipoCama::Quemados => "fa-fire",
            TipoCama::Otro => "fa-question",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cama - Gestión de Camas UCI
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum EstadoCama {
    #[default]
    Libre,
    Ocupada,
    Mantenimiento,
    Limpieza,
}

impl EstadoCama {
    pub fn label(&self) -> &'static str {
        match self {
            EstadoCama::Libre => "Libre",
            EstadoCama::Ocupada => "Ocupada",
            EstadoCama::Mantenimiento => "Mantenimiento",
            EstadoCama::Limpieza => "Limpieza",
        }
    }

    pub fn puede_asignar(&self) -> bool {
        matches!(self, EstadoCama::Libre)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cama {
    #[serde(skip_serializing, skip_deserializing, default)]
    pub id: Option<String>,

    #[serde(default)]
    pub cama_id: String,

    #[serde(default)]
    pub numero: u8,

    #[serde(default)]
    pub tipo: TipoCama,

    #[serde(default)]
    pub estado: EstadoCama,

    #[serde(default)]
    pub paciente_id: Option<String>,

    #[serde(default)]
    pub paciente_nombre: Option<String>,

    #[serde(default)]
    pub created_at: String,
}

impl Cama {
    pub fn new(numero: u8, tipo: TipoCama) -> Self {
        Self {
            id: None,
            cama_id: Uuid::new_v4().to_string(),
            numero,
            tipo,
            estado: EstadoCama::Libre,
            paciente_id: None,
            paciente_nombre: None,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

impl Default for Cama {
    fn default() -> Self {
        Self::new(1, TipoCama::General)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Equipo - Gestión de Equipos Clínicos
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum TipoEquipo {
    #[default]
    VentiladorMecanico,
    Monitor,
    Computador,
    BombaInfusion,
   Otro,
}

impl TipoEquipo {
    pub fn label(&self) -> &'static str {
        match self {
            TipoEquipo::VentiladorMecanico => "Ventilador Mecánico",
            TipoEquipo::Monitor => "Monitor",
            TipoEquipo::Computador => "Computador",
            TipoEquipo::BombaInfusion => "Bomba de Infusión",
            TipoEquipo::Otro => "Otro",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TipoEquipo::VentiladorMecanico => "fa-wind",
            TipoEquipo::Monitor => "fa-desktop",
            TipoEquipo::Computador => "fa-laptop",
            TipoEquipo::BombaInfusion => "fa-syringe",
            TipoEquipo::Otro => "fa-kit-medical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum EstadoEquipo {
    #[default]
    Activo,
    Mantenimiento,
    Inactivo,
    Reparacion,
}

impl EstadoEquipo {
    pub fn label(&self) -> &'static str {
        match self {
            EstadoEquipo::Activo => "Activo",
            EstadoEquipo::Mantenimiento => "En Mantenimiento",
            EstadoEquipo::Inactivo => "Inactivo",
            EstadoEquipo::Reparacion => "En Reparación",
        }
    }

    pub fn disponible(&self) -> bool {
        matches!(self, EstadoEquipo::Activo)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equipo {
    #[serde(skip_serializing, skip_deserializing, default)]
    pub id: Option<String>,

    #[serde(default)]
    pub equipo_id: String,

    #[serde(default)]
    pub nombre: String,

    #[serde(default)]
    pub tipo: TipoEquipo,

    #[serde(default)]
    pub marca: String,

    #[serde(default)]
    pub modelo: String,

    #[serde(default)]
    pub serial: String,

    #[serde(default)]
    pub estado: EstadoEquipo,

    #[serde(default)]
    pub cama_id: Option<String>,

    #[serde(default)]
    pub proveedor: String,

    #[serde(default)]
    pub fecha_compra: String,

    #[serde(default)]
    pub garantia_hasta: Option<String>,

    #[serde(default)]
    pub notas: String,

    #[serde(default)]
    pub created_at: String,
}

impl Equipo {
    pub fn new(nombre: String, tipo: TipoEquipo) -> Self {
        Self {
            id: None,
            equipo_id: Uuid::new_v4().to_string(),
            nombre,
            tipo,
            marca: String::new(),
            modelo: String::new(),
            serial: String::new(),
            estado: EstadoEquipo::Activo,
            cama_id: None,
            proveedor: String::new(),
            fecha_compra: Utc::now().format("%Y-%m-%d").to_string(),
            garantia_hasta: None,
            notas: String::new(),
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

impl Default for Equipo {
    fn default() -> Self {
        Self::new(String::new(), TipoEquipo::Otro)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin DTOs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdminStats {
    pub total_camas: u8,
    pub camas_libres: u8,
    pub camas_ocupadas: u8,
    pub camas_mantenimiento: u8,
    pub camas_por_tipo: Vec<TipoCamaCount>,
    pub total_equipos: u32,
    pub equipos_activos: u32,
    pub equipos_mantenimiento: u32,
    pub equipos_disponibles: u32,
    pub equipos_por_tipo: Vec<EquipoTipoCount>,
    pub total_staff: u32,
    pub medicos_activos: u32,
    pub enfermeros_activos: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipoCamaCount {
    pub tipo: String,
    pub total: u8,
    pub libres: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipoTipoCount {
    pub tipo: String,
    pub total: u32,
    pub disponibles: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitCamasRequest {
    pub cantidad: u8,
    pub tipo: Option<String>,
}
