use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use dmart_shared::models::*;
use crate::db::Database;
use crate::db as db_ops;

// GET /api/patients/:id/export/csv
pub async fn export_csv(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
) -> impl IntoResponse {
    let patient = match db_ops::get_patient(&db, &patient_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return error_response("Paciente no encontrado"),
        Err(e)    => return error_response(&e.to_string()),
    };

    let measurements = match db_ops::get_measurements_for_patient(&db, &patient_id).await {
        Ok(m)  => m,
        Err(e) => return error_response(&e.to_string()),
    };

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(vec![]);

    // Header row
    wtr.write_record(&[
        "Fecha/Hora", "Apache II Score", "GCS Total",
        "Severidad", "Mortalidad Estimada (%)",
        "Temperatura (°C)", "PAM (mmHg)", "FC (lpm)", "FR (rpm)",
        "FiO2", "pH Arterial", "Na (mEq/L)", "K (mEq/L)",
        "Creatinina (mg/dL)", "Hematocrito (%)", "Leucocitos (x10³)",
        "GCS Ocular", "GCS Verbal", "GCS Motor",
        "Notas",
    ]).ok();

    for m in &measurements {
        wtr.write_record(&[
            &m.timestamp,
            &m.apache_score.to_string(),
            &m.gcs_score.to_string(),
            m.severity.label(),
            &format!("{:.1}", m.mortality_risk),
            &format!("{:.1}", m.apache_data.temperatura),
            &format!("{:.1}", m.apache_data.presion_arterial_media),
            &format!("{:.1}", m.apache_data.frecuencia_cardiaca),
            &format!("{:.1}", m.apache_data.frecuencia_respiratoria),
            &format!("{:.2}", m.apache_data.fio2),
            &format!("{:.2}", m.apache_data.ph_arterial),
            &format!("{:.1}", m.apache_data.sodio_serico),
            &format!("{:.1}", m.apache_data.potasio_serico),
            &format!("{:.2}", m.apache_data.creatinina),
            &format!("{:.1}", m.apache_data.hematocrito),
            &format!("{:.1}", m.apache_data.leucocitos),
            &m.gcs_data.apertura_ocular.to_string(),
            &m.gcs_data.respuesta_verbal.to_string(),
            &m.gcs_data.respuesta_motora.to_string(),
            &m.notas,
        ]).ok();
    }

    let data = wtr.into_inner().unwrap_or_default();
    let filename = format!("UCI_{}_{}.csv",
        patient.apellido.replace(' ', "_"),
        patient.cedula
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from(data))
        .unwrap_or_else(|_| error_response("Error building response"))
}

// GET /api/patients/:id/export/pdf
pub async fn export_pdf(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
) -> impl IntoResponse {
    let patient = match db_ops::get_patient(&db, &patient_id).await {
        Ok(Some(p)) => p,
        Ok(None)    => return error_response("Paciente no encontrado"),
        Err(e)      => return error_response(&e.to_string()),
    };

    let measurements = match db_ops::get_measurements_for_patient(&db, &patient_id).await {
        Ok(m)  => m,
        Err(e) => return error_response(&e.to_string()),
    };

    match generate_pdf(&patient, &measurements) {
        Ok(bytes) => {
            let filename = format!("UCI_{}_{}.pdf",
                patient.apellido.replace(' ', "_"),
                patient.cedula
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/pdf")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(axum::body::Body::from(bytes))
                .unwrap_or_else(|_| error_response("Error building response"))
        }
        Err(e) => error_response(&e.to_string()),
    }
}

fn generate_pdf(patient: &Patient, measurements: &[Measurement]) -> anyhow::Result<Vec<u8>> {
    use printpdf::*;

    let (doc, page1, layer1) = PdfDocument::new(
        &format!("UCI — {} {}", patient.nombre, patient.apellido),
        Mm(210.0), Mm(297.0), "Datos del Paciente",
    );

    let page_ref = doc.get_page(page1);
    let layer = page_ref.get_layer(layer1);
    let font = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
    let font_reg = doc.add_builtin_font(BuiltinFont::Helvetica)?;

    let mut y = 270.0_f32;
    let left = 15.0_f32;

    let write_line = |text: &str, bold: bool, size: f32, y_pos: f32| {
        let f = if bold { &font } else { &font_reg };
        layer.use_text(text, size, Mm(left), Mm(y_pos), f);
    };

    // Header
    write_line("SISTEMA UCI — REGISTRO DE PACIENTE", true, 14.0, y);
    y -= 8.0;
    write_line(&format!("Historia Clínica: {}   Cédula: {}", patient.historia_clinica, patient.cedula), false, 10.0, y);
    y -= 6.0;
    write_line(&format!("Paciente: {} {}", patient.nombre, patient.apellido), true, 12.0, y);
    y -= 6.0;
    write_line(&format!("Sexo: {:?}   Color de Piel: {}", patient.sexo, patient.color_piel.label()), false, 9.0, y);
    y -= 5.0;
    write_line(&format!("Fecha Nacimiento: {}   Ingreso UCI: {}", patient.fecha_nacimiento, patient.fecha_ingreso_uci), false, 9.0, y);
    y -= 5.0;
    write_line(&format!("Diagnóstico UCI: {}", patient.diagnostico_uci), false, 9.0, y);
    y -= 5.0;
    write_line(&format!("Tipo Admisión: {:?}   VM: {}   Procesos Invasivos: {}", 
        patient.tipo_admision, 
        patient.ventilacion_mecanica,
        patient.procesos_invasivos.join(", ")
    ), false, 9.0, y);

    y -= 8.0;
    write_line("─── EVOLUCIÓN APACHE II ───", true, 11.0, y);
    y -= 6.0;
    write_line("  Fecha/Hora              Apache II   GCS   Severidad       Mortalidad", true, 9.0, y);
    y -= 5.0;

    for m in measurements {
        if y < 20.0 {
            let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "Continuación");
            let _pl = doc.get_page(new_page);
            let _ll = _pl.get_layer(new_layer);
            y = 270.0;
        }
        let line = format!("  {:<25} {:>8}    {:>3}   {:<12}    {:>6.1}%",
            &m.timestamp[..19],
            m.apache_score,
            m.gcs_score,
            m.severity.label(),
            m.mortality_risk,
        );
        write_line(&line, false, 8.0, y);
        y -= 4.5;
    }

    y -= 5.0;
    write_line(&format!("Estado actual de gravedad: {}", patient.estado_gravedad.label()), true, 10.0, y);
    y -= 4.0;
    write_line(&format!("Mortalidad estimada: {}", patient.estado_gravedad.mortality_estimate()), false, 9.0, y);

    let bytes = doc.save_to_bytes()?;
    Ok(bytes)
}

fn error_response(msg: &str) -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            format!("{{\"success\":false,\"error\":\"{}\"}}", msg)
        ))
        .unwrap()
}
