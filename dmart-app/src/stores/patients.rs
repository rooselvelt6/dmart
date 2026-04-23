use gloo_storage::{LocalStorage, Storage};
use dmart_shared::models::*;
use crate::api;

const CACHE_KEY: &str = "dmart_patients_cache";
const CACHE_TTL_SECS: u64 = 60;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    patients: Vec<PatientListItem>,
    timestamp: u64,
}

fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn load_patients_cached() -> Option<Vec<PatientListItem>> {
    LocalStorage::get::<CacheEntry>(CACHE_KEY).ok().and_then(|entry| {
        if get_current_timestamp() - entry.timestamp < CACHE_TTL_SECS {
            Some(entry.patients)
        } else {
            None
        }
    })
}

pub fn save_patients_cached(patients: &[PatientListItem]) {
    let entry = CacheEntry {
        patients: patients.to_vec(),
        timestamp: get_current_timestamp(),
    };
    let _ = LocalStorage::set(CACHE_KEY, entry);
}

pub async fn fetch_patients_cached() -> Vec<PatientListItem> {
    if let Some(cached) = load_patients_cached() {
        return cached;
    }
    
    match api::list_patients(None).await {
        Ok(patients) => {
            save_patients_cached(&patients);
            patients
        }
        Err(_) => Vec::new()
    }
}