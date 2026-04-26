use axum::{
    extract::{Path, State},
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

// ─── Admin Stats ───────────────────────────────────────────────────

pub async fn get_admin_stats(State(db): State<Database>) -> ApiResult<AdminStats> {
    let _pacientes = crate::db::list_patients(&db).await.map_err(err_to_str)?;
    let camas = crate::db::list_camas(&db).await.map_err(err_to_str)?;
    let equipos = crate::db::list_equipos(&db).await.map_err(err_to_str)?;
    let users = crate::db::list_users(&db).await.map_err(err_to_str)?;

    let stats = AdminStats {
        total_camas: camas.len() as u8,
        camas_libres: camas.iter().filter(|c| c.estado == EstadoCama::Libre).count() as u8,
        camas_ocupadas: camas.iter().filter(|c| c.estado == EstadoCama::Ocupada).count() as u8,
        camas_mantenimiento: camas.iter().filter(|c| c.estado == EstadoCama::Mantenimiento || c.estado == EstadoCama::Limpieza).count() as u8,
        total_equipos: equipos.len() as u32,
        equipos_activos: equipos.iter().filter(|e| e.estado == EstadoEquipo::Activo).count() as u32,
        equipos_mantenimiento: equipos.iter().filter(|e| e.estado == EstadoEquipo::Mantenimiento || e.estado == EstadoEquipo::Reparacion).count() as u32,
        total_staff: users.len() as u32,
        medicos_activos: users.iter().filter(|u| u.rol == UserRole::Medico && u.activo).count() as u32,
        enfermeros_activos: users.iter().filter(|u| u.rol == UserRole::Enfermero && u.activo).count() as u32,
    };

    Ok(Json(ApiResponse::ok(stats)))
}

// ─── Camas API ───────────────────────────────────────────────────

pub async fn init_camas_api(
    State(db): State<Database>,
    Json(req): Json<InitCamasRequest>,
) -> ApiResult<Vec<Cama>> {
    let existentes = crate::db::list_camas(&db).await.map_err(err_to_str)?;
    if !existentes.is_empty() {
        return Ok(Json(ApiResponse::err("Las camas ya están inicializadas")));
    }

    let camas = crate::db::init_camas(&db, req.cantidad).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(camas)))
}

#[derive(serde::Deserialize)]
pub struct InitCamasRequest {
    cantidad: u8,
}

pub async fn list_camas_api(State(db): State<Database>) -> ApiResult<Vec<Cama>> {
    let camas = crate::db::list_camas(&db).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(camas)))
}

pub async fn get_cama_api(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> ApiResult<Cama> {
    let cama = crate::db::get_cama(&db, &id).await.map_err(err_to_str)?;
    match cama {
        Some(c) => Ok(Json(ApiResponse::ok(c))),
        None => Ok(Json(ApiResponse::err("Cama no encontrada"))),
    }
}

pub async fn update_cama_api(
    State(db): State<Database>,
    Path(id): Path<String>,
    Json(cama): Json<Cama>,
) -> ApiResult<Cama> {
    let updated = crate::db::update_cama(&db, &id, cama).await.map_err(err_to_str)?;
    match updated {
        Some(c) => Ok(Json(ApiResponse::ok(c))),
        None => Ok(Json(ApiResponse::err("Cama no encontrada"))),
    }
}

pub async fn delete_cama_api(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> ApiResult<()> {
    crate::db::delete_cama(&db, &id).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(())))
}

pub async fn get_camas_disponibles(State(db): State<Database>) -> ApiResult<Vec<Cama>> {
    let todas = crate::db::list_camas(&db).await.map_err(err_to_str)?;
    let disponibles: Vec<Cama> = todas.into_iter()
        .filter(|c| c.estado == EstadoCama::Libre)
        .collect();
    Ok(Json(ApiResponse::ok(disponibles)))
}

// ─── Equipos API ───────────────────────────────────────────────────

pub async fn list_equipos_api(State(db): State<Database>) -> ApiResult<Vec<Equipo>> {
    let equipos = crate::db::list_equipos(&db).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(equipos)))
}

pub async fn create_equipo_api(
    State(db): State<Database>,
    Json(equipo): Json<Equipo>,
) -> ApiResult<Equipo> {
    let created = crate::db::create_equipo(&db, equipo).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(created)))
}

pub async fn get_equipo_api(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> ApiResult<Equipo> {
    let equipo = crate::db::get_equipo(&db, &id).await.map_err(err_to_str)?;
    match equipo {
        Some(e) => Ok(Json(ApiResponse::ok(e))),
        None => Ok(Json(ApiResponse::err("Equipo no encontrado"))),
    }
}

pub async fn update_equipo_api(
    State(db): State<Database>,
    Path(id): Path<String>,
    Json(equipo): Json<Equipo>,
) -> ApiResult<Equipo> {
    let updated = crate::db::update_equipo(&db, &id, equipo).await.map_err(err_to_str)?;
    match updated {
        Some(e) => Ok(Json(ApiResponse::ok(e))),
        None => Ok(Json(ApiResponse::err("Equipo no encontrado"))),
    }
}

pub async fn delete_equipo_api(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> ApiResult<()> {
    crate::db::delete_equipo(&db, &id).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(())))
}

pub async fn list_equipos_por_cama_api(
    State(db): State<Database>,
    Path(cama_id): Path<String>,
) -> ApiResult<Vec<Equipo>> {
    let equipos = crate::db::list_equipos_por_cama(&db, &cama_id).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(equipos)))
}

pub async fn asignar_equipo_cama_api(
    State(db): State<Database>,
    Json(req): Json<AsignarEquipoCamaRequest>,
) -> ApiResult<Option<Equipo>> {
    let updated = crate::db::asignar_equipo_cama(&db, &req.equipo_id, &req.cama_id)
        .await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(updated)))
}

#[derive(serde::Deserialize)]
pub struct AsignarEquipoCamaRequest {
    equipo_id: String,
    cama_id: String,
}

pub async fn desvincular_equipo_api(
    State(db): State<Database>,
    Path(equipo_id): Path<String>,
) -> ApiResult<Option<Equipo>> {
    let updated = crate::db::desvincular_equipo_cama(&db, &equipo_id)
        .await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(updated)))
}

// ─── Staff Users API ─────────────────────────────────────────────

pub async fn list_staff_api(State(db): State<Database>) -> ApiResult<Vec<User>> {
    let staff = crate::db::list_staff(&db).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(staff)))
}

pub async fn create_staff_api(
    State(db): State<Database>,
    Json(user): Json<User>,
) -> ApiResult<User> {
    let created = crate::db::create_user(&db, user).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(created)))
}

pub async fn get_staff_api(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> ApiResult<User> {
    let user = crate::db::get_user(&db, &id).await.map_err(err_to_str)?;
    match user {
        Some(u) => Ok(Json(ApiResponse::ok(u))),
        None => Ok(Json(ApiResponse::err("Usuario no encontrado"))),
    }
}

pub async fn update_staff_api(
    State(db): State<Database>,
    Path(id): Path<String>,
    Json(user): Json<User>,
) -> ApiResult<User> {
    let updated = crate::db::update_user(&db, &id, user).await.map_err(err_to_str)?;
    match updated {
        Some(u) => Ok(Json(ApiResponse::ok(u))),
        None => Ok(Json(ApiResponse::err("Usuario no encontrado"))),
    }
}

pub async fn delete_staff_api(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> ApiResult<()> {
    crate::db::delete_user(&db, &id).await.map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(())))
}

pub async fn toggle_user_active(
    State(db): State<Database>,
    Path(id): Path<String>,
) -> ApiResult<User> {
    let user = crate::db::get_user(&db, &id).await.map_err(err_to_str)?;
    if let Some(mut u) = user {
        u.activo = !u.activo;
        let updated = crate::db::update_user(&db, &id, u).await.map_err(err_to_str)?;
        match updated {
            Some(user) => Ok(Json(ApiResponse::ok(user))),
            None => Ok(Json(ApiResponse::err("Usuario no encontrado"))),
        }
    } else {
        Ok(Json(ApiResponse::err("Usuario no encontrado")))
    }
}

// ─── Asignar paciente a cama (para registro de paciente) ───────────

pub async fn check_camas_disponibles(State(db): State<Database>) -> ApiResult<CheckCamasResponse> {
    let cama_libre = crate::db::get_cama_libre(&db).await.map_err(err_to_str)?;
    
    match cama_libre {
        Some(c) => Ok(Json(ApiResponse::ok(CheckCamasResponse {
            disponible: true,
            cama_id: Some(c.cama_id),
            numero: c.numero,
        }))),
        None => Ok(Json(ApiResponse::ok(CheckCamasResponse {
            disponible: false,
            cama_id: None,
            numero: 0,
        }))),
    }
}

#[derive(serde::Serialize)]
pub struct CheckCamasResponse {
    pub disponible: bool,
    pub cama_id: Option<String>,
    pub numero: u8,
}