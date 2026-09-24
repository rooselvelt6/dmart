/// Sesión del usuario: identidad, rol y renovación del access token.
///
/// El access token se guarda SOLO en memoria (nunca en `localStorage` ni en la
/// URL), para no dejar material de sesión expuesto a XSS persistente. La
/// identidad puede persistir en `localStorage` (nombre/rol para el gating tras
/// una recarga); la sesión real se reanuda con `/auth/refresh` (cookie
/// httpOnly) re-parseada en `app.rs`.
use dmart_shared::models::{LoginResponse, UserInfo};
use gloo_storage::{LocalStorage, Storage};
use gloo_timers::future::TimeoutFuture;
use std::sync::{Mutex, OnceLock};
use wasm_bindgen_futures::spawn_local;

/// Clave legacy de la versión anterior (token en `localStorage`). Se borra al
/// pasar por `save_session`/`clear_session` para no dejar restos.
const AUTH_KEY_LEGACY: &str = "dmart_auth";
const USER_KEY: &str = "dmart_user";

/// Access token en memoria. WASM es single-threaded → un `Mutex` es de sobra.
static ACCESS_TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn token_slot() -> &'static Mutex<Option<String>> {
    ACCESS_TOKEN.get_or_init(|| Mutex::new(None))
}

/// Access token vigente (se envía solo por `Authorization: Bearer`, nunca en la URL).
pub fn access_token() -> Option<String> {
    token_slot().lock().ok().and_then(|slot| slot.clone())
}

/// Guarda el par (access token en memoria, identidad en localStorage) tras login/refresh.
pub fn save_session(resp: &LoginResponse) {
    if let Ok(mut slot) = token_slot().lock() {
        *slot = Some(resp.token.clone());
    }
    LocalStorage::delete(AUTH_KEY_LEGACY);
    LocalStorage::set(USER_KEY, &resp.user).ok();
}

/// Guarda solo la identidad (p. ej. al recuperarla vía `GET /auth/me`).
pub fn save_user(user: &UserInfo) {
    LocalStorage::set(USER_KEY, user).ok();
}

/// Usuario autenticado (si hay sesión guardada).
pub fn current_user() -> Option<UserInfo> {
    LocalStorage::get::<UserInfo>(USER_KEY).ok()
}

/// True mientras exista access token en memoria (independiente de la identidad).
pub fn has_token() -> bool {
    access_token().is_some()
}

/// Permiso RBAC del usuario actual (`patients:create`, `users:read`, ...).
pub fn user_has(permission: &str) -> bool {
    current_user().is_some_and(|u| u.rol.can(permission))
}

/// Solo Administrador: controla el panel de administración y las acciones de
/// personal, camas y equipos.
pub fn is_admin() -> bool {
    user_has("users:read")
}

/// Limpia la sesión local (logout o expiración).
pub fn clear_session() {
    if let Ok(mut slot) = token_slot().lock() {
        *slot = None;
    }
    LocalStorage::delete(AUTH_KEY_LEGACY);
    LocalStorage::delete(USER_KEY);
}

/// Renovación periódica del access token: intenta `/auth/refresh` cada 10 min
/// y actualiza token e identidad. Si la sesión expiró localmente, se detiene.
pub fn start_session_refresh() {
    spawn_local(async move {
        loop {
            TimeoutFuture::new(10 * 60 * 1000).await;
            if !has_token() {
                break;
            }
            match crate::api::refresh_session().await {
                Ok(resp) => save_session(&resp),
                Err(_) => {
                    // El refresh puede fallar porque la cookie Secure no viaja
                    // sobre HTTP puro, o porque la sesión fue revocada. En ese
                    // caso el access token sigue válido hasta su expiración.
                }
            }
        }
    });
}
