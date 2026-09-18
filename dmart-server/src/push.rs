//! SPEC-052 / tarea 3.9 — Web Push (VAPID) para alertas clínicas.
//!
//! El backend firma los mensajes con VAPID (ES256) y cifra el payload con
//! `aes128gcm` (RFC 8291) usando `ring` (sin dependencias nuevas más allá de
//! las ya presentes vía `jsonwebtoken`). Las suscripciones del navegador se
//! guardan en `push_subscription` y las claves VAPID en `push_config`.
//!
//! El envío es *best-effort*: nunca debe romper el flujo clínico que lo dispara.

use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use chrono::Utc;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_128_GCM};
use ring::agreement::{self, UnparsedPublicKey, ECDH_P256};
use ring::hkdf;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{EcdsaKeyPair, ECDSA_P256_SHA256_ASN1_SIGNING, KeyPair};
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::escalation::Severity;

const PUSH_CONFIG_PUBLIC: &str = "vapid_public";
const PUSH_CONFIG_PRIVATE: &str = "vapid_private";
const VAPID_DEFAULT_SUBJECT: &str = "mailto:admin@dmart.local";
const RECORD_SIZE: u32 = 4096;

static GLOBAL_PUSH: OnceLock<PushService> = OnceLock::new();

pub fn push() -> Option<&'static PushService> {
    GLOBAL_PUSH.get()
}

fn b64url_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn b64url_decode(input: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input.trim().trim_end_matches('='))
        .map_err(|e| format!("base64url inválido: {e}"))
}

/// Fila persistida de una suscripción Web Push.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushSubscriptionRow {
    pub user_id: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
    pub created_at: String,
}

/// Datos de entrada para registrar una suscripción.
#[derive(Debug, Clone, Deserialize)]
pub struct NewSubscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
}

/// Servicio de Web Push con claves VAPID cargadas o generadas.
pub struct PushService {
    public_key: String,
    private_pkcs8: Vec<u8>,
    subject: String,
    client: reqwest::Client,
}

impl PushService {
    /// Carga las claves VAPID de `push_config` o, si faltan, las genera y
    /// persiste. Los env `DMART_VAPID_*` tienen prioridad.
    pub async fn load_or_generate(db: Surreal<Db>) -> Result<Self, String> {
        let subject =
            std::env::var("DMART_VAPID_SUBJECT").unwrap_or_else(|_| VAPID_DEFAULT_SUBJECT.to_string());

        let (public_key, private_pkcs8) = match (
            std::env::var("DMART_VAPID_PUBLIC_KEY").ok(),
            std::env::var("DMART_VAPID_PRIVATE_KEY").ok(),
        ) {
            (Some(pubk), Some(privk)) => (pubk, b64url_decode(&privk)?),
            _ => load_or_create_keys(&db).await?,
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| format!("http client: {e}"))?;

        Ok(Self {
            public_key,
            private_pkcs8,
            subject,
            client,
        })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// Registra (o actualiza) una suscripción para el usuario. Devuelve `true`
    /// si era nueva.
    pub async fn subscribe(
        &self,
        db: &Surreal<Db>,
        user_id: &str,
        sub: NewSubscription,
    ) -> Result<bool, String> {
        let existing: Vec<PushSubscriptionRow> = db
            .query("SELECT * FROM push_subscription WHERE user_id = $uid AND endpoint = $ep LIMIT 1")
            .bind(("uid", user_id.to_string()))
            .bind(("ep", sub.endpoint.clone()))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;

        if !existing.is_empty() {
            db.query(
                "UPDATE push_subscription SET p256dh = $p, auth = $a, user_agent = $ua \
                 WHERE user_id = $uid AND endpoint = $ep",
            )
            .bind(("p", sub.p256dh))
            .bind(("a", sub.auth))
            .bind(("ua", sub.user_agent))
            .bind(("uid", user_id.to_string()))
            .bind(("ep", sub.endpoint))
            .await
            .map_err(|e| e.to_string())?;
            return Ok(false);
        }

        let row = PushSubscriptionRow {
            user_id: user_id.to_string(),
            endpoint: sub.endpoint,
            p256dh: sub.p256dh,
            auth: sub.auth,
            user_agent: sub.user_agent,
            created_at: Utc::now().to_rfc3339(),
        };
        db.create::<Option<PushSubscriptionRow>>("push_subscription")
            .content(row)
            .await
            .map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub async fn unsubscribe(
        &self,
        db: &Surreal<Db>,
        user_id: &str,
        endpoint: &str,
    ) -> Result<usize, String> {
        let removed: Vec<PushSubscriptionRow> = db
            .query("DELETE push_subscription WHERE user_id = $uid AND endpoint = $ep RETURN BEFORE")
            .bind(("uid", user_id.to_string()))
            .bind(("ep", endpoint.to_string()))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())?;
        Ok(removed.len())
    }

    async fn all_subscriptions(db: &Surreal<Db>) -> Result<Vec<PushSubscriptionRow>, String> {
        db.query("SELECT * FROM push_subscription")
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())
    }

    async fn subscriptions_for_user(
        db: &Surreal<Db>,
        user_id: &str,
    ) -> Result<Vec<PushSubscriptionRow>, String> {
        db.query("SELECT * FROM push_subscription WHERE user_id = $uid")
            .bind(("uid", user_id.to_string()))
            .await
            .map_err(|e| e.to_string())?
            .take(0)
            .map_err(|e| e.to_string())
    }

    /// Envía una notificación a un usuario. Devuelve cuántas entregas se
    /// intentaron con éxito.
    pub async fn send_to_user(
        &self,
        db: &Surreal<Db>,
        user_id: &str,
        title: &str,
        body: &str,
    ) -> usize {
        let subs = match Self::subscriptions_for_user(db, user_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("push: no se pudieron leer suscripciones: {e}");
                return 0;
            }
        };
        self.send_to_subs(db, subs, title, body).await
    }

    /// Envía una notificación a todas las suscripciones. Devuelve
    /// `(enviadas, fallidas)`.
    pub async fn send_to_all(&self, db: &Surreal<Db>, title: &str, body: &str) -> (usize, usize) {
        let subs = match Self::all_subscriptions(db).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("push: no se pudieron leer suscripciones: {e}");
                return (0, 0);
            }
        };
        let total = subs.len();
        let sent = self.send_to_subs(db, subs, title, body).await;
        (sent, total.saturating_sub(sent))
    }

    async fn send_to_subs(
        &self,
        db: &Surreal<Db>,
        subs: Vec<PushSubscriptionRow>,
        title: &str,
        body: &str,
    ) -> usize {
        let payload = json!({
            "title": title,
            "body": body,
            "tag": "dmart-alert",
            "data": { "url": "/escalation" }
        })
        .to_string();
        let mut sent = 0usize;
        for sub in subs {
            match self.send_one(&sub, payload.as_bytes()).await {
                Ok(()) => sent += 1,
                Err(SendError::Gone) => {
                    let _ = self.unsubscribe(db, &sub.user_id, &sub.endpoint).await;
                }
                Err(SendError::Failed(e)) => {
                    tracing::warn!("push: envío falló a {}: {e}", sub.endpoint);
                }
            }
        }
        sent
    }

    async fn send_one(&self, sub: &PushSubscriptionRow, payload: &[u8]) -> Result<(), SendError> {
        let ua_public = b64url_decode(&sub.p256dh).map_err(SendError::Failed)?;
        let auth_secret = b64url_decode(&sub.auth).map_err(SendError::Failed)?;
        if ua_public.len() != 65 || auth_secret.len() != 16 {
            return Err(SendError::Failed(
                "claves de suscripción con longitud inválida".to_string(),
            ));
        }

        let url = reqwest::Url::parse(&sub.endpoint).map_err(|e| SendError::Failed(e.to_string()))?;
        let audience = url.origin().ascii_serialization();
        let authorization = self.vapid_authorization(&audience).map_err(SendError::Failed)?;

        let body = encrypt_aes128gcm(payload, &ua_public, &auth_secret)
            .map_err(SendError::Failed)?;

        let resp = self
            .client
            .post(sub.endpoint.clone())
            .header("Authorization", authorization)
            .header("Content-Encoding", "aes128gcm")
            .header("Content-Type", "application/octet-stream")
            .header("TTL", "60")
            .body(body)
            .send()
            .await
            .map_err(|e| SendError::Failed(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else if status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::GONE
        {
            Err(SendError::Gone)
        } else {
            Err(SendError::Failed(format!("push service status {status}")))
        }
    }

    /// Construye el header `Authorization` VAPID con un JWT ES256.
    fn vapid_authorization(&self, audience: &str) -> Result<String, String> {
        let header = json!({ "typ": "JWT", "alg": "ES256" });
        let exp = Utc::now().timestamp() + 12 * 3600;
        let claims = json!({
            "aud": audience,
            "exp": exp,
            "sub": self.subject,
        });
        let header_b64 = b64url_encode(&serde_json::to_vec(&header).map_err(|e| e.to_string())?);
        let claims_b64 = b64url_encode(&serde_json::to_vec(&claims).map_err(|e| e.to_string())?);
        let signing_input = format!("{header_b64}.{claims_b64}");

        let rng = SystemRandom::new();
        let key = EcdsaKeyPair::from_pkcs8(
            &ECDSA_P256_SHA256_ASN1_SIGNING,
            &self.private_pkcs8,
            &rng,
        )
        .map_err(|e| format!("VAPID key inválida: {e}"))?;
        let sig = key
            .sign(&rng, signing_input.as_bytes())
            .map_err(|e| format!("VAPID sign: {e}"))?;

        Ok(format!(
            "vapid t={signing_input}.{}, k={}",
            b64url_encode(sig.as_ref()),
            self.public_key
        ))
    }
}

enum SendError {
    Gone,
    Failed(String),
}

async fn load_or_create_keys(db: &Surreal<Db>) -> Result<(String, Vec<u8>), String> {
    let existing: Vec<PushConfigRow> = db
        .query("SELECT key, value FROM push_config WHERE key IN [$pub, $priv]")
        .bind(("pub", PUSH_CONFIG_PUBLIC.to_string()))
        .bind(("priv", PUSH_CONFIG_PRIVATE.to_string()))
        .await
        .map_err(|e| e.to_string())?
        .take(0)
        .map_err(|e| e.to_string())?;

    let public = existing
        .iter()
        .find(|r| r.key == PUSH_CONFIG_PUBLIC)
        .map(|r| r.value.clone());
    let private = existing
        .iter()
        .find(|r| r.key == PUSH_CONFIG_PRIVATE)
        .map(|r| r.value.clone());

    if let (Some(pubk), Some(privk)) = (public, private) {
        return Ok((pubk, b64url_decode(&privk)?));
    }

    generate_and_store_keys(db).await
}

async fn generate_and_store_keys(db: &Surreal<Db>) -> Result<(String, Vec<u8>), String> {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
        .map_err(|e| format!("generar VAPID: {e}"))?;
    let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
        .map_err(|e| format!("cargar VAPID: {e}"))?;
    let public = b64url_encode(key.public_key().as_ref());
    let private_b64 = b64url_encode(pkcs8.as_ref());

    for (key, value) in [
        (PUSH_CONFIG_PUBLIC, public.clone()),
        (PUSH_CONFIG_PRIVATE, private_b64.clone()),
    ] {
        db.query("UPSERT push_config SET key = $k, value = $v WHERE key = $k")
            .bind(("k", key.to_string()))
            .bind(("v", value))
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok((public, pkcs8.as_ref().to_vec()))
}

#[derive(Debug, Clone, Deserialize)]
struct PushConfigRow {
    key: String,
    value: String,
}

struct Len(usize);

impl hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Result<Vec<u8>, String> {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, salt);
    let prk = salt.extract(ikm);
    let info = [info];
    let okm = prk
        .expand(&info, Len(len))
        .map_err(|_| "hkdf expand".to_string())?;
    let mut out = vec![0u8; len];
    okm.fill(&mut out).map_err(|_| "hkdf fill".to_string())?;
    Ok(out)
}

/// Cifra un payload según RFC 8291 (`aes128gcm`, single record).
fn encrypt_aes128gcm(
    payload: &[u8],
    ua_public: &[u8],
    auth_secret: &[u8],
) -> Result<Vec<u8>, String> {
    let rng = SystemRandom::new();

    let eph = agreement::EphemeralPrivateKey::generate(&ECDH_P256, &rng)
        .map_err(|e| format!("ephemeral key: {e}"))?;
    let as_public = eph
        .compute_public_key()
        .map_err(|e| format!("ephemeral public: {e}"))?
        .as_ref()
        .to_vec();

    let shared = agreement::agree_ephemeral(
        eph,
        &UnparsedPublicKey::new(&ECDH_P256, ua_public),
        |secret| secret.to_vec(),
    )
    .map_err(|_| "ECDH compartido inválido".to_string())?;

    let mut key_info = Vec::with_capacity(14 + 65 + 65);
    key_info.extend_from_slice(b"WebPush: info\0");
    key_info.extend_from_slice(ua_public);
    key_info.extend_from_slice(&as_public);
    let ikm = hkdf_sha256(auth_secret, &shared, &key_info, 32)?;

    let mut salt = [0u8; 16];
    rng.fill(&mut salt).map_err(|_| "salt rng".to_string())?;
    let cek = hkdf_sha256(&salt, &ikm, b"Content-Encoding: aes128gcm\0", 16)?;
    let nonce = hkdf_sha256(&salt, &ikm, b"Content-Encoding: nonce\0", 12)?;

    let mut record = Vec::with_capacity(payload.len() + 1);
    record.extend_from_slice(payload);
    record.push(0x02); // delimitador de último registro (RFC 8188)

    let unbound = UnboundKey::new(&AES_128_GCM, &cek).map_err(|e| format!("cek: {e}"))?;
    let key = LessSafeKey::new(unbound);
    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(&nonce);
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce_arr),
        Aad::empty(),
        &mut record,
    )
    .map_err(|e| format!("aead: {e}"))?;

    let mut body = Vec::with_capacity(16 + 4 + 1 + 65 + record.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(as_public.len() as u8);
    body.extend_from_slice(&as_public);
    body.extend_from_slice(&record);
    Ok(body)
}

/// Inicializa el servicio global. Sin DB válida el push queda deshabilitado.
pub async fn init_global_push(db: Surreal<Db>) {
    if std::env::var("PUSH_ENABLED").is_ok_and(|v| v == "false") {
        tracing::info!("push: deshabilitado por PUSH_ENABLED=false");
        return;
    }
    match PushService::load_or_generate(db).await {
        Ok(service) => {
            tracing::info!("push: VAPID inicializado ({}…)", &service.public_key()[..8.min(service.public_key().len())]);
            let _ = GLOBAL_PUSH.set(service);
        }
        Err(e) => tracing::warn!("push: no se pudo inicializar: {e}"),
    }
}

/// Notifica a todos los suscritos que se creó una escalación. Best-effort:
/// se ejecuta en background y nunca propaga errores.
pub fn notify_escalation(
    db: crate::db::Database,
    patient_id: String,
    alert_type: String,
    severity: Severity,
) {
    let Some(service) = push() else {
        return;
    };
    let level = match severity {
        Severity::Critical => "CRÍTICA",
        Severity::High => "ALTA",
        Severity::Medium => "MEDIA",
        Severity::Low => "BAJA",
    };
    // Sin PHI: solo tipo de alerta, severidad y un identificador interno.
    let title = format!("Alerta {level} — UCI");
    let body = format!("{alert_type} · paciente {patient_id}");
    tokio::spawn(async move {
        let (sent, failed) = service.send_to_all(&db, &title, &body).await;
        tracing::info!("push: escalación notificada (enviadas {sent}, fallidas {failed})");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_b64_roundtrip() {
        let data = vec![0u8, 1, 2, 250, 251, 255, 128, 64];
        let encoded = b64url_encode(&data);
        assert!(!encoded.contains('='));
        assert_eq!(b64url_decode(&encoded).unwrap(), data);
    }

    #[test]
    fn hkdf_is_deterministic_and_length_bounded() {
        let a = hkdf_sha256(&[0u8; 16], &[1u8; 32], b"info", 32).unwrap();
        let b = hkdf_sha256(&[0u8; 16], &[1u8; 32], b"info", 32).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    fn test_service() -> PushService {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        PushService {
            public_key: b64url_encode(key.public_key().as_ref()),
            private_pkcs8: pkcs8.as_ref().to_vec(),
            subject: VAPID_DEFAULT_SUBJECT.to_string(),
            client: reqwest::Client::new(),
        }
    }

    #[test]
    fn vapid_jwt_has_es256_header_and_aud_matching_endpoint() {
        let svc = test_service();
        let auth = svc.vapid_authorization("https://fcm.googleapis.com").unwrap();
        assert!(auth.starts_with("vapid t="), "prefijo VAPID: {auth}");

        let (_, rest) = auth.split_once("t=").expect("t=");
        let (payload, _k) = rest.split_once(", k=").expect(", k=");
        let (unsigned, _sig) = payload.rsplit_once('.').expect("JWT de 3 segmentos");
        let (header_b64, claims_b64) = unsigned.split_once('.').expect("dos segmentos");
        let header: serde_json::Value =
            serde_json::from_slice(&b64url_decode(header_b64).unwrap()).unwrap();
        assert_eq!(header["typ"], "JWT");
        assert_eq!(header["alg"], "ES256");
        let claims: serde_json::Value =
            serde_json::from_slice(&b64url_decode(claims_b64).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        assert_eq!(claims["sub"], VAPID_DEFAULT_SUBJECT);
        assert!(claims["exp"].as_i64().unwrap() > chrono::Utc::now().timestamp());
    }

    #[test]
    fn aes128gcm_body_has_expected_header_len_and_roundtrip_fields() {
        let ua_public = {
            // Clave pública efímera del navegador: un punto P-256 de 65 bytes.
            let rng = SystemRandom::new();
            let eph = agreement::EphemeralPrivateKey::generate(&ECDH_P256, &rng).unwrap();
            eph.compute_public_key().unwrap().as_ref().to_vec()
        };
        let auth_secret = [3u8; 16];

        let payload = br#"{"title":"Alerta ALTA","body":"test"}"#;
        let body = encrypt_aes128gcm(payload, &ua_public, &auth_secret).unwrap();

        // RFC 8291: 16 bytes de salt + 4 bytes de rs (4096) + 1 byte de longitud
        // de clave pública + 65 bytes de clav pública + ciphertext+tag.
        assert_eq!(body.len(), 16 + 4 + 1 + 65 + payload.len() + 1 + 16);
        assert_eq!(&body[16..20], &4096u32.to_be_bytes(), "rs = 4096");
        assert_eq!(body[20], 65, "longitud de la clave pública");

        // El cuerpo comienza con el salt (16 bytes aleatorios).
        let salt = &body[..16];
        assert_ne!(salt, &[0u8; 16], "salt no nulo");
    }
}
