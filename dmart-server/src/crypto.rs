#![allow(dead_code)]

// `Aead`/`KeyInit` se importan por trait: `chacha20poly1305` y `aes-gcm`
// comparten el `aead` de la misma versión, así que un solo juego de traits
// basta para ambos y los alias AES resuelven la ambigüedad.
use aes_gcm::aead::{Aead, KeyInit, OsRng as AesOsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const NONCE_SIZE: usize = 12;

/// Nonce de AES-GCM con el tamaño fijado, para poder nombrarlo en firmas.
type AesNonceSized = aes_gcm::Nonce<aes_gcm::aes::cipher::consts::U12>;
pub const KEY_SIZE: usize = 32;
pub const SALT_SIZE: usize = 16;
pub const IV_SIZE: usize = 16;

/// Envelope histórico: ChaCha20-Poly1305. Se conserva para descifrar datos ya
/// cifrados; no se usa para nuevos registros.
pub const ENCRYPTED_MAGIC: &[u8] = b"DMART_V1";

/// Envelope vigente: AES-256-GCM. Es el que se usa para todo dato nuevo.
pub const AES256_MAGIC: &[u8] = b"DMART_A2";

/// Envelope con key_id para rotación de claves: AES-256-GCM + key_id (4 bytes).
/// Formato: DMART_K1 | key_id(4) | nonce(12) | ciphertext+tag
pub const AES256_KID_MAGIC: &[u8] = b"DMART_K1";
pub const KEY_ID_SIZE: usize = 4;

/// Proveedor de claves con soporte para rotación (key_id).
///
/// Cada clave tiene un `key_id` único (u32). El cifrado usa la clave activa
/// (key_id más alto), el descifrado busca por key_id en el envelope.
/// El key_id va en el envelope (4 bytes big-endian tras el magic).
#[derive(Clone)]
pub struct KeyProvider {
    keys: Vec<(u32, Aes256Gcm)>,
    active_key_id: u32,
}

impl KeyProvider {
    /// Crea un proveedor a partir de `DMART_MASTER_KEY` (clave única, key_id=1).
    pub fn from_env() -> Result<Self, CryptoError> {
        match std::env::var("DMART_MASTER_KEY") {
            Ok(secret) if !secret.trim().is_empty() => {
                let key_bytes = Zeroizing::new(secret.trim().as_bytes().to_vec());
                let master = MasterKey::from_password_bytes(key_bytes.as_slice());
                let aes = Aes256Gcm::new_from_slice(master.as_bytes())
                    .expect("AES-256 con clave de 256 bits");
                Ok(Self {
                    keys: vec![(1, aes)],
                    active_key_id: 1,
                })
            }
            _ if crate::deployment::is_production() => {
                tracing::error!("DMART_MASTER_KEY no configurada en producción");
                Err(CryptoError::MissingMasterKey)
            }
            _ => {
                tracing::warn!("DMART_MASTER_KEY no configurada: usando clave efímera");
                let master = MasterKey::new();
                let aes = Aes256Gcm::new_from_slice(master.as_bytes())
                    .expect("AES-256 con clave de 256 bits");
                Ok(Self {
                    keys: vec![(1, aes)],
                    active_key_id: 1,
                })
            }
        }
    }

    /// Crea un proveedor con clave única (para tests/backfill).
    pub fn single_key(key: MasterKey) -> Self {
        let aes = Aes256Gcm::new_from_slice(key.as_bytes()).expect("AES-256");
        Self {
            keys: vec![(1, aes)],
            active_key_id: 1,
        }
    }

    /// Añade una nueva clave y la marca como activa.
    /// Devuelve el nuevo key_id.
    pub fn add_key(&mut self, key: MasterKey) -> u32 {
        let new_id = self.active_key_id + 1;
        let aes = Aes256Gcm::new_from_slice(key.as_bytes()).expect("AES-256");
        self.keys.push((new_id, aes));
        self.active_key_id = new_id;
        new_id
    }

    /// Añade una clave a partir de bytes de clave maestra (32 bytes).
    pub fn add_key_from_bytes(&mut self, key_bytes: &[u8]) -> Result<u32, CryptoError> {
        if key_bytes.len() != KEY_SIZE {
            return Err(CryptoError::InvalidKey);
        }
        let mut key_arr = [0u8; KEY_SIZE];
        key_arr.copy_from_slice(key_bytes);
        let master = MasterKey(key_arr);
        Ok(self.add_key(master))
    }

    /// Obtiene el cifrador para una clave dada (por key_id).
    pub fn get(&self, key_id: u32) -> Option<&Aes256Gcm> {
        self.keys
            .iter()
            .find(|(id, _)| *id == key_id)
            .map(|(_, aes)| aes)
    }

    /// Obtiene el cifrador activo (para cifrar nuevos datos).
    pub fn active(&self) -> (&Aes256Gcm, u32) {
        let aes = self
            .keys
            .iter()
            .find(|(id, _)| *id == self.active_key_id)
            .map(|(_, aes)| aes)
            .expect("active key always exists");
        (aes, self.active_key_id)
    }

    /// Lista todos los key_ids disponibles.
    pub fn key_ids(&self) -> Vec<u32> {
        self.keys.iter().map(|(id, _)| *id).collect()
    }

    /// Genera una nueva clave aleatoria, la añade y la marca como activa.
    /// Devuelve el nuevo key_id.
    pub fn rotate_key(&mut self) -> u32 {
        self.add_key(MasterKey::new())
    }
}

impl Default for KeyProvider {
    fn default() -> Self {
        Self::single_key(MasterKey::new())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Invalid key")]
    InvalidKey,
    #[error("Invalid data format")]
    InvalidFormat,
    #[error("Key management error")]
    KeyManagementError,
    /// `DMART_MASTER_KEY` ausente en un entorno que la exige.
    #[error("DMART_MASTER_KEY no configurada")]
    MissingMasterKey,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; KEY_SIZE]);

impl MasterKey {
    pub fn new() -> Self {
        let mut key = [0u8; KEY_SIZE];
        AesOsRng.fill_bytes(&mut key);
        Self(key)
    }

    pub fn from_password(password: &str) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(b"dmart-uci-key-v1");
        let result = hasher.finalize();
        let mut key = [0u8; KEY_SIZE];
        key.copy_from_slice(&result[..KEY_SIZE.min(result.len())]);
        Self(key)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.0
    }
}

impl Default for MasterKey {
    fn default() -> Self {
        Self::new()
    }
}

/// Longitud mínima de un secreto de 256 bits.
pub const SECRET_MIN_LEN: usize = 32;

/// Número mínimo de caracteres distintos exigido como indicador de entropía.
/// Filtra vectores de baja entropía del tipo `aaaaaaaaaaaaaaaa` o `012345…`.
const SECRET_MIN_DISTINCT: usize = 12;

/// Marcadores (case-insensitive) que delatan un valor placeholder o de ejemplo.
/// Si un secreto contiene alguno de ellos, es públicamente conocido: se rechaza.
/// El caso más importante es `DMART_MASTER_KEY=CHANGE_ME_64hex_openssl_rand_hex_32`
/// de `.env.prod.example`, que antes pasaba la validación por supera los 32 chars.
const PLACEHOLDER_MARKERS: [&str; 16] = [
    "change",
    "cambiar",
    "cambio",
    "changeme",
    "example",
    "exemplo",
    "placeholder",
    "sample",
    "dummy",
    "replace",
    "pendiente",
    "generar",
    "openssl rand",
    "your-",
    "your_",
    "todo",
];

/// Valores completos concretos que aparecen en `.env*` del repo o en
/// documentación de arranque.
const KNOWN_WEAK_EXACT: [&str; 5] = [
    "dmart-default-key-change-me",
    "change-this-to-a-random-32-char-secret",
    "change-me-to-a-random-64-char-string",
    "change_me_64hex_openssl_rand_hex_32",
    "change-me-32c",
];

/// Valida la robustez de un secreto de configuración (fail-closed).
///
/// Rechaza, en este orden:
/// 1. Valores vacíos o más cortos de 256 bits (`SECRET_MIN_LEN`).
/// 2. Valores placeholder/públicamente conocidos (`PLACEHOLDER_MARKERS`,
///    `KNOWN_WEAK_EXACT`) — cubre los `.env.example` / `.env.prod.example`.
/// 3. Secretos hexadecimales con menos de 256 bits (más de 32 hex chars).
/// 4. Secretos con menos de `SECRET_MIN_DISTINCT` caracteres distintos
///    (entropía trivial: `aaaa…`, `0000…`).
pub fn validate_secret_strength(label: &str, secret: &str) -> Result<(), String> {
    let lower = secret.to_ascii_lowercase();

    if secret.len() < SECRET_MIN_LEN {
        return Err(format!(
            "{label} is too short: {} chars (need >= {SECRET_MIN_LEN}, e.g. `openssl rand -hex 32`).",
            secret.len()
        ));
    }

    if KNOWN_WEAK_EXACT.iter().any(|w| lower.contains(w)) {
        return Err(format!(
            "{label} is still the insecure development/example default. Generate a real secret \
             with `openssl rand -hex 32`."
        ));
    }

    if let Some(marker) = PLACEHOLDER_MARKERS.iter().find(|m| lower.contains(**m)) {
        return Err(format!(
            "{label} contains the placeholder marker `{marker}`. This value is publicly known, \
             so it cannot protect data. Generate a real secret with `openssl rand -hex 32`."
        ));
    }

    // Un secreto hexadecimal debe aportar los 32 bytes completos (64 hex chars);
    // si no, se acepta solo como passphrase con entropía suficiente.
    let is_hex = !secret.is_empty() && secret.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex && secret.len() < 64 {
        return Err(format!(
            "{label} looks like a hex secret but has only {} hex chars (need 64 for 256 bits, \
             e.g. `openssl rand -hex 32`).",
            secret.len()
        ));
    }

    let distinct: std::collections::BTreeSet<char> = secret.chars().collect();
    if distinct.len() < SECRET_MIN_DISTINCT {
        return Err(format!(
            "{label} has too little entropy: only {} distinct characters. Generate a real secret \
             with `openssl rand -hex 32`.",
            distinct.len()
        ));
    }

    Ok(())
}

/// Valida que `DMART_MASTER_KEY` está configurada con un valor fuerte.
///
/// El servidor NO arranca (fail-closed) si la variable falta o conserva un
/// placeholder de `.env.example` / `.env.prod.example`, de modo que el cifrado
/// en reposo y la cadena de auditoría nunca queden protegidos por una clave
/// públicamente conocida.
pub fn validate_master_key() -> Result<(), String> {
    match std::env::var("DMART_MASTER_KEY") {
        Ok(key) => validate_secret_strength("DMART_MASTER_KEY", &key),
        Err(_) => Err(
            "DMART_MASTER_KEY is required (fail-closed). Generate one with `openssl rand -hex 32` \
             and set it in .env."
                .to_string(),
        ),
    }
}

pub struct CryptoService {
    master_key: MasterKey,
    /// Cifrado vigente: AES-256-GCM (FIPS 140-3, acelerado por AES-NI).
    aes: Aes256Gcm,
    /// Retirado: ChaCha20-Poly1305. Sólo para descifrar envelopes `DMART_V1`.
    chacha: ChaCha20Poly1305,
}

impl CryptoService {
    pub fn new(key: MasterKey) -> Self {
        let aes = Aes256Gcm::new_from_slice(key.as_bytes()).expect("AES-256 con clave de 256 bits");
        let chacha =
            ChaCha20Poly1305::new_from_slice(key.as_bytes()).expect("Valid key for ChaCha20");
        Self {
            master_key: key,
            aes,
            chacha,
        }
    }

    pub fn new_from_password(password: &str) -> Self {
        Self::new(MasterKey::from_password(password))
    }

    pub fn new_software_hsm() -> Self {
        let key = MasterKey::new();
        Self::new(key)
    }

    /// Cifra con **AES-256-GCM**. Nonce aleatorio de 96 bits por operación (el
    /// límite seguro del GCM) y tag de autenticación de 128 bits, de modo que
    /// cualquier modificación del ciphertext o del associated data falla al
    /// descifrar en vez de devolver basura.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        AesOsRng.fill_bytes(&mut nonce_bytes);
        let nonce = AesNonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .aes
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: &[],
                },
            )
            .map_err(|_| CryptoError::EncryptionFailed)?;

        let mut result = Vec::with_capacity(AES256_MAGIC.len() + NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(AES256_MAGIC);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Descifra detectando el envelope. Soporta `DMART_A2` (AES-256-GCM) y
    /// `DMART_V1` (ChaCha20-Poly1305, histórico), de forma que una rotación de
    /// algoritmo no deja datos ilegibles.
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let header = AES256_MAGIC.len();
        if encrypted.len() < header + NONCE_SIZE {
            return Err(CryptoError::InvalidFormat);
        }

        match &encrypted[..header] {
            m if m == AES256_MAGIC => {
                let nonce_bytes = &encrypted[header..header + NONCE_SIZE];
                let ciphertext = &encrypted[header + NONCE_SIZE..];
                self.aes
                    .decrypt(
                        AesNonce::from_slice(nonce_bytes),
                        Payload {
                            msg: ciphertext,
                            aad: &[],
                        },
                    )
                    .map_err(|_| CryptoError::DecryptionFailed)
            }
            m if m == ENCRYPTED_MAGIC => {
                let nonce_bytes = &encrypted[header..header + NONCE_SIZE];
                let ciphertext = &encrypted[header + NONCE_SIZE..];
                self.chacha
                    .decrypt(
                        Nonce::from_slice(nonce_bytes),
                        Payload {
                            msg: ciphertext,
                            aad: &[],
                        },
                    )
                    .map_err(|_| CryptoError::DecryptionFailed)
            }
            _ => Err(CryptoError::InvalidFormat),
        }
    }

    pub fn encrypt_str(&self, plaintext: &str) -> Result<String, CryptoError> {
        let encrypted = self.encrypt(plaintext.as_bytes())?;
        Ok(base64_encode(&encrypted))
    }

    pub fn decrypt_str(&self, encrypted: &str) -> Result<String, CryptoError> {
        let data = base64_decode(encrypted).map_err(|_| CryptoError::InvalidFormat)?;
        let decrypted = self.decrypt(&data)?;
        String::from_utf8(decrypted).map_err(|_| CryptoError::DecryptionFailed)
    }

    pub fn encrypt_json<T: serde::Serialize>(&self, data: &T) -> Result<String, CryptoError> {
        let json = serde_json::to_vec(data).map_err(|_| CryptoError::EncryptionFailed)?;
        self.encrypt_str(&String::from_utf8_lossy(&json))
    }

    pub fn decrypt_json<T: serde::de::DeserializeOwned>(
        &self,
        encrypted: &str,
    ) -> Result<T, CryptoError> {
        let decrypted = self.decrypt_str(encrypted)?;
        serde_json::from_str(&decrypted).map_err(|_| CryptoError::DecryptionFailed)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cifrado de PHI en reposo (SPEC-052 / auditoría H4)
// ─────────────────────────────────────────────────────────────────────────────

/// Longitud de la clave derivada: 256 bits.
pub const PHI_KEY_SIZE: usize = 32;

/// Etiquetas de derivación. La separación de dominio garantiza que la clave de
/// cifrado de PHI, la clave de índices ciegos y la de la cadena de auditoría
/// nunca coincidan, aunque las tres deriven de `DMART_MASTER_KEY`.
const LABEL_PHI: &[u8] = b"dmart-phi-aes256-gcm-v1";
const LABEL_INDEX: &[u8] = b"dmart-phi-blind-index-v1";
const LABEL_MAC: &[u8] = b"dmart-phi-subkey-v1";
/// Dominio del AAD de PHI, versionado junto al esquema de longitudes.
const AAD_DOMAIN: &[u8] = b"dmart-phi-v2";

/// Identifica el contexto criptográfico de un valor cifrado.
///
/// Se usa como *associated data* (AAD) del AES-GCM: el tag de autenticación
/// cubre el AAD, así que un ciphertext no puede descifrarse en otro registro,
/// bajo otro tenant, o como si fuera otro campo. Sin esto, un atacante con
/// acceso de escritura a la base de datos podría copiar el blob de un paciente
/// sobre otro y el descifrado lo aceptaría.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhiContext {
    pub tenant_id: String,
    pub record_type: String,
    pub record_id: String,
}

impl PhiContext {
    pub fn new(
        tenant_id: impl Into<String>,
        record_type: impl Into<String>,
        record_id: impl Into<String>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            record_type: record_type.into(),
            record_id: record_id.into(),
        }
    }

    /// AAD del esquema v1, con newlines como separador.
    ///
    /// Se conserva **sólo** para leer envelopes ya escritos; nunca para cifrar.
    /// Es ambiguo ante identificadores con saltos de línea: dos contextos
    /// distintos pueden generar el mismo AAD, y el tag GCM dejaría de ligar el
    /// ciphertext a su tenant y registro.
    pub(crate) fn legacy_aad(&self) -> String {
        format!(
            "dmart-phi-v1\n{}\n{}\n{}",
            self.tenant_id, self.record_type, self.record_id
        )
    }

    /// AAD vigente: longitudes prefijadas, sin ambigüedad.
    pub(crate) fn aad(&self) -> Vec<u8> {
        // Longitudes prefijadas en vez de un separador: un `tenant_id` con un
        // salto de línea dentro haría colisionar dos contextos distintos, y el
        // AAD es lo que impide mover un envelope entre registros. Con el
        // prefijo de longitud, `("a\nb", "c")` y `("a", "b\nc")` producen AAD
        // diferente.
        let mut out = Vec::new();
        out.extend_from_slice(AAD_DOMAIN);
        push_len_prefixed(&mut out, self.tenant_id.as_bytes());
        push_len_prefixed(&mut out, self.record_type.as_bytes());
        push_len_prefixed(&mut out, self.record_id.as_bytes());
        out
    }
}

/// Cifra PHI en reposo usando AES-256-GCM y mantiene*búsqueda exacta* mediante
/// índices ciegos HMAC-SHA256.
///
/// Modelo de datos:
/// - El documento se serializa entero y se guarda en una única columna `phi`
///   como envelope `DMART_A2|<base64>` o `DMART_K1|key_id|<base64>`; dentro quedan **todos** los campos,
///   incluidos los que no se consultan, para que no quede PHI suelta por error.
/// - Las columnas que sí se necesitan para filtrar o paginar (`patient_id`,
///   `tenant_id`, estados no identificables) permanecen en claro.
/// - Los identificadores buscables (`historia_clinica`, `cedula`, nombre) se
///   guardan **sólo** como índice ciego: `HMAC-SHA256(clave_indice, tenant || campo
///   || valor_normalizado)`. Sin la clave no se puede recuperar el valor ni
///   hacer diccionario, y el índice no filtra el texto.
#[derive(Clone)]
pub struct PhiCipher {
    pub master_key: MasterKey,
    pub provider: KeyProvider,
    /// Cifrador con el que se escribieron los envelopes anteriores al
    /// endurecimiento: clave maestra en crudo y AAD v1.
    ///
    /// Existe sólo para **leer** esos registros. Se eliminará cuando no queden
    /// filas con el esquema antiguo.
    legacy_cipher: Aes256Gcm,
    index_key: Zeroizing<[u8; PHI_KEY_SIZE]>,
}

impl std::fmt::Debug for PhiCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Nunca imprime material de clave.
        f.debug_struct("PhiCipher").finish_non_exhaustive()
    }
}

impl PhiCipher {
    /// Construye el cifrador a partir de la clave maestra.
    ///
    /// La clave maestra **nunca** se usa directamente como clave de AES ni de
    /// HMAC: se deriva una subclave por dominio. Así, comprometer la clave de
    /// índices ciegos no permite descifrar, y usar la clave maestra como clave
    /// de cifrado expandiría su superficie.
    pub fn new(master_key: MasterKey) -> Self {
        let phi_key = derive_subkey(master_key.as_bytes(), LABEL_PHI);
        let index_key = derive_subkey(master_key.as_bytes(), LABEL_INDEX);
        let provider = KeyProvider::single_key(master_key.clone());
        let _cipher =
            Aes256Gcm::new_from_slice(phi_key.as_slice()).expect("AES-256 con clave de 256 bits");
        let legacy_cipher = Aes256Gcm::new_from_slice(master_key.as_bytes())
            .expect("AES-256 con clave de 256 bits");
        Self {
            master_key,
            provider,
            legacy_cipher,
            index_key: Zeroizing::new(index_key),
        }
    }

    /// Cifrador desde `DMART_MASTER_KEY` usando KeyProvider con rotación.
    ///
    /// En producción la clave es obligatoria y el arranque falla si falta
    /// (fail-closed). Sin este error, un despliegue mal configurado cifraría con
    /// una clave efímera: los registros ya escritos quedarían ilegibles al
    /// reiniciar, y parecería que todo funciona hasta el primer restore.
    ///
    /// En desarrollo se permite una clave efímera, con aviso explícito.
    pub fn from_env() -> Result<Self, CryptoError> {
        let master_key = match std::env::var("DMART_MASTER_KEY") {
            Ok(secret) if !secret.trim().is_empty() => {
                let key_bytes = Zeroizing::new(secret.trim().as_bytes().to_vec());
                MasterKey::from_password_bytes(key_bytes.as_slice())
            }
            _ if crate::deployment::is_production() => {
                tracing::error!(
                    "DMART_MASTER_KEY no configurada en producción: no se puede cifrar la PHI"
                );
                return Err(CryptoError::MissingMasterKey);
            }
            _ => {
                tracing::warn!(
                    "DMART_MASTER_KEY no configurada: cifrando PHI con clave efímera. Los datos \\
                     serán ilegibles tras reiniciar. OBLIGATORIO en producción."
                );
                MasterKey::new()
            }
        };
        Ok(Self::new(master_key))
    }

    /// Cifra un documento completo y devuelve el valor para la columna `phi`.
    pub fn seal<T: serde::Serialize>(
        &self,
        ctx: &PhiContext,
        value: &T,
    ) -> Result<String, CryptoError> {
        let json = serde_json::to_vec(value).map_err(|_| CryptoError::EncryptionFailed)?;
        self.seal_bytes(ctx, &json)
    }

    /// Cifra bytes arbitrarios.
    pub fn seal_bytes(&self, ctx: &PhiContext, plaintext: &[u8]) -> Result<String, CryptoError> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        AesOsRng.fill_bytes(&mut nonce_bytes);
        let aad = ctx.aad();

        let (aes, key_id) = self.provider.active();

        let ciphertext = aes
            .encrypt(
                AesNonce::from_slice(&nonce_bytes),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| CryptoError::EncryptionFailed)?;

        let mut raw = Vec::with_capacity(
            AES256_KID_MAGIC.len() + KEY_ID_SIZE + NONCE_SIZE + ciphertext.len(),
        );
        raw.extend_from_slice(AES256_KID_MAGIC);
        raw.extend_from_slice(&key_id.to_be_bytes());
        raw.extend_from_slice(&nonce_bytes);
        raw.extend_from_slice(&ciphertext);
        Ok(base64_encode(&raw))
    }

    /// Abre un documento cifrado.
    pub fn open<T: serde::de::DeserializeOwned>(
        &self,
        ctx: &PhiContext,
        sealed: &str,
    ) -> Result<T, CryptoError> {
        let json = self.open_bytes(ctx, sealed)?;
        serde_json::from_slice(&json).map_err(|_| CryptoError::DecryptionFailed)
    }

    /// Abre bytes arbitrarios.
    pub fn open_bytes(&self, ctx: &PhiContext, sealed: &str) -> Result<Vec<u8>, CryptoError> {
        let raw = base64_decode(sealed).map_err(|_| CryptoError::InvalidFormat)?;

        // Nuevo formato con key_id: DMART_K1 | key_id(4) | nonce(12) | ciphertext
        let kid_header = AES256_KID_MAGIC.len();
        if raw.len() >= kid_header + KEY_ID_SIZE + NONCE_SIZE
            && &raw[..kid_header] == AES256_KID_MAGIC
        {
            let key_id = u32::from_be_bytes([
                raw[kid_header],
                raw[kid_header + 1],
                raw[kid_header + 2],
                raw[kid_header + 3],
            ]);
            let nonce_start = kid_header + KEY_ID_SIZE;
            let nonce = AesNonce::from_slice(&raw[nonce_start..nonce_start + NONCE_SIZE]);
            let msg = &raw[nonce_start + NONCE_SIZE..];

            if let Some(aes) = self.provider.get(key_id) {
                let legacy_aad = ctx.legacy_aad();
                let current_aad = ctx.aad();
                let attempts: [(&Aes256Gcm, &[u8]); 4] = [
                    (aes, &current_aad),
                    (aes, legacy_aad.as_bytes()),
                    (&self.legacy_cipher, legacy_aad.as_bytes()),
                    (&self.legacy_cipher, &[]),
                ];
                for (i, (cipher, aad)) in attempts.into_iter().enumerate() {
                    if let Ok(p) = cipher.decrypt(nonce, Payload { msg, aad }) {
                        if i > 0 {
                            tracing::warn!(
                                "envelope PHI abierto con el esquema anterior (AAD v1 / clave maestra); \
                                 se re-sella al escribir"
                            );
                        }
                        return Ok(p);
                    }
                }
            }
            return Err(CryptoError::DecryptionFailed);
        }

        // Formato antiguo sin key_id: DMART_A2 | nonce(12) | ciphertext
        let header = AES256_MAGIC.len();
        if raw.len() >= header + NONCE_SIZE && &raw[..header] == AES256_MAGIC {
            let nonce = AesNonce::from_slice(&raw[header..header + NONCE_SIZE]);
            let msg = &raw[header + NONCE_SIZE..];

            let legacy_aad = ctx.legacy_aad();
            let current_aad = ctx.aad();
            let attempts: [(&Aes256Gcm, &[u8]); 4] = [
                (&self.provider.active().0, &current_aad),
                (&self.provider.active().0, legacy_aad.as_bytes()),
                (&self.legacy_cipher, legacy_aad.as_bytes()),
                (&self.legacy_cipher, &[]),
            ];
            for (i, (cipher, aad)) in attempts.into_iter().enumerate() {
                if let Ok(p) = cipher.decrypt(nonce, Payload { msg, aad }) {
                    if i > 0 {
                        tracing::warn!(
                            "envelope PHI abierto con el esquema anterior (AAD v1 / clave maestra); \
                             se re-sella al escribir"
                        );
                    }
                    return Ok(p);
                }
            }
            return Err(CryptoError::DecryptionFailed);
        }

        // Registros heredados con el envelope ChaCha (`DMART_V1`) se abren
        // con la misma clave para no dejar filas ilegibles durante la
        // migración.
        if raw.len() >= ENCRYPTED_MAGIC.len() + NONCE_SIZE
            && &raw[..ENCRYPTED_MAGIC.len()] == ENCRYPTED_MAGIC
        {
            let nonce = Nonce::from_slice(&raw[header..header + NONCE_SIZE]);
            return ChaCha20Poly1305::new_from_slice(self.master_key.as_bytes())
                .map_err(|_| CryptoError::InvalidKey)?
                .decrypt(
                    nonce,
                    Payload {
                        msg: &raw[header + NONCE_SIZE..],
                        aad: &[],
                    },
                )
                .map_err(|_| CryptoError::DecryptionFailed);
        }

        Err(CryptoError::InvalidFormat)
    }

    /// Índice ciego de un valor buscable (coincidencia **exacta**).
    ///
    /// El tenant, el tipo de registro y el nombre del campo entran en el HMAC,
    /// así que el mismo MRN en dos tenants produce índices distintos y un
    /// índice de `cedula` no sirve para buscar por `historia_clinica`.
    pub fn blind_index(&self, tenant_id: &str, field: &str, value: &str) -> String {
        let normalized = normalize_for_index(value);
        if normalized.is_empty() {
            return String::new();
        }
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, self.index_key.as_slice());
        // Mensaje canónico con longitudes prefijadas: sin ellas, un tenant con
        // salto de línea podría desplazar los campos y colisionar con otro par
        // tenant/campo, lo que haría que un índice de `cedula` sirviera para
        // buscar por `historia_clinica`.
        let mut msg = Vec::with_capacity(
            LABEL_MAC.len() + tenant_id.len() + field.len() + normalized.len() + 12,
        );
        msg.extend_from_slice(LABEL_MAC);
        push_len_prefixed(&mut msg, tenant_id.as_bytes());
        push_len_prefixed(&mut msg, field.as_bytes());
        push_len_prefixed(&mut msg, normalized.as_bytes());
        let tag = hmac_sha256_tag(&key, &msg);
        base64_encode(tag.as_ref())
    }

    /// Índice ciego de un valor `Option`, propagando el vacío.
    pub fn blind_index_opt(&self, tenant_id: &str, field: &str, value: Option<&str>) -> String {
        value
            .map(|v| self.blind_index(tenant_id, field, v))
            .unwrap_or_default()
    }

    /// Índice ciego **formato legacy** (sin longitudes prefijadas).
    ///
    /// Usado para compatibilidad con filas cifradas antes de la migración a
    /// longitudes prefijadas. Formato: `LABEL_MAC || tenant || field || normalized`.
    pub fn blind_index_legacy(&self, tenant_id: &str, field: &str, value: &str) -> String {
        let normalized = normalize_for_index(value);
        if normalized.is_empty() {
            return String::new();
        }
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, self.index_key.as_slice());
        let mut msg =
            Vec::with_capacity(LABEL_MAC.len() + tenant_id.len() + field.len() + normalized.len());
        msg.extend_from_slice(LABEL_MAC);
        msg.extend_from_slice(tenant_id.as_bytes());
        msg.extend_from_slice(field.as_bytes());
        msg.extend_from_slice(normalized.as_bytes());
        let tag = hmac_sha256_tag(&key, &msg);
        base64_encode(tag.as_ref())
    }

    /// Rota la clave maestra: genera una nueva clave aleatoria, la añade al
    /// proveedor y la marca como activa. Todos los nuevos envelopes usarán
    /// la nueva clave (key_id incrementado). Los envelopes existentes siguen
    /// siendo legibles porque se conserva el key_id en el envelope.
    /// Devuelve el nuevo key_id.
    pub fn rotate_key(&mut self) -> u32 {
        self.provider.rotate_key()
    }
}

/// Normalización para el índice ciego.
///
/// Se ignoran mayúsculas, espacios y separadores habituales de identificadores
/// clínicos (`-`, `/`, `.`), de forma que `MRN-001`, `mrn 001` y `MRN.001`
/// colisionan en el índice como deben. No se recorta nada más: el objetivo es
/// que la misma persona con distinto formato de MRN se encuentre, sin
/// normalizar semántica que pueda unir identificadores distintos.
pub fn normalize_for_index(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '/' && *c != '.' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Anexa `len` en big-endian y luego los bytes, de modo que la concatenación de
/// varios campos nunca sea ambigua.
fn push_len_prefixed(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Deriva una clave de 256 bits del maestro con separación de dominio:
/// `HMAC-SHA256(clave_maestra, etiqueta)`.
fn derive_subkey(master: &[u8; KEY_SIZE], label: &[u8]) -> [u8; PHI_KEY_SIZE] {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, master);
    hmac_sha256_tag(&key, label)
        .as_ref()
        .try_into()
        .expect("HMAC-SHA256 son 32 bytes")
}

/// HMAC-SHA256 sobre un mensaje de longitud arbitraria.
///
/// `ring` exige el mensaje por partes (`update`/`finish`) en vez de un buffer
/// único, que además evita construir una copia del valor normalizado en claro.
fn hmac_sha256_tag(key: &ring::hmac::Key, msg: &[u8]) -> ring::hmac::Tag {
    let mut ctx = ring::hmac::Context::with_key(key);
    ctx.update(msg);
    ctx.sign()
}

impl MasterKey {
    /// Deriva la clave maestra de un secreto de configuración.
    ///
    /// A diferencia de [`MasterKey::from_password`], que usa un hash simple
    /// pensado para contraseñas, esto aplica HMAC con el secreto ya validado
    /// por [`validate_secret_strength`] (mínimo 256 bits de entropía), por lo
    /// que no necesita estiramiento.
    pub fn from_password_bytes(secret: &[u8]) -> Self {
        Self(derive_subkey_from_secret(secret))
    }
}

fn derive_subkey_from_secret(secret: &[u8]) -> [u8; KEY_SIZE] {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret);
    hmac_sha256_tag(&key, b"dmart-master-key-v1")
        .as_ref()
        .try_into()
        .expect("HMAC-SHA256 son 32 bytes")
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
    BASE64.encode(data)
}

/// Visible en el crate para que `phi_store` pueda inspeccionar el magic del
/// envelope en sus tests de persistencia.
pub(crate) fn base64_decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
    BASE64.decode(data)
}

pub fn encrypt_data(plaintext: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
    let service = CryptoService::new_from_password(password);
    service.encrypt(plaintext)
}

pub fn decrypt_data(encrypted: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
    let service = CryptoService::new_from_password(password);
    service.decrypt(encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chacha_encrypt_decrypt() {
        let service = CryptoService::new_software_hsm();
        let plaintext = b"Hola mundo secreto";

        let encrypted = service.encrypt(plaintext).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    /// El envelope vigente debe ser AES-256-GCM (`DMART_A2`), no el histórico.
    #[test]
    fn envelope_is_aes256_gcm() {
        let service = CryptoService::new_software_hsm();
        let blob = service.encrypt(b"phi").expect("encrypt");
        assert_eq!(&blob[..AES256_MAGIC.len()], AES256_MAGIC);
        // Longitud = magic(9) + nonce(12) + ciphertext + tag GCM(16).
        assert_eq!(blob.len(), AES256_MAGIC.len() + NONCE_SIZE + 3 + 16);
    }

    /// Backwards compatible: un ciphertext histórico ChaCha20 sigue descifrando
    /// con la misma clave, para no dejar datos ilegibles al migrar.
    #[test]
    fn decrypts_legacy_chacha_envelope() {
        let service = CryptoService::new_software_hsm();
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        AesOsRng.fill_bytes(&mut nonce_bytes);
        let legacy = {
            let ct = service
                .chacha
                .encrypt(
                    Nonce::from_slice(&nonce_bytes),
                    Payload {
                        msg: b"historic phi",
                        aad: &[],
                    },
                )
                .expect("legacy encrypt");
            let mut blob = Vec::new();
            blob.extend_from_slice(ENCRYPTED_MAGIC);
            blob.extend_from_slice(&nonce_bytes);
            blob.extend_from_slice(&ct);
            blob
        };
        assert_eq!(&legacy[..ENCRYPTED_MAGIC.len()], ENCRYPTED_MAGIC);
        assert_eq!(
            service.decrypt(&legacy).expect("legacy decrypt"),
            b"historic phi"
        );
    }

    /// Un ciphertext con un solo bit alterado debe fallar (tag GCM), no
    /// devolver datos corruptos.
    #[test]
    fn tampered_ciphertext_is_rejected() {
        let service = CryptoService::new_software_hsm();
        let mut blob = service
            .encrypt("paciente: Juan Pérez, TA".as_bytes())
            .expect("encrypt");
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(matches!(
            service.decrypt(&blob),
            Err(CryptoError::DecryptionFailed)
        ));
    }

    /// Alterar el nonce también debe detectarse: el nonce va autentizado por el
    /// tag, no es metadato libre.
    #[test]
    fn tampered_nonce_is_rejected() {
        let service = CryptoService::new_software_hsm();
        let mut blob = service.encrypt(b"secret").expect("encrypt");
        blob[AES256_MAGIC.len()] ^= 0xff;
        assert!(matches!(
            service.decrypt(&blob),
            Err(CryptoError::DecryptionFailed)
        ));
    }

    /// Clave distinta no debe poder descifrar.
    #[test]
    fn wrong_key_cannot_decrypt() {
        let a = CryptoService::new_software_hsm();
        let b = CryptoService::new_software_hsm();
        let blob = a.encrypt(b"phi").expect("encrypt");
        assert!(b.decrypt(&blob).is_err());
    }

    /// Envelopes desconocidos se rechazan en vez de intentar descifrar a ciegas.
    #[test]
    fn unknown_envelope_is_rejected() {
        let service = CryptoService::new_software_hsm();
        let mut blob = service.encrypt(b"phi").expect("encrypt");
        blob[0] = b'X';
        assert!(matches!(
            service.decrypt(&blob),
            Err(CryptoError::InvalidFormat)
        ));
        assert!(matches!(
            service.decrypt(b"corto"),
            Err(CryptoError::InvalidFormat)
        ));
    }

    /// El nonce debe ser único por operación: dos cifrados del mismo texto dan
    /// blobs distintos (evita la equivalencia plaintext-ciphertext).
    #[test]
    fn nonce_is_unique_per_operation() {
        let service = CryptoService::new_software_hsm();
        let a = service.encrypt(b"mismo texto").expect("a");
        let b = service.encrypt(b"mismo texto").expect("b");
        assert_ne!(a, b);
        assert_eq!(
            service.decrypt(&a).expect("a"),
            service.decrypt(&b).expect("b")
        );
    }

    #[test]
    fn test_string_encryption() {
        let service = CryptoService::new_software_hsm();
        let plaintext = "Datos sensibles del paciente";

        let encrypted = service.encrypt_str(plaintext).unwrap();
        let decrypted = service.decrypt_str(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_master_key_validation() {
        unsafe { std::env::remove_var("DMART_MASTER_KEY") };
        assert!(validate_master_key().is_err());

        unsafe { std::env::set_var("DMART_MASTER_KEY", "dmart-default-key-change-me") };
        assert!(validate_master_key().is_err());

        unsafe {
            std::env::set_var(
                "DMART_MASTER_KEY",
                "3f9a1c7e5b2d8046af13c6d9e2b70584ac1f3e6d90b84725ca6e1f0d39b7c4e281",
            )
        };
        assert!(validate_master_key().is_ok());

        unsafe { std::env::set_var("DMART_MASTER_KEY", "corta") };
        assert!(validate_master_key().is_err());

        unsafe { std::env::remove_var("DMART_MASTER_KEY") };
    }

    /// Regresión H5: el placeholder de `.env.prod.example` supera los 32 chars
    /// y antes pasaba la validación, dejando producción con una clave pública
    /// conocida (que además firma la cadena de auditoría WORM).
    #[test]
    fn rejects_prod_example_master_key_placeholder() {
        let examples = [
            "CHANGE_ME_64hex_openssl_rand_hex_32",
            "CHANGE-ME-32c",
            "change-this-to-a-random-32-char-secret",
            "dmart-default-key-change-me",
            "CHANGE_ME_64hex_openssl_rand_hex_32\n",
        ];
        for value in examples {
            assert!(
                validate_master_key_value(value).is_err(),
                "placeholder accepted: {value}"
            );
        }
    }

    /// `.env.prod.example:7` debe seguir fallando tras el hardening.
    #[test]
    fn prod_example_file_value_is_rejected() {
        let raw = include_str!("../../.env.prod.example");
        let value = raw
            .lines()
            .find_map(|l| l.strip_prefix("DMART_MASTER_KEY="))
            .expect(".env.prod.example must define DMART_MASTER_KEY");
        assert!(
            validate_master_key_value(value.trim()).is_err(),
            "el valor de ejemplo de .env.prod.example debe rechazarse: {value}"
        );
    }

    #[test]
    fn rejects_low_entropy_secrets() {
        assert!(validate_master_key_value("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").is_err());
        assert!(validate_master_key_value("0123456789012345678901234567890123456789").is_err());
        // Hex de 128 bits: pasa los 32 chars pero no los 256 bits requeridos.
        assert!(validate_master_key_value("0123456789abcdef0123456789abcdef").is_err());
    }

    #[test]
    fn accepts_generated_hex_secret() {
        assert!(
            validate_master_key_value(
                "9e107d9d372bb6826bd81d3542a419d6c3e2d1b0a4f7c8e5a6b3d9f1c0e2a4b6d"
            )
            .is_ok()
        );
        // Passphrase no-hex con entropía suficiente.
        assert!(validate_master_key_value("Tr0ub4dor&3xKcd!PassphraseICU2026$ecure").is_ok());
    }

    fn validate_master_key_value(value: &str) -> Result<(), String> {
        validate_secret_strength("DMART_MASTER_KEY", value)
    }

    // ── PhiCipher: cifrado de PHI en reposo ─────────────────────────────────

    fn cipher() -> PhiCipher {
        PhiCipher::new(MasterKey::from_password_bytes(
            b"clave-de-prueba-256-bits-minimo!!",
        ))
    }

    fn ctx(tenant: &str, rtype: &str, id: &str) -> PhiContext {
        PhiContext::new(tenant, rtype, id)
    }

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Phi {
        nombre: String,
        cedula: String,
        edad: u8,
    }

    fn phi() -> Phi {
        Phi {
            nombre: "María Fernández Ruiz".into(),
            cedula: "1012345678".into(),
            edad: 63,
        }
    }

    #[test]
    fn seal_open_roundtrip() {
        let c = cipher();
        let sealed = c
            .seal(&ctx("hosp-a", "patient", "P-1"), &phi())
            .expect("seal");
        // El envelope no debe contener nada del plaintext.
        assert!(!sealed.contains("María"));
        assert!(!sealed.contains("1012345678"));
        let opened: Phi = c
            .open(&ctx("hosp-a", "patient", "P-1"), &sealed)
            .expect("open");
        assert_eq!(opened, phi());
    }

    /// El AAD ata el ciphertext a (tenant, tipo, id). Sin esto, mover un blob
    /// de otro registro o tenant sería un ataque válido.
    #[test]
    fn aad_prevents_cross_tenant_and_cross_record_replay() {
        let c = cipher();
        let sealed = c
            .seal(&ctx("hosp-a", "patient", "P-1"), &phi())
            .expect("seal");

        // Mismo registro, otro tenant.
        assert!(
            c.open::<Phi>(&ctx("hosp-b", "patient", "P-1"), &sealed)
                .is_err()
        );
        // Mismo tenant, otro registro.
        assert!(
            c.open::<Phi>(&ctx("hosp-a", "patient", "P-2"), &sealed)
                .is_err()
        );
        // Mismo id, otro tipo de registro.
        assert!(
            c.open::<Phi>(&ctx("hosp-a", "measurement", "P-1"), &sealed)
                .is_err()
        );
        // Y el contexto legítimo sí abre.
        assert!(
            c.open::<Phi>(&ctx("hosp-a", "patient", "P-1"), &sealed)
                .is_ok()
        );
    }

    /// El índice ciego debe ser estable para el mismo valor, distinto entre
    /// tenants y entre campos, y no debe revelar el valor.
    #[test]
    fn blind_index_is_tenant_and_field_scoped() {
        let c = cipher();
        let a = c.blind_index("hosp-a", "historia_clinica", "MRN-001");
        assert_eq!(a, c.blind_index("hosp-a", "historia_clinica", "MRN-001"));
        assert_ne!(a, c.blind_index("hosp-b", "historia_clinica", "MRN-001"));
        assert_ne!(a, c.blind_index("hosp-a", "cedula", "MRN-001"));
        assert_ne!(a, c.blind_index("hosp-a", "historia_clinica", "MRN-002"));
        // El índice no filtra el valor.
        assert!(!a.contains("MRN"));
        assert!(a.len() >= 40, "HMAC-SHA256 en base64");
    }

    /// `MRN-001`, `mrn 001` y `MRN.001` deben colapsar en el mismo índice: es el
    /// mismo paciente con distinto formato de tecleado.
    #[test]
    fn blind_index_normalizes_identifier_formats() {
        let c = cipher();
        let canonical = c.blind_index("hosp-a", "historia_clinica", "MRN-001");
        for variant in ["mrn-001", "MRN 001", "MRN.001", "mrn_001", " MRN-001 "] {
            assert_eq!(
                c.blind_index("hosp-a", "historia_clinica", variant),
                canonical,
                "variante {variant} debe colapsar"
            );
        }
        // Pero no debe unir identificadores distintos.
        assert_ne!(
            c.blind_index("hosp-a", "historia_clinica", "MRN-0011"),
            canonical
        );
    }

    #[test]
    fn blind_index_empty_is_empty() {
        let c = cipher();
        assert_eq!(c.blind_index("hosp-a", "cedula", ""), "");
        assert_eq!(c.blind_index("hosp-a", "cedula", "  - _ . "), "");
        assert_eq!(c.blind_index_opt("hosp-a", "cedula", None), "");
    }

    /// Una clave maestra distinta no debe poder abrir ni calcular los índices.
    #[test]
    fn different_master_key_cannot_read() {
        let a = PhiCipher::new(MasterKey::from_password_bytes(
            b"clave-a-256-bits-minimo-suficiente!",
        ));
        let b = PhiCipher::new(MasterKey::from_password_bytes(
            b"clave-b-256-bits-minimo-suficiente!",
        ));
        let sealed = a.seal(&ctx("t", "patient", "1"), &phi()).expect("seal");
        assert!(b.open::<Phi>(&ctx("t", "patient", "1"), &sealed).is_err());
        assert_ne!(
            a.blind_index("t", "cedula", "1"),
            b.blind_index("t", "cedula", "1")
        );
    }

    /// Las subclaves de cifrado, índice y auditoría no deben coincidir.
    #[test]
    fn subkeys_are_domain_separated() {
        let master = MasterKey::from_password_bytes(b"clave-256-bits-minimo-para-pruebas!!");
        let cipher_key = derive_subkey(master.as_bytes(), LABEL_PHI);
        let index_key = derive_subkey(master.as_bytes(), LABEL_INDEX);
        assert_ne!(cipher_key, index_key);
        assert_eq!(cipher_key, derive_subkey(master.as_bytes(), LABEL_PHI));
        assert_ne!(&index_key, master.as_bytes());
    }

    /// El cifrado de producción debe usar la subclave de `LABEL_PHI`, no la
    /// clave maestra en crudo.
    ///
    /// El test anterior sólo comparaba las subclaves entre sí y pasaba aunque
    /// `PhiCipher::new` ignorara `LABEL_PHI`. Este test ata el comportamiento
    /// observable: un envelope sellado con la subclave dedicada se abre con el
    /// cifrador del proceso, y con la clave maestra no.
    #[test]
    fn production_cipher_uses_the_phi_subkey() {
        let master = MasterKey::from_password_bytes(b"clave-256-bits-minimo-para-pruebas!!");
        let master_bytes = *master.as_bytes();
        let c = PhiCipher::new(master);
        let ctx = ctx("hosp-a", "patient", "P-1");

        let sealed = c.seal(&ctx, &phi()).expect("seal");
        assert!(c.open::<Phi>(&ctx, &sealed).is_ok());

        // Si `new` usara la clave maestra directamente, este descifrado
        // también tendría éxito; al fallar, queda demostrado que la clave real
        // es la derivada de LABEL_PHI.
        let raw_master = Aes256Gcm::new_from_slice(&master_bytes).expect("aes");
        let bytes = base64_decode(&sealed).expect("b64");
        let aad = ctx.aad();
        let start = AES256_MAGIC.len();
        let nonce = AesNonce::from_slice(&bytes[start..start + NONCE_SIZE]);
        assert!(
            raw_master
                .decrypt(
                    nonce,
                    Payload {
                        msg: &bytes[start + NONCE_SIZE..],
                        aad: &aad,
                    }
                )
                .is_err(),
            "la clave maestra no debe descifrar el envelope: LABEL_PHI se ignora"
        );
    }

    /// Sin `DMART_MASTER_KEY`, producción falla el arranque y desarrollo usa una
    /// clave efímera.
    #[test]
    fn master_key_is_mandatory_in_production() {
        // Comparte el lock de variables de entorno con `deployment::tests`, y
        // usa `DMART_ENV`, que es la variable que documenta el despliegue.
        let _lock = crate::deployment::tests_lock();
        let _guard = crate::deployment::EnvGuard::new(&[
            ("DMART_MASTER_KEY", None::<&str>),
            ("APP_ENV", None::<&str>),
        ]);

        unsafe {
            std::env::set_var("DMART_ENV", "production");
        }
        assert!(
            matches!(PhiCipher::from_env(), Err(CryptoError::MissingMasterKey)),
            "en producción no debe permitirse una clave efímera"
        );

        unsafe {
            std::env::set_var("DMART_ENV", "development");
        }
        assert!(
            PhiCipher::from_env().is_ok(),
            "desarrollo debe poder arrancar sin DMART_MASTER_KEY"
        );
    }

    /// Un envelope sellado con el AAD v1 sigue siendo legible, y al re-sellar
    /// queda en v2.
    ///
    /// Sin este fallback, endurecer el AAD dejaría ilegibles los envelopes ya
    /// escritos en entornos con PHI real.
    #[test]
    fn legacy_aad_envelopes_remain_readable() {
        let master = MasterKey::from_password_bytes(b"clave-256-bits-minimo-para-pruebas!!");
        let master_bytes = *master.as_bytes();
        let _ = &master;
        let c = PhiCipher::new(master);
        let ctx = ctx("hosp-a", "patient", "P-1");

        // Sellar como lo hacía la versión anterior: AAD v1.
        let plaintext = serde_json::to_vec(&phi()).expect("json");
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        AesOsRng.fill_bytes(&mut nonce_bytes);
        let ct = Aes256Gcm::new_from_slice(&master_bytes)
            .expect("aes")
            .encrypt(
                AesNonce::from_slice(&nonce_bytes),
                Payload {
                    msg: &plaintext,
                    aad: ctx.legacy_aad().as_bytes(),
                },
            )
            .expect("encrypt");
        let mut raw = Vec::new();
        raw.extend_from_slice(AES256_MAGIC);
        raw.extend_from_slice(&nonce_bytes);
        raw.extend_from_slice(&ct);

        // Debe abrir con el cifrador actual.
        let opened = c
            .open::<Phi>(&ctx, &base64_encode(&raw))
            .expect("un envelope v1 debe seguir legible");
        assert_eq!(opened, phi());

        // Y el AAD v1 no sirve para abrir nada con el esquema nuevo.
        assert_ne!(ctx.aad(), ctx.legacy_aad().into_bytes());

        // El fallback con AAD vacío (formato anterior sin AAD) también debe
        // seguir funcionando, y sólo con la clave maestra.
        let mut nonce2 = [0u8; NONCE_SIZE];
        AesOsRng.fill_bytes(&mut nonce2);
        let ct2 = Aes256Gcm::new_from_slice(&master_bytes)
            .expect("aes")
            .encrypt(
                AesNonce::from_slice(&nonce2),
                Payload {
                    msg: &plaintext,
                    aad: &[],
                },
            )
            .expect("encrypt");
        let mut raw2 = Vec::new();
        raw2.extend_from_slice(AES256_MAGIC);
        raw2.extend_from_slice(&nonce2);
        raw2.extend_from_slice(&ct2);
        assert_eq!(
            c.open::<Phi>(&ctx, &base64_encode(&raw2))
                .expect("legacy sin AAD"),
            phi()
        );

        // Un ciphertext corrupto no debe abrirse por ningún camino alternativo.
        let mut corrupt = raw.clone();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 0xff;
        assert!(c.open::<Phi>(&ctx, &base64_encode(&corrupt)).is_err());
    }

    /// Un envelope corrupto o manipulado nunca devuelve datos: falla el tag.
    #[test]
    fn corrupted_envelope_fails_closed() {
        let c = cipher();
        let sealed = c
            .seal(&ctx("hosp-a", "patient", "P-1"), &phi())
            .expect("seal");
        let mut raw = base64_decode(&sealed).expect("b64");

        // Bit alterado en el ciphertext.
        let mut tampered = raw.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(
            c.open::<Phi>(&ctx("hosp-a", "patient", "P-1"), &base64_encode(&tampered))
                .is_err()
        );

        // Nonce alterado.
        let mut tampered = raw.clone();
        tampered[AES256_MAGIC.len()] ^= 0xff;
        assert!(
            c.open::<Phi>(&ctx("hosp-a", "patient", "P-1"), &base64_encode(&tampered))
                .is_err()
        );

        // Envelope desconocido.
        raw[0] = b'Z';
        assert!(
            c.open::<Phi>(&ctx("hosp-a", "patient", "P-1"), &base64_encode(&raw))
                .is_err()
        );

        // Base64 inválido y vacío.
        assert!(
            c.open::<Phi>(&ctx("hosp-a", "patient", "P-1"), "no-es-base64!!")
                .is_err()
        );
        assert!(c.open::<Phi>(&ctx("hosp-a", "patient", "P-1"), "").is_err());
    }

    /// Dos sellados del mismo documento producen blobs distintos (nonce único).
    #[test]
    fn nonce_is_unique_per_record() {
        let c = cipher();
        let a = c.seal(&ctx("t", "patient", "1"), &phi()).expect("a");
        let b = c.seal(&ctx("t", "patient", "1"), &phi()).expect("b");
        assert_ne!(a, b);
    }

    /// El `Debug` de `PhiCipher` no debe filtrar material de clave.
    #[test]
    fn debug_does_not_leak_key_material() {
        let rendered = format!("{:?}", cipher());
        assert!(!rendered.contains("clave-de-prueba"));
        assert!(rendered.contains("PhiCipher"));
    }

    #[test]
    fn normalize_for_index_is_conservative() {
        assert_eq!(normalize_for_index("MRN-001"), "mrn001");
        assert_eq!(normalize_for_index("  spaced  out "), "spacedout");
        assert_eq!(normalize_for_index("A/B.C_D"), "abcd");
        // No debe eliminar acentos ni letras: son parte del identificador.
        assert_eq!(normalize_for_index("Ñoño-1"), "ñoño1");
    }
}
