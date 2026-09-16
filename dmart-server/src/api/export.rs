use crate::db as db_ops;
use crate::db::Database;
use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use dmart_shared::models::*;
use printpdf::*;

// GET /api/patients/:id/export/csv
pub async fn export_csv(
    State(db): State<Database>,
    Path(patient_id): Path<String>,
) -> impl IntoResponse {
    let patient = match db_ops::get_patient(&db, &patient_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return error_response("Paciente no encontrado"),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            return error_response(&msg);
        }
    };

    let measurements = match db_ops::get_measurements_for_patient(&db, &patient_id).await {
        Ok(m) => m,
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            return error_response(&msg);
        }
    };

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(vec![]);

    // Header row
    wtr.write_record([
        "Fecha/Hora",
        "Apache II Score",
        "GCS Total",
        "Severidad",
        "Mortalidad Estimada (%)",
        "Temperatura (°C)",
        "PAM (mmHg)",
        "FC (lpm)",
        "FR (rpm)",
        "FiO2",
        "pH Arterial",
        "Na (mEq/L)",
        "K (mEq/L)",
        "Creatinina (mg/dL)",
        "Hematocrito (%)",
        "Leucocitos (x10³)",
        "GCS Ocular",
        "GCS Verbal",
        "GCS Motor",
        "Notas",
    ])
    .ok();

    for m in &measurements {
        wtr.write_record([
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
        ])
        .ok();
    }

    let data = wtr.into_inner().unwrap_or_default();
    let (apellido, cedula) = sanitize_filename(&patient);
    let filename = format!("UCI_{}_{}.csv", apellido, cedula);

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
        Ok(None) => return error_response("Paciente no encontrado"),
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            return error_response(&msg);
        }
    };

    let measurements = match db_ops::get_measurements_for_patient(&db, &patient_id).await {
        Ok(m) => m,
        Err(e) => {
            let msg = crate::security::sanitize_internal_error(&e);
            return error_response(&msg);
        }
    };

    match generate_pdf(&patient, &measurements) {
        Ok(bytes) => {
            let (apellido, cedula) = sanitize_filename(&patient);
            let filename = format!("UCI_{}_{}.pdf", apellido, cedula);
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

/// Emite una línea de texto en el PDF (coordenadas `y_top` estilo "desde arriba",
/// convertidas a las coordenadas desde abajo-izquierda de printpdf 0.12).
fn push_text(
    ops: &mut Vec<Op>,
    left: Mm,
    page_h: f32,
    y_top: f32,
    text: &str,
    bold: bool,
    size: f32,
) {
    let y_bottom = Mm(page_h - y_top);
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor {
        pos: Point::new(left, y_bottom),
    });
    ops.push(Op::SetFont {
        font: if bold {
            PdfFontHandle::Builtin(BuiltinFont::HelveticaBold)
        } else {
            PdfFontHandle::Builtin(BuiltinFont::Helvetica)
        },
        size: Pt(size),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
}

fn generate_pdf(patient: &Patient, measurements: &[Measurement]) -> anyhow::Result<Vec<u8>> {
    let mut doc = PdfDocument::new(&format!("UCI — {} {}", patient.nombre, patient.apellido));
    let mut pages: Vec<Vec<Op>> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    let left = Mm(15.0);
    let page_h = 297.0_f32;

    let mut y = 270.0_f32;

    // Header
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        "SISTEMA UCI — REGISTRO DE PACIENTE",
        true,
        14.0,
    );
    y -= 8.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!(
            "Historia Clínica: {}   Cédula: {}",
            patient.historia_clinica, patient.cedula
        ),
        false,
        10.0,
    );
    y -= 6.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!("Paciente: {} {}", patient.nombre, patient.apellido),
        true,
        12.0,
    );
    y -= 6.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!(
            "Sexo: {:?}   Color de Piel: {}",
            patient.sexo,
            patient.color_piel.label()
        ),
        false,
        9.0,
    );
    y -= 5.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!(
            "Fecha Nacimiento: {}   Ingreso UCI: {}",
            patient.fecha_nacimiento, patient.fecha_ingreso_uci
        ),
        false,
        9.0,
    );
    y -= 5.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!("Diagnóstico UCI: {}", patient.diagnostico_uci),
        false,
        9.0,
    );
    y -= 5.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!(
            "Tipo Admisión: {:?}   VM: {}   Procesos Invasivos: {}",
            patient.tipo_admision,
            patient.ventilacion_mecanica,
            patient.procesos_invasivos.join(", ")
        ),
        false,
        9.0,
    );

    y -= 8.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        "─── EVOLUCIÓN APACHE II ───",
        true,
        11.0,
    );
    y -= 6.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        "  Fecha/Hora              Apache II   GCS   Severidad       Mortalidad",
        true,
        9.0,
    );
    y -= 5.0;

    for m in measurements {
        if y < 20.0 {
            pages.push(std::mem::take(&mut ops));
            y = 270.0;
        }
        let line = format!(
            "  {:<25} {:>8}    {:>3}   {:<12}    {:>6.1}%",
            &m.timestamp[..19],
            m.apache_score,
            m.gcs_score,
            m.severity.label(),
            m.mortality_risk,
        );
        push_text(&mut ops, left, page_h, y, &line, false, 8.0);
        y -= 4.5;
    }

    y -= 5.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!(
            "Estado actual de gravedad: {}",
            patient.estado_gravedad.label()
        ),
        true,
        10.0,
    );
    y -= 4.0;
    push_text(
        &mut ops,
        left,
        page_h,
        y,
        &format!(
            "Mortalidad estimada: {}",
            patient.estado_gravedad.mortality_estimate()
        ),
        false,
        9.0,
    );

    pages.push(ops);

    let pages: Vec<PdfPage> = pages
        .into_iter()
        .map(|ops| PdfPage::new(Mm(210.0), Mm(page_h), ops))
        .collect();
    let mut warnings = Vec::new();
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings);
    Ok(bytes)
}

/// Genera un nombre de archivo seguro para `Content-Disposition`.
///
/// Los datos del paciente (apellido, cédula) son input de usuario: si contienen
/// comillas, `CR`/`LF` o `;` podrían inyectar headers HTTP y romper el
/// `filename="..."`. Se reemplazan caracteres de control por `_` y se eliminan
/// los caracteres peligrosos para header injection.
fn sanitize_filename(patient: &Patient) -> (String, String) {
    fn clean(v: &str) -> String {
        v.chars()
            .map(|c| match c {
                '"' | ';' | '\r' | '\n' | '\\' => '_',
                _ => c,
            })
            .collect()
    }
    (
        clean(&patient.apellido).replace(' ', "_"),
        clean(&patient.cedula),
    )
}

fn error_response(msg: &str) -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(format!(
            "{{\"success\":false,\"error\":\"{}\"}}",
            msg
        )))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmart_shared::models::{Patient, SeverityLevel};

    #[test]
    fn sanitize_filename_prevents_header_injection() {
        let mut patient = Patient::new();
        patient.apellido = "González\";\r\nX-Evil: 1".into();
        patient.cedula = "8801\n011;2345".into();

        let (apellido, cedula) = sanitize_filename(&patient);
        assert!(!apellido.contains('"'), "debe eliminar comillas");
        assert!(!apellido.contains('\r'), "debe eliminar CR");
        assert!(!apellido.contains('\n'), "debe eliminar LF");
        assert!(!apellido.contains(';'), "debe eliminar punto y coma");
        assert!(!cedula.contains('\n'), "cedula sin saltos de línea");

        // no se puede romper el header `filename="..."` con lo sanitizado
        let filename = format!("UCI_{}_{}.csv", apellido, cedula);
        assert!(!filename.contains("\""), "filename sin comillas");
        assert!(
            !filename.contains('\n') && !filename.contains('\r'),
            "filename sin CR/LF"
        );
    }

    #[test]
    fn generate_pdf_produces_valid_document() {
        let mut patient = Patient::new();
        patient.nombre = "María".into();
        patient.apellido = "González".into();
        patient.historia_clinica = "HC-001".into();
        patient.cedula = "88010112345".into();
        patient.diagnostico_uci = "Neumonía severa".into();
        patient.estado_gravedad = SeverityLevel::Critico;

        let mut m = dmart_shared::models::Measurement::new(
            &patient.patient_id,
            dmart_shared::models::ApacheIIData::default(),
            dmart_shared::models::GcsData::default(),
        );
        m.timestamp = "2026-09-13T10:00:00Z".into();
        m.apache_score = 25;
        m.gcs_score = 9;
        m.severity = SeverityLevel::Severo;
        m.mortality_risk = 0.45;

        let bytes = generate_pdf(&patient, &[m]).expect("pdf gen");
        assert!(bytes.len() > 1000, "PDF demasiado corto");
        assert!(bytes.starts_with(b"%PDF"), "debe empezar con cabecera PDF");

        // El contenido de texto viaja comprimido (FlateDecode); la validación
        // estructural fiable es re-parsear el documento que acabamos de emitir
        // (bytes propios, trusted, no input de terceros).
        let mut warnings = Vec::new();
        let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings)
            .expect("el PDF generado debe re-parsear como documento válido");
        assert_eq!(parsed.page_count(), 1, "debe tener una página");
    }
}
