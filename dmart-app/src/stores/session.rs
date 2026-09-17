/// Sesión del usuario: identidad, rol y renovación del access token.
use dmart_shared::models::{LoginResponse, UserInfo};
use gloo_storage::{LocalStorage, Storage};
use gloo_timers::future::TimeoutFuture;
use leptos::task::spawn_local;

const AUTH_KEY: &str = "dmart_auth";
const USER_KEY: &str = "dmart_user";

/// Guarda el par (access token, identidad) tras login/refresh.
pub fn save_session(resp: &LoginResponse) {
    LocalStorage::set(AUTH_KEY, &resp.token).ok();
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

/// True mientras exista access token (independiente de la identidad).
pub fn has_token() -> bool {
    LocalStorage::get::<String>(AUTH_KEY).is_ok()
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
    LocalStorage::delete(AUTH_KEY);
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
