use anyhow::Result;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use dmart_shared::models::{ApiResponse, MfaSettings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::auth::{AuthService, Claims, LoginResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaSetupResponse {
    pub secret: String,
    pub otpauth_uri: String,
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MfaCodeRequest {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub backup_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MfaConfirmRequest {
    pub code: String,
}

fn issuer() -> &'static str {
    static ISSUER: OnceLock<String> = OnceLock::new();
    ISSUER.get_or_init(|| {
        std::env::var("DMART_APP_NAME")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "dMart UCI".to_string())
    })
}

fn new_totp(secret_base32: &str, account: &str) -> Result<TOTP, String> {
    let bytes = Secret::Encoded(secret_base32.to_string())
        .to_bytes()
        .map_err(|e| format!("Secreto TOTP inválido: {e}"))?;
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        bytes,
        Some(issuer().to_string()),
        account.to_string(),
    )
    .map_err(|e| e.to_string())
}

fn code_valid(totp: &TOTP, code: &str) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    totp.check(code, now)
}

fn hash_code(code: &str) -> String {
    format!("{:x}", Sha256::digest(code.trim().as_bytes()))
}

const BACKUP_CODE_COUNT: usize = 10;
const BACKUP_CODE_LEN: usize = 8;

fn generate_backup_codes() -> Vec<String> {
    use rand::Rng;
    let mut csprng = rand::thread_rng();
    (0..BACKUP_CODE_COUNT)
        .map(|_| {
            const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
            (0..BACKUP_CODE_LEN)
                .map(|_| ALPHABET[csprng.gen_range(0..ALPHABET.len())] as char)
                .collect()
        })
        .collect()
}

/// Servicio de segundo factor basado en TOTP RFC 6238 (SHA-1, 6 dígitos, 30 s).
#[derive(Clone)]
pub struct MfaService {
    db: Surreal<Db>,
}

impl MfaService {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }

    /// Inicia la activación de MFA para un usuario: genera el secreto y los
    /// códigos de respaldo. El secreto queda como `pending_secret` hasta que el
    /// usuario confirme con el primer código TOTP.
    pub async fn setup(&self, user_id: &str) -> Result<MfaSetupResponse, String> {
        let secret = Secret::generate_secret();
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret.to_bytes().map_err(|e| e.to_string())?,
            Some(issuer().to_string()),
            user_id.to_string(),
        )
        .map_err(|e| e.to_string())?;
        let secret_str = totp.get_secret_base32();
        let otpauth_uri = totp.get_url();

        let backup_codes = generate_backup_codes();

        let settings = MfaSettings {
            user_id: user_id.to_string(),
            enabled: false,
            secret: None,
            pending_secret: Some(secret_str.clone()),
            backup_codes: backup_codes.iter().map(|c| hash_code(c)).collect(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        crate::db::upsert_mfa_settings(&self.db, settings)
            .await
            .map_err(|e| e.to_string())?;

        Ok(MfaSetupResponse {
            secret: secret_str,
            otpauth_uri,
            backup_codes,
        })
    }

    /// Confirma la activación verificando un código TOTP contra el
    /// `pending_secret`; a partir de aquí MFA queda habilitado.
    pub async fn confirm(&self, user_id: &str, code: &str) -> Result<(), String> {
        let settings = crate::db::get_mfa_settings(&self.db, user_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Activa MFA primero (POST /auth/mfa/setup)".to_string())?;

        let pending = settings
            .pending_secret
            .clone()
            .ok_or_else(|| "No hay configuración MFA pendiente".to_string())?;

        let totp = new_totp(&pending, user_id)?;
        if !code_valid(&totp, code) {
            return Err("Código de verificación inválido".to_string());
        }

        let mut updated = settings;
        updated.enabled = true;
        updated.secret = updated.pending_secret.take();
        crate::db::upsert_mfa_settings(&self.db, updated)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Verifica el código (TOTP o backup) de un usuario con MFA habilitado.
    /// Los códigos de respaldo se consumen (se eliminan tras el primer uso).
    pub async fn verify(
        &self,
        user_id: &str,
        code: Option<&str>,
        backup_code: Option<&str>,
    ) -> Result<bool, String> {
        let settings = crate::db::get_mfa_settings(&self.db, user_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "MFA no configurado".to_string())?;

        if !settings.enabled {
            return Ok(true);
        }

        let secret = settings
            .secret
            .as_deref()
            .ok_or_else(|| "MFA sin secreto configurado".to_string())?;
        let totp = new_totp(secret, user_id)?;

        if let Some(code) = code
            && code_valid(&totp, code)
        {
            return Ok(true);
        }

        if let Some(bk) = backup_code {
            let hashed = hash_code(bk);
            if settings.backup_codes.contains(&hashed) {
                let remaining: Vec<String> = settings
                    .backup_codes
                    .iter()
                    .filter(|c| **c != hashed)
                    .cloned()
                    .collect();
                let mut updated = settings;
                updated.backup_codes = remaining;
                crate::db::upsert_mfa_settings(&self.db, updated)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Desactiva MFA tras verificar el código actual.
    pub async fn disable(&self, user_id: &str, code: &str) -> Result<(), String> {
        let ok = self.verify(user_id, Some(code), None).await?;
        if !ok {
            return Err("Código de verificación inválido".to_string());
        }
        crate::db::delete_mfa_settings(&self.db, user_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Handler: POST /api/auth/mfa/setup
pub async fn setup(
    claims: Claims,
    State(db): State<crate::db::Database>,
) -> Json<ApiResponse<MfaSetupResponse>> {
    let service = MfaService::new((*db).clone());
    match service.setup(&claims.sub).await {
        Ok(res) => Json(ApiResponse::ok(res)),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// Handler: POST /api/auth/mfa/confirm
pub async fn confirm(
    claims: Claims,
    State(db): State<crate::db::Database>,
    Json(req): Json<MfaConfirmRequest>,
) -> Json<ApiResponse<()>> {
    let service = MfaService::new((*db).clone());
    match service.confirm(&claims.sub, &req.code).await {
        Ok(_) => Json(ApiResponse::ok(())),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// Handler: POST /api/auth/mfa/verify
///
/// Recibe el token de reto (scope `mfa`, de corta duración) emitido durante el
/// login y el código del segundo factor. Devuelve el token de sesión completo.
pub async fn verify(
    State(db): State<crate::db::Database>,
    headers: HeaderMap,
    Json(req): Json<MfaCodeRequest>,
) -> Response {
    let auth_service = AuthService::new((*db).clone());
    let challenge = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(crate::auth::extract_token_from_header)
        .ok_or_else(|| "Token de reto requerido".to_string())
        .and_then(|t| auth_service.verify_token(t).map(|c| (t, c)))
        .and_then(|(t, claims)| {
            if claims.scope == "mfa" {
                Ok((t, claims))
            } else {
                Err("Token de reto MFA requerido".to_string())
            }
        });

    let (auth_service, claims) = match challenge {
        Ok((_, claims)) => (auth_service, claims),
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<LoginResponse>::err(e)),
            )
                .into_response();
        }
    };

    let service = MfaService::new((*db).clone());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    match service
        .verify(&claims.sub, req.code.as_deref(), req.backup_code.as_deref())
        .await
    {
        Ok(true) => {
            match auth_service
                .complete_mfa_login(&claims.sub, user_agent, None)
                .await
            {
                Ok(login) => {
                    let mut resp =
                        (StatusCode::OK, Json(ApiResponse::ok(login.clone()))).into_response();
                    if !login.refresh_token.is_empty()
                        && let Ok(value) = axum::http::HeaderValue::from_str(
                            &crate::auth::refresh_cookie(&login.refresh_token),
                        )
                    {
                        resp.headers_mut()
                            .insert(axum::http::header::SET_COOKIE, value);
                    }
                    resp
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<LoginResponse>::err(e)),
                )
                    .into_response(),
            }
        }
        Ok(false) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<LoginResponse>::err(
                "Código de verificación inválido".to_string(),
            )),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<LoginResponse>::err(e)),
        )
            .into_response(),
    }
}

/// Handler: POST /api/auth/mfa/disable
pub async fn disable(
    claims: Claims,
    State(db): State<crate::db::Database>,
    Json(req): Json<MfaConfirmRequest>,
) -> Json<ApiResponse<()>> {
    let service = MfaService::new((*db).clone());
    match service.disable(&claims.sub, &req.code).await {
        Ok(_) => Json(ApiResponse::ok(())),
        Err(e) => Json(ApiResponse::err(e)),
    }
}
