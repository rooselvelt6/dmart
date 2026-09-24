use crate::api;
use dmart_shared::models::*;
use gloo_storage::{LocalStorage, Storage};
use std::sync::{Mutex, OnceLock};

/// Clave legacy de la versión anterior (lista de pacientes con PHI en
/// `localStorage`). Se borra al pasar por la caché para no dejar datos clínicos
/// persistidos.
const CACHE_KEY_LEGACY: &str = "dmart_patients_cache";
const CACHE_TTL_SECS: u64 = 60;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    patients: Vec<PatientListItem>,
    timestamp: u64,
}

/// Caché de pacientes SOLO en memoria (nunca en `localStorage`): evita dejar
/// PHI persistida en el navegador expuesta a XSS. WASM es single-threaded →
/// `Mutex` de sobra.
static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

fn cache_slot() -> &'static Mutex<Option<CacheEntry>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn get_current_timestamp() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

pub fn load_patients_cached() -> Option<Vec<PatientListItem>> {
    // Elimina la caché legacy (PHI en localStorage) de la versión anterior.
    LocalStorage::delete(CACHE_KEY_LEGACY);
    let entry: CacheEntry = cache_slot().lock().ok()?.clone()?;
    if get_current_timestamp() - entry.timestamp < CACHE_TTL_SECS {
        Some(entry.patients)
    } else {
        None
    }
}

pub fn save_patients_cached(patients: &[PatientListItem]) {
    LocalStorage::delete(CACHE_KEY_LEGACY);
    let entry = CacheEntry {
        patients: patients.to_vec(),
        timestamp: get_current_timestamp(),
    };
    if let Ok(mut slot) = cache_slot().lock() {
        *slot = Some(entry);
    }
}

pub async fn fetch_patients_cached() -> Vec<PatientListItem> {
    if let Some(cached) = load_patients_cached() {
        return cached;
    }

    match api::list_patients(None, Some("activos")).await {
        Ok(patients) => {
            save_patients_cached(&patients);
            patients
        }
        Err(_) => Vec::new(),
    }
}
