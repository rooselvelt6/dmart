//! SPEC-017: Device Registry — registro y estado de dispositivos clínicos.
//!
//! Modelos y operaciones de la tabla SurrealDB `device_registry` (monitores,
//! ventiladores, bombas de infusión). El estado `online/offline/mantenimiento`
//! se guarda como string; el cambio online↔offline lo decide la ventana de
//! heartbeat: un dispositivo cuyo `last_seen_at` supera
//! `stale_window(heartbeat_interval_secs)` se marca `offline` (salvo que esté
//! en mantenimiento) al recalcularse en listado/summary, y cada heartbeat lo
//! devuelve a `online` renovando `last_seen_at`.
//!
//! El `id` del dispositivo es el record key de SurrealDB, pero siempre se lee
//! proyectado con `meta::id(id) AS id` (string plano): evita el round-trip de
//! `RecordId` por JSON (que serializa como enum) y permite hacer lookups por el
//! mismo string que devuelve la API.

use crate::db::Database;
use crate::realtime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// Intervalo nominal de heartbeat por defecto (segundos).
pub const DEFAULT_HEARTBEAT_INTERVAL_SECS: i64 = 60;

/// Estados operativos del dispositivo (coinciden con el string en BD).
pub const ESTADO_ONLINE: &str = "online";
pub const ESTADO_OFFLINE: &str = "offline";
pub const ESTADO_MANTENIMIENTO: &str = "mantenimiento";

/// Ventana de tolerancia antes de declarar un dispositivo offline: 3× el
/// intervalo nominal (mínimo 60s) — cantidad razonable fija (SPEC-017).
pub fn stale_window(heartbeat_interval_secs: i64) -> i64 {
    (heartbeat_interval_secs * 3).max(60)
}

/// Proyección de columnas con el record key como string plano.
const ROW_FIELDS: &str = "meta::id(id) AS id, device_type, fabricante, modelo, \
     firmware, serial, cama_id, ubicacion, estado, registered_at, \
     last_seen_at, heartbeat_interval_secs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceState {
    Online,
    Offline,
    Mantenimiento,
}

impl DeviceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceState::Online => ESTADO_ONLINE,
            DeviceState::Offline => ESTADO_OFFLINE,
            DeviceState::Mantenimiento => ESTADO_MANTENIMIENTO,
        }
    }
}

impl core::str::FromStr for DeviceState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ESTADO_ONLINE => Ok(DeviceState::Online),
            ESTADO_OFFLINE => Ok(DeviceState::Offline),
            ESTADO_MANTENIMIENTO => Ok(DeviceState::Mantenimiento),
            _ => Err(format!(
                "estado inválido '{s}' (esperado: {ESTADO_ONLINE}/{ESTADO_OFFLINE}/{ESTADO_MANTENIMIENTO})"
            )),
        }
    }
}

/// Un dispositivo clínico registrado en la UCI. `id` es el record key plano.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalDevice {
    pub id: String,
    pub device_type: String,
    pub fabricante: String,
    pub modelo: String,
    pub firmware: String,
    pub serial: String,
    pub cama_id: Option<String>,
    pub ubicacion: Option<String>,
    pub estado: String,
    pub registered_at: i64,
    pub last_seen_at: Option<i64>,
    pub heartbeat_interval_secs: i64,
}

impl ClinicalDevice {
    pub fn is_mantenimiento(&self) -> bool {
        self.estado == ESTADO_MANTENIMIENTO
    }
}

/// Payload de alta desde la API (`register_device`); `id` opcional para que
/// los gateways puedan re-registrar un dispositivo con identidad propia.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceInput {
    pub id: Option<String>,
    pub device_type: String,
    pub fabricante: String,
    pub modelo: String,
    #[serde(default)]
    pub firmware: String,
    pub serial: String,
    #[serde(default)]
    pub cama_id: Option<String>,
    #[serde(default)]
    pub ubicacion: Option<String>,
    #[serde(default)]
    pub estado: Option<String>,
    #[serde(default)]
    pub heartbeat_interval_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCount {
    pub estado: String,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCount {
    pub device_type: String,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatusSummary {
    pub total: u64,
    pub por_estado: Vec<StateCount>,
    pub por_tipo: Vec<TypeCount>,
}

/// Registra un dispositivo; genera `id` (record key) si no viene en el payload.
#[allow(clippy::result_large_err)]
pub async fn register(
    db: &Database,
    input: RegisterDeviceInput,
) -> Result<ClinicalDevice, surrealdb::Error> {
    let id = input
        .id
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let estado = input
        .estado
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ESTADO_ONLINE.to_string());
    let now = Utc::now().timestamp_millis();

    let device = ClinicalDevice {
        id: id.clone(),
        device_type: input.device_type,
        fabricante: input.fabricante,
        modelo: input.modelo,
        firmware: input.firmware,
        serial: input.serial,
        cama_id: input.cama_id,
        ubicacion: input.ubicacion,
        estado,
        registered_at: now,
        last_seen_at: None,
        heartbeat_interval_secs: input
            .heartbeat_interval_secs
            .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SECS),
    };

    let sql = r#"
        CREATE device_registry CONTENT {
            id: $id,
            device_type: $device_type,
            fabricante: $fabricante,
            modelo: $modelo,
            firmware: $firmware,
            serial: $serial,
            cama_id: $cama_id,
            ubicacion: $ubicacion,
            estado: $estado,
            registered_at: $registered_at,
            last_seen_at: NONE,
            heartbeat_interval_secs: $heartbeat_interval_secs
        }
    "#;
    db.query(sql)
        .bind((
            "id",
            surrealdb::RecordId::from(("device_registry", id.clone())),
        ))
        .bind(("device_type", device.device_type.clone()))
        .bind(("fabricante", device.fabricante.clone()))
        .bind(("modelo", device.modelo.clone()))
        .bind(("firmware", device.firmware.clone()))
        .bind(("serial", device.serial.clone()))
        .bind(("cama_id", device.cama_id.clone()))
        .bind(("ubicacion", device.ubicacion.clone()))
        .bind(("estado", device.estado.clone()))
        .bind(("registered_at", device.registered_at))
        .bind(("heartbeat_interval_secs", device.heartbeat_interval_secs))
        .await?;

    realtime::publish(
        "device",
        json!({ "event": "registered", "device_id": device.id, "estado": device.estado }),
    );

    Ok(device)
}

/// Obtiene un dispositivo por su id (record key plano).
#[allow(clippy::result_large_err)]
pub async fn get(db: &Database, id: &str) -> Result<Option<ClinicalDevice>, surrealdb::Error> {
    let sql = format!("SELECT {ROW_FIELDS} FROM device_registry WHERE meta::id(id) = $id LIMIT 1");
    let mut q = db.query(sql).bind(("id", id.to_string())).await?;
    let device: Option<ClinicalDevice> = q.take(0)?;
    Ok(device)
}

/// Recalcula el estado derivado de la ventana de heartbeat: cualquier
/// dispositivo (no en mantenimiento) cuyo `last_seen_at` sea más antiguo que
/// `stale_window(heartbeat_interval_secs)` pasa a `offline`. Devuelve cuántos
/// registros se marcaron en esta pasada.
#[allow(clippy::result_large_err)]
pub async fn reconcile_stale_devices(db: &Database) -> Result<usize, surrealdb::Error> {
    let now = Utc::now().timestamp_millis();

    let sql = r#"
        SELECT meta::id(id) AS device_id,
               last_seen_at,
               heartbeat_interval_secs
        FROM device_registry
        WHERE estado != $mantenimiento
          AND last_seen_at != NONE
    "#;
    let mut q = db
        .query(sql)
        .bind(("mantenimiento", ESTADO_MANTENIMIENTO))
        .await?;
    let rows: Vec<serde_json::Value> = q.take(0)?;

    let to_offline: Vec<String> = rows
        .into_iter()
        .filter_map(|row| {
            let id = row.get("device_id")?.as_str()?;
            let last_seen = row.get("last_seen_at")?.as_i64()?;
            let interval = row
                .get("heartbeat_interval_secs")?
                .as_i64()
                .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SECS);
            (last_seen < now - stale_window(interval)).then(|| id.to_string())
        })
        .collect();

    let count = to_offline.len();
    if count > 0 {
        let sql = r#"
            UPDATE device_registry SET estado = $offline
            WHERE meta::id(id) IN $ids
              AND estado != $mantenimiento
        "#;
        db.query(sql)
            .bind(("offline", ESTADO_OFFLINE))
            .bind(("ids", to_offline))
            .bind(("mantenimiento", ESTADO_MANTENIMIENTO))
            .await?;
    }
    Ok(count)
}

/// Lista dispositivos (opcionalmente filtrados por estado), recalculando antes
/// los estados offline según la ventana de heartbeat.
#[allow(clippy::result_large_err)]
pub async fn list(
    db: &Database,
    estado: Option<&str>,
) -> Result<Vec<ClinicalDevice>, surrealdb::Error> {
    reconcile_stale_devices(db).await?;
    let devices: Vec<ClinicalDevice> = if let Some(estado) = estado.filter(|s| !s.is_empty()) {
        let sql = format!(
            "SELECT {ROW_FIELDS} FROM device_registry WHERE estado = $estado ORDER BY registered_at ASC"
        );
        db.query(sql)
            .bind(("estado", estado.to_string()))
            .await?
            .take(0)?
    } else {
        let sql = format!("SELECT {ROW_FIELDS} FROM device_registry ORDER BY registered_at ASC");
        db.query(sql).await?.take(0)?
    };
    Ok(devices)
}

/// Registra un heartbeat: renueva `last_seen_at`, devuelve el dispositivo a
/// `online` (preservando `mantenimiento`) y publica el evento en tiempo real.
/// Devuelve `Ok(None)` si el dispositivo no existe.
#[allow(clippy::result_large_err)]
pub async fn heartbeat(
    db: &Database,
    id: &str,
) -> Result<Option<ClinicalDevice>, surrealdb::Error> {
    let Some(mut device) = get(db, id).await? else {
        return Ok(None);
    };

    let now = Utc::now().timestamp_millis();
    device.last_seen_at = Some(now);
    if !device.is_mantenimiento() {
        device.estado = ESTADO_ONLINE.to_string();
    }

    let sql = r#"
        UPDATE device_registry
        SET estado = $estado, last_seen_at = $last_seen_at
        WHERE meta::id(id) = $id
    "#;
    db.query(sql)
        .bind(("estado", device.estado.clone()))
        .bind(("last_seen_at", now))
        .bind(("id", id.to_string()))
        .await?;

    realtime::publish(
        "device",
        json!({
            "event": "heartbeat",
            "device_id": device.id,
            "estado": device.estado,
            "last_seen_at": now
        }),
    );

    Ok(Some(device))
}

/// Agregación por estado y por tipo de dispositivo (GROUP BY SurrealQL).
/// Devuelve también el total general y recalcula estados offline.
#[allow(clippy::result_large_err)]
pub async fn status_summary(db: &Database) -> Result<DeviceStatusSummary, surrealdb::Error> {
    reconcile_stale_devices(db).await?;

    let por_estado: Vec<serde_json::Value> = db
        .query("SELECT estado, count() AS total FROM device_registry GROUP BY estado")
        .await?
        .take(0)?;
    let por_tipo: Vec<serde_json::Value> = db
        .query("SELECT device_type, count() AS total FROM device_registry GROUP BY device_type")
        .await?
        .take(0)?;

    let mut por_estado_vec: Vec<StateCount> = por_estado
        .into_iter()
        .filter_map(|row| {
            Some(StateCount {
                estado: row.get("estado")?.as_str()?.to_string(),
                total: row.get("total")?.as_u64()?,
            })
        })
        .collect();
    por_estado_vec.sort_by_key(|a| std::cmp::Reverse(a.total));

    let mut por_tipo_vec: Vec<TypeCount> = por_tipo
        .into_iter()
        .filter_map(|row| {
            Some(TypeCount {
                device_type: row.get("device_type")?.as_str()?.to_string(),
                total: row.get("total")?.as_u64()?,
            })
        })
        .collect();
    por_tipo_vec.sort_by_key(|a| std::cmp::Reverse(a.total));

    // `count()` sin GROUP BY no respeta el alias en SurrealDB (devuelve la
    // clave `count` por fila), así que el total se deduce de la agregación por
    // estado (la tabla ya quedó reconciliada arriba).
    let total = por_estado_vec.iter().map(|c| c.total).sum();

    Ok(DeviceStatusSummary {
        total,
        por_estado: por_estado_vec,
        por_tipo: por_tipo_vec,
    })
}
