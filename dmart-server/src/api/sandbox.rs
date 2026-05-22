use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use dmart_shared::models::*;
use crate::db::Database;
use anyhow::Error;

type ApiResult<T> = Result<Json<ApiResponse<T>>, (StatusCode, String)>;

fn err_to_str(e: Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

pub async fn generate_patients(
    State(db): State<Database>,
    Json(req): Json<SandboxGenerateRequest>,
) -> ApiResult<String> {
    let count = req.cantidad_pacientes.min(100).max(1);
    let mut created = 0u32;

    for _ in 0..count {
        let patient = generate_synthetic_patient();
        if let Ok(p) = crate::db::create_patient(&db, patient).await {
            let pid = p.patient_id;
            for _ in 0..req.mediciones_por_paciente.min(20) {
                let (apache, gcs) = generate_synthetic_measurement();
                let m = Measurement::new(&pid, apache, gcs);
                let _ = crate::db::create_measurement(&db, m).await;
            }
            created += 1;
        }
    }

    let msg = format!("Generados {} pacientes sintéticos con mediciones", created);
    tracing::info!("🧪 {}", msg);
    Ok(Json(ApiResponse::ok(msg)))
}

pub async fn clear_sandbox(
    State(db): State<Database>,
) -> ApiResult<String> {
    let pacientes = crate::db::list_patients(&db).await.map_err(err_to_str)?;
    let mut deleted = 0u32;
    for p in &pacientes {
        if let Err(e) = crate::db::delete_patient(&db, &p.patient_id).await {
            tracing::warn!("Error deleting synthetic patient {}: {}", p.patient_id, e);
        } else {
            deleted += 1;
        }
    }
    let msg = format!("Eliminados {} pacientes de sandbox", deleted);
    tracing::info!("🧹 {}", msg);
    Ok(Json(ApiResponse::ok(msg)))
}

fn generate_synthetic_patient() -> Patient {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let nombres = ["Carlos", "Maria", "Jose", "Ana", "Luis", "Carmen", "Pedro", "Rosa", "Juan", "Marta"];
    let apellidos = ["Garcia", "Rodriguez", "Martinez", "Lopez", "Gonzalez", "Perez", "Sanchez", "Diaz", "Torres", "Ramirez"];

    let idx_nom = rng.gen_range(0..nombres.len());
    let idx_ape = rng.gen_range(0..apellidos.len());
    let sexo = if rng.gen_bool(0.5) { Sexo::Masculino } else { Sexo::Femenino };
    let edad = rng.gen_range(18..=90);

    let mut p = Patient::new();
    p.nombre = nombres[idx_nom].to_string();
    p.apellido = apellidos[idx_ape].to_string();
    p.sexo = sexo;
    p.cedula = format!("V-{}", rng.gen_range(1000000..=30000000));
    let year: i32 = 2026i32 - edad as i32;
    p.edad = edad;
    p.fecha_nacimiento = format!("{:04}-{:02}-{:02}", year, rng.gen_range(1..=12), rng.gen_range(1..=28));
    p.pais = "Venezuela".to_string();
    p
}

fn generate_synthetic_measurement() -> (ApacheIIData, GcsData) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let apache = ApacheIIData {
        temperatura: (36.0 + rng.gen::<f32>() * 4.0 * 10.0).round() / 10.0,
        presion_arterial_media: rng.gen_range(50..=150) as f32,
        presion_sistolica: rng.gen_range(80..=200) as f32,
        frecuencia_cardiaca: rng.gen_range(40..=180) as f32,
        frecuencia_respiratoria: rng.gen_range(8..=40) as f32,
        fio2: (0.21 + rng.gen::<f32>() * 0.79 * 10.0).round() / 10.0,
        pao2: Some(rng.gen_range(50..=200) as f32),
        spo2: rng.gen_range(80..=100) as f32,
        ph_arterial: (7.0 + rng.gen::<f32>() * 0.6 * 10.0).round() / 10.0,
        sodio_serico: rng.gen_range(120..=160) as f32,
        potasio_serico: (2.0 + rng.gen::<f32>() * 5.0 * 10.0).round() / 10.0,
        creatinina: (0.3 + rng.gen::<f32>() * 8.0 * 10.0).round() / 10.0,
        falla_renal_aguda: rng.gen_bool(0.1),
        bilirrubina: (0.2 + rng.gen::<f32>() * 15.0 * 10.0).round() / 10.0,
        hematocrito: rng.gen_range(20..=55) as f32,
        leucocitos: (1.0 + rng.gen::<f32>() * 40.0 * 10.0).round() / 10.0,
        plaquetas: rng.gen_range(20..=500) as f32,
        gcs_ojos: rng.gen_range(1..=4),
        gcs_verbal: rng.gen_range(1..=5),
        gcs_motor: rng.gen_range(1..=6),
        gcs_total: 0,
        edad: rng.gen_range(18..=90),
        ..ApacheIIData::default()
    };

    let gcs = GcsData {
        apertura_ocular: apache.gcs_ojos,
        respuesta_verbal: apache.gcs_verbal,
        respuesta_motora: apache.gcs_motor,
    };

    (apache, gcs)
}
