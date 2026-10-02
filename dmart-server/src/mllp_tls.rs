//! mTLS para el listener HL7/MLLP (SPEC-052, auditoría H3).
//!
//! MLLP es un protocolo de texto plano sobre TCP: tal cual transporta constantes
//! vitales y cualquier secreto de autenticación **en claro**, y cualquier host de
//! la red puede inyectar o leer. Este módulo cierra esa brecha exigiendo **mTLS**
//! en el listener:
//!
//! * **TLS 1.3 exclusivamente** (no se habilita el feature `tls12`): sin
//!   renegociación y sin cifrados de generaciones anteriores.
//! * **Sólo AES-256-GCM** (`TLS13_AES_256_GCM_SHA384`). Se construye un
//!   `CryptoProvider` propio descartando AES-128 y ChaCha20-Poly1305, de modo que
//!   el enlace bedside quede cifrado con AES-256, igual que los datos en reposo.
//! * **Certificado de cliente obligatorio** (`WebPkiClientVerifier`), validado
//!   contra la CA propia de monitores (`DMART_MLLP_CA_FILE`), nunca contra la CA
//!   pública del sistema.
//! * **Pinning de identidad por fingerprint SHA-256 del certificado**. La
//!   identidad se vincula al `MSH.3` que la conexión puede declarar
//!   (`DMART_MLLP_CLIENT_IDS`). Así:
//!   - un certificado no revocado/revocado por la CA sigue sin servir si no está
//!     en el pin (no depende sólo de la validez de la cadena),
//!   - un monitor no puede hacerse pasar por otro: si el pin dice `monitor-bed-1`
//!     y el frame dice `ventilador-9`, el frame se rechaza.
//!
//! Claves privadas: se leen con [`rustls_pemfile::read_all`] y se envuelven en
//! tipos de clave de `rustls` que no exponen copias intermedias en un `Vec` sin
//! limpiar.

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustls::client::danger::ServerCertVerifier;
use rustls::crypto::{CryptoProvider, ring};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use sha2::{Digest, Sha256};

/// Suite aceptada para el listener MLLP.
///
/// Se ofrece únicamente AES-256-GCM. `TLS13_AES_128_GCM_SHA256` y
/// `TLS13_CHACHA20_POLY1305_SHA256` quedan excluidos: el canal de PHI bedside se
/// cifra con AES-256, alineado con el cifrado en reposo.
pub const CIPHER_SUITE_AES256_GCM: rustls::SupportedCipherSuite =
    rustls::crypto::ring::cipher_suite::TLS13_AES_256_GCM_SHA384;

/// Protocolos aceptados. Sólo TLS 1.3 (sin TLS 1.2 ni renegociación).
pub const ALLOWED_PROTOCOLS: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

/// Identidad de un monitor, fijada por el fingerprint de su certificado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    /// SHA-256 del DER del certificado de cliente, hex minúsculas, sin `:`.
    pub cert_sha256: String,
    /// `MSH.3` que esa identidad puede declarar en sus mensajes.
    pub msh_sender: String,
}

impl ClientIdentity {
    /// Normaliza un fingerprint admitido con o sin separadores `:`/guiones.
    pub fn normalize_fingerprint(raw: &str) -> String {
        raw.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect()
    }
}

/// Configuración del material y de la política mTLS del listener.
#[derive(Debug, Clone)]
pub struct MllpTlsConfig {
    /// Certificado del servidor (cadena completa, PEM).
    pub cert_path: PathBuf,
    /// Clave privada del servidor (PEM PKCS#8, PKCS#1 o SEC1).
    pub key_path: PathBuf,
    /// CA que firma los certificados de los monitores.
    pub ca_path: PathBuf,
    /// Identidades permitidas (fingerprint → `MSH.3`).
    pub identities: Vec<ClientIdentity>,
}

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("configuración mTLS incompleta: {0}")]
    Incomplete(&'static str),
    #[error("no se pudo leer {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("PEM inválido en {path}: {reason}")]
    Pem { path: PathBuf, reason: String },
    #[error("no hay certificados utilizables en {0}")]
    NoCerts(PathBuf),
    #[error("política TLS inválida: {0}")]
    Policy(String),
}

impl MllpTlsConfig {
    /// Lee la configuración del entorno.
    ///
    /// Habilitado si `DMART_MLLP_CERT_FILE`, `DMART_MLLP_KEY_FILE` y
    /// `DMART_MLLP_CA_FILE` están presentes (los tres o ninguno). Con mTLS
    /// activo `DMART_MLLP_CLIENT_IDS` es **obligatoria**: sin ella se aceptaría
    /// cualquier certificado emitido por la CA.
    ///
    /// `DMART_MLLP_CLIENT_IDS` tiene la forma
    /// `<sha256-del-cert>=<MSH.3>[,<sha256>=<MSH.3>...]`.
    pub fn from_env() -> Result<Option<Self>, TlsError> {
        let cert = env_opt("DMART_MLLP_CERT_FILE");
        let key = env_opt("DMART_MLLP_KEY_FILE");
        let ca = env_opt("DMART_MLLP_CA_FILE");

        match (cert, key, ca) {
            (None, None, None) => Ok(None),
            (Some(cert), Some(key), Some(ca)) => {
                let identities = parse_client_ids(&env_csv("DMART_MLLP_CLIENT_IDS"));
                if identities.is_empty() {
                    return Err(TlsError::Incomplete(
                        "DMART_MLLP_CLIENT_IDS es obligatoria con mTLS activo: sin ella se \
                         aceptaría cualquier certificado de la CA",
                    ));
                }
                Ok(Some(Self {
                    cert_path: cert.into(),
                    key_path: key.into(),
                    ca_path: ca.into(),
                    identities,
                }))
            }
            _ => Err(TlsError::Incomplete(
                "mTLS está a medias: define DMART_MLLP_CERT_FILE, DMART_MLLP_KEY_FILE y \
                 DMART_MLLP_CA_FILE a la vez, o ninguna",
            )),
        }
    }

    /// CryptoProvider restringido a AES-256-GCM.
    ///
    /// rustls 0.23 configura las suites a nivel de provider, no de builder: hay
    /// que clonar el provider de `ring` y sustituir su lista de suites.
    pub fn crypto_provider() -> Arc<CryptoProvider> {
        let mut provider = ring::default_provider();
        provider.cipher_suites = vec![CIPHER_SUITE_AES256_GCM];
        Arc::new(provider)
    }

    /// Carga el material y construye el [`tokio_rustls::TlsAcceptor`].
    ///
    /// Falla si la clave no corresponde al certificado, si la CA no contiene
    /// raíces utilizables, o si el fingerprint del servidor no está en el pin.
    pub fn acceptor(&self) -> Result<tokio_rustls::TlsAcceptor, TlsError> {
        let cert_chain = load_certs(&self.cert_path)?;
        if cert_chain.is_empty() {
            return Err(TlsError::NoCerts(self.cert_path.clone()));
        }
        let key = load_private_key(&self.key_path)?;
        let roots = load_certs(&self.ca_path)?;
        if roots.is_empty() {
            return Err(TlsError::NoCerts(self.ca_path.clone()));
        }

        // Verificador de clientes: exige certificado y valida la cadena contra
        // la CA propia de monitores.
        let verifier = WebPkiClientVerifier::builder(Arc::new(root_store(roots)?))
            .build()
            .map_err(|e| TlsError::Policy(format!("verificador de clientes: {e}")))?;

        let server_config = ServerConfig::builder_with_provider(Self::crypto_provider())
            .with_protocol_versions(ALLOWED_PROTOCOLS)
            .map_err(|e| TlsError::Policy(format!("protocolos: {e}")))?
            .with_client_cert_verifier(verifier)
            .with_single_cert(cert_chain, key)
            .map_err(|e| TlsError::Policy(format!("certificado del servidor: {e}")))?;

        debug_assert_eq!(
            server_config.crypto_provider().cipher_suites.len(),
            1,
            "la política TLS debe quedar restringida a AES-256-GCM"
        );

        Ok(tokio_rustls::TlsAcceptor::from(Arc::new(server_config)))
    }

    /// Fingerprint SHA-256 (hex) del primer certificado de una cadena.
    pub fn fingerprint(der: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(der);
        let digest = hasher.finalize();
        // `write!` sobre un array de hex no puede fallar, pero se evita `unwrap`
        // en camino caliente usando un LUT.
        const LUT: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(64);
        for byte in digest {
            out.push(LUT[(byte >> 4) as usize] as char);
            out.push(LUT[(byte & 0x0f) as usize] as char);
        }
        out
    }

    /// Resuelve el `MSH.3` autorizado para el certificado presentado.
    ///
    /// Devuelve `None` si el fingerprint no está en el pin: la conexión se
    /// cierra antes de procesar cualquier dato.
    pub fn sender_for_cert(&self, der: &[u8]) -> Option<&str> {
        let fp = Self::fingerprint(der);
        self.identities
            .iter()
            .find(|id| id.cert_sha256 == fp)
            .map(|id| id.msh_sender.as_str())
    }

    /// `MSH.3` admitidos, para logging y para la allowlist de emisores.
    pub fn allowed_senders(&self) -> Vec<String> {
        self.identities
            .iter()
            .map(|id| id.msh_sender.clone())
            .collect()
    }

    /// Fingerprints admitidos, en hexadecimal.
    pub fn allowed_fingerprints(&self) -> Vec<String> {
        self.identities
            .iter()
            .map(|id| id.cert_sha256.clone())
            .collect()
    }
}

/// Parsea `DMART_MLLP_CLIENT_IDS`: `<fingerprint>=<msh3>[,...]`.
///
/// Entradas mal formadas se descartan silenciosamente en el parser pero
/// [`MllpTlsConfig::from_env`] falla si no queda ninguna válida, de modo que un
/// typo no degrade a "acepta cualquier certificado" sin que se note.
pub fn parse_client_ids(raw: &[String]) -> Vec<ClientIdentity> {
    raw.iter()
        .filter_map(|entry| {
            let (fp, sender) = entry.split_once('=')?;
            let fp = ClientIdentity::normalize_fingerprint(fp);
            let sender = sender.trim();
            if fp.len() != 64 || !fp.chars().all(|c| c.is_ascii_hexdigit()) || sender.is_empty() {
                return None;
            }
            Some(ClientIdentity {
                cert_sha256: fp,
                msh_sender: sender.to_string(),
            })
        })
        .collect()
}

fn root_store(roots: Vec<CertificateDer<'static>>) -> Result<RootCertStore, TlsError> {
    let mut store = RootCertStore::empty();
    for cert in roots {
        store
            .add(cert)
            .map_err(|e| TlsError::Policy(format!("CA de clientes: {e}")))?;
    }
    Ok(store)
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let file = File::open(path).map_err(|source| TlsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Pem {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
}

fn pem_key_from(reader: &mut impl std::io::BufRead, path: &Path) -> Option<PrivateKeyDer<'static>> {
    for item in rustls_pemfile::read_all(reader) {
        match item.ok()? {
            rustls_pemfile::Item::Pkcs8Key(k) => return Some(PrivateKeyDer::Pkcs8(k)),
            rustls_pemfile::Item::Pkcs1Key(k) => return Some(PrivateKeyDer::Pkcs1(k)),
            rustls_pemfile::Item::Sec1Key(k) => return Some(PrivateKeyDer::Sec1(k)),
            _ => continue,
        }
    }
    let _ = path;
    None
}

fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, TlsError> {
    let file = File::open(path).map_err(|source| TlsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    pem_key_from(&mut reader, path).ok_or_else(|| TlsError::Pem {
        path: path.to_path_buf(),
        reason: "no contiene una clave privada PEM (PKCS#8, PKCS#1 o SEC1)".into(),
    })
}

/// Configuración de cliente mTLS.
///
/// La usan las pruebas y los gateways que necesitan hablar MLLP sobre mTLS sin
/// montar un listener propio.
pub fn client_config(
    ca_pem: &[u8],
    client_cert_pem: &[u8],
    client_key_pem: &[u8],
) -> Result<ClientConfig, TlsError> {
    let mut roots = RootCertStore::empty();
    for cert in parse_pem_certs(Cursor::new(ca_pem))? {
        roots
            .add(cert)
            .map_err(|e| TlsError::Policy(format!("CA: {e}")))?;
    }
    let certs = parse_pem_certs(Cursor::new(client_cert_pem))?;
    if certs.is_empty() {
        return Err(TlsError::NoCerts(PathBuf::from("<cert cliente>")));
    }
    let mut reader = Cursor::new(client_key_pem);
    let key =
        pem_key_from(&mut reader, Path::new("<key cliente>")).ok_or_else(|| TlsError::Pem {
            path: PathBuf::from("<key cliente>"),
            reason: "no contiene clave privada PEM".into(),
        })?;

    ClientConfig::builder_with_provider(MllpTlsConfig::crypto_provider())
        .with_protocol_versions(ALLOWED_PROTOCOLS)
        .map_err(|e| TlsError::Policy(format!("protocolos cliente: {e}")))?
        .with_root_certificates(roots)
        .with_client_auth_cert(certs, key)
        .map_err(|e| TlsError::Policy(format!("certificado de cliente: {e}")))
}

fn parse_pem_certs(cursor: Cursor<&[u8]>) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let mut reader = BufReader::new(cursor);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Pem {
            path: PathBuf::from("<memoria>"),
            reason: e.to_string(),
        })
}

// Re-export para que los tests de integración construyan verificadores de
// servidor sin depender de rutas privadas del crate.
pub use rustls::client::WebPkiServerVerifier;

/// Helper para verificar el certificado del servidor en el lado cliente con un
/// fingerprint fijado (defensa contra CA mal configurada).
pub fn server_verifier(
    ca_pem: &[u8],
    expected_server_fingerprint: &str,
) -> Result<Arc<dyn ServerCertVerifier>, TlsError> {
    let mut roots = RootCertStore::empty();
    for cert in parse_pem_certs(Cursor::new(ca_pem))? {
        roots
            .add(cert)
            .map_err(|e| TlsError::Policy(format!("CA: {e}")))?;
    }
    let expected = ClientIdentity::normalize_fingerprint(expected_server_fingerprint);
    if expected.len() != 64 {
        return Err(TlsError::Policy(
            "el fingerprint del servidor debe ser 64 hex chars".into(),
        ));
    }
    let verifier = WebPkiServerVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| TlsError::Policy(format!("verificador de servidor: {e}")))?;
    Ok(Arc::new(PinnedServerVerifier {
        inner: verifier,
        expected,
    }))
}

/// Verificador que delega la cadena en `WebPkiServerVerifier` y además exige
/// que el fingerprint del servidor sea el fijado.
struct PinnedServerVerifier {
    inner: Arc<WebPkiServerVerifier>,
    expected: String,
}

impl std::fmt::Debug for PinnedServerVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedServerVerifier")
            .field("expected", &self.expected)
            .finish_non_exhaustive()
    }
}

impl ServerCertVerifier for PinnedServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;
        if MllpTlsConfig::fingerprint(end_entity.as_ref()) != self.expected {
            return Err(rustls::Error::General(
                "fingerprint del servidor distinto del fijado".into(),
            ));
        }
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_csv(key: &str) -> Vec<String> {
    env_opt(key)
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(identities: Vec<ClientIdentity>) -> MllpTlsConfig {
        MllpTlsConfig {
            cert_path: "cert.pem".into(),
            key_path: "key.pem".into(),
            ca_path: "ca.pem".into(),
            identities,
        }
    }

    fn id(fp: &str, sender: &str) -> ClientIdentity {
        ClientIdentity {
            cert_sha256: fp.to_string(),
            msh_sender: sender.to_string(),
        }
    }

    const FP_A: &str = "aa000000000000000000000000000000000000000000000000000000000000bb";
    const FP_B: &str = "cc000000000000000000000000000000000000000000000000000000000000dd";

    #[test]
    fn cipher_policy_is_aes256_only() {
        assert_eq!(
            CIPHER_SUITE_AES256_GCM.suite(),
            rustls::CipherSuite::TLS13_AES_256_GCM_SHA384
        );
        let provider = MllpTlsConfig::crypto_provider();
        assert_eq!(provider.cipher_suites.len(), 1);
        assert_eq!(provider.cipher_suites[0], CIPHER_SUITE_AES256_GCM);
        assert!(
            !provider
                .cipher_suites
                .iter()
                .any(|s| matches!(s.suite(), rustls::CipherSuite::TLS13_AES_128_GCM_SHA256))
        );
        assert!(!provider.cipher_suites.iter().any(|s| matches!(
            s.suite(),
            rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256
        )));
    }

    #[test]
    fn protocol_policy_is_tls13_only() {
        assert_eq!(ALLOWED_PROTOCOLS.len(), 1);
        assert_eq!(
            ALLOWED_PROTOCOLS[0].version,
            rustls::ProtocolVersion::TLSv1_3
        );
    }

    #[test]
    fn identity_is_bound_by_certificate_fingerprint() {
        let c = cfg(vec![id(FP_A, "monitor-bed-1")]);
        let der = vec![0x30u8; 10];
        // El fingerprint real del DER no coincide con el pin → sin emisor.
        assert_eq!(c.sender_for_cert(&der), None);
        // Y con el pin correcto resuelve al MSH.3 autorizado.
        let pinned = vec![0u8; 32];
        let fp = MllpTlsConfig::fingerprint(&pinned);
        let c2 = cfg(vec![id(&fp, "monitor-bed-1")]);
        assert_eq!(c2.sender_for_cert(&pinned), Some("monitor-bed-1"));
        assert_eq!(c.allowed_senders(), vec!["monitor-bed-1".to_string()]);
    }

    #[test]
    fn fingerprint_is_stable_and_hex() {
        let der = b"\x01\x02\x03";
        let fp = MllpTlsConfig::fingerprint(der);
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(fp, MllpTlsConfig::fingerprint(der));
        assert_ne!(fp, MllpTlsConfig::fingerprint(b"\x01\x02\x04"));
    }

    #[test]
    fn client_ids_parsing() {
        let raw = vec![
            format!("{}:{}={}", "AA", &"b".repeat(62), "monitor-bed-1"),
            format!("{}=ventilador-7", "c".repeat(64)),
            "basura".to_string(),
            format!("{}=no-es-hex", "z".repeat(64)),
            format!("{}={}", "d".repeat(10), "x"),
            format!("{}=", "e".repeat(64)),
        ];
        let ids = parse_client_ids(&raw);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].cert_sha256, format!("aa{}", "b".repeat(62)));
        assert_eq!(ids[0].msh_sender, "monitor-bed-1");
        assert_eq!(ids[1].msh_sender, "ventilador-7");
    }

    #[test]
    fn fingerprint_normalization() {
        assert_eq!(
            ClientIdentity::normalize_fingerprint("AA:BB-cc_11"),
            "aabbcc11"
        );
    }

    #[test]
    fn missing_files_report_io_errors() {
        let err = cfg(vec![id(FP_A, "m")]).acceptor().err();
        assert!(
            matches!(err, Some(TlsError::Io { .. })),
            "debe fallar por ficheros ausentes, se obtuvo {err:?}"
        );
    }

    #[test]
    fn env_helpers_tolerate_absent_vars() {
        assert_eq!(env_opt("DMART_MLLP_VAR_INEXISTENTE_TEST"), None);
        assert!(env_csv("DMART_MLLP_VAR_INEXISTENTE_TEST").is_empty());
    }
}

#[cfg(test)]
mod mtls_integration_tests {
    use super::*;
    use rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType,
    };
    use std::net::SocketAddr;

    /// Material de la CA propia de monitores: certificado + clave.
    struct Ca {
        cert: rcgen::Certificate,
        key: KeyPair,
        pem: String,
    }

    fn make_ca() -> Ca {
        let mut params = CertificateParams::new(Vec::<String>::new()).expect("ca params");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "dmart-monitors-ca");
        params.distinguished_name = dn;
        let key = KeyPair::generate().expect("ca key");
        let cert = params.self_signed(&key).expect("ca cert");
        let pem = cert.pem();
        Ca { cert, key, pem }
    }

    /// Monitor: certificado firmado por la CA, con su PEM de clave y el
    /// fingerprint que se usará como pin.
    struct Monitor {
        cert_pem: String,
        key_pem: String,
        fingerprint: String,
    }

    fn make_monitor(ca: &Ca, cn: &str) -> Monitor {
        let mut params = CertificateParams::new(vec![cn.to_string()]).expect("monitor params");
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, cn);
        params.distinguished_name = dn;
        params.subject_alt_names = vec![SanType::DnsName(cn.try_into().expect("dns ia5"))];

        let key = KeyPair::generate().expect("monitor key");
        let cert = params
            .signed_by(&key, &ca.cert, &ca.key)
            .expect("firmar monitor");
        let fingerprint = MllpTlsConfig::fingerprint(cert.der().as_ref());
        Monitor {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
            fingerprint,
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        cfg: MllpTlsConfig,
        ca_pem: String,
        monitors: Vec<Monitor>,
    }

    /// Monta CA + servidor + monitores y deja la configuración lista para
    /// `acceptor()`.
    fn fixture() -> Fixture {
        let ca = make_ca();
        let dir = tempfile::tempdir().expect("tempdir");

        let mut server_params =
            CertificateParams::new(vec!["localhost".to_string()]).expect("srv params");
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "localhost");
        server_params.distinguished_name = dn;
        let server_key = KeyPair::generate().expect("srv key");
        let server_cert = server_params
            .signed_by(&server_key, &ca.cert, &ca.key)
            .expect("firmar srv");

        let cert_path = write_temp(&dir, "server.pem", &server_cert.pem());
        let key_path = write_temp(&dir, "server.key", &server_key.serialize_pem());
        let ca_path = write_temp(&dir, "ca.pem", &ca.pem);

        let monitors = vec![
            make_monitor(&ca, "monitor-bed-1"),
            make_monitor(&ca, "monitor-bed-2"),
        ];
        let identities = monitors
            .iter()
            .enumerate()
            .map(|(i, m)| ClientIdentity {
                cert_sha256: m.fingerprint.clone(),
                msh_sender: format!("emisor-bed-{}", i + 1),
            })
            .collect();

        Fixture {
            _dir: dir,
            cfg: MllpTlsConfig {
                cert_path,
                key_path,
                ca_path,
                identities,
            },
            ca_pem: ca.pem,
            monitors,
        }
    }

    fn write_temp(dir: &tempfile::TempDir, name: &str, pem: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, pem).expect("escribir pem");
        path
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    /// mTLS extremo a extremo: se negocia **TLS 1.3 + AES-256-GCM** y el
    /// certificado del cliente es obligatorio. Esta es la prueba de que el
    /// tráfico MLLP (con PHI) ya no viaja en claro.
    #[test]
    fn mtls_roundtrip_uses_tls13_aes256() {
        let fx = fixture();
        let acceptor = fx.cfg.acceptor().expect("acceptor");
        let rt = runtime();

        // Reserva el puerto antes de spawnear para que el cliente lo conozca.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
        let addr: SocketAddr = probe.local_addr().expect("addr");
        drop(probe);

        let client_cfg = client_config(
            fx.ca_pem.as_bytes(),
            fx.monitors[0].cert_pem.as_bytes(),
            fx.monitors[0].key_pem.as_bytes(),
        )
        .expect("client config");
        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_cfg));
        let tls_cfg = fx.cfg.clone();

        rt.block_on(async {
            // El listener se crea dentro del mismo `block_on` para que no haya
            // carrera entre `bind` y `connect`.
            let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
            let server = tokio::spawn(async move {
                let (tcp, _peer) = listener.accept().await.expect("accept");
                let tls = acceptor.accept(tcp).await.expect("tls accept");
                let conn = tls.get_ref().1;
                let version = conn.protocol_version().expect("version");
                let suite = conn.negotiated_cipher_suite().expect("suite");
                let certs = conn.peer_certificates().expect("peer certs");
                let peer_fp = MllpTlsConfig::fingerprint(certs[0].as_ref());
                // El pin resuelve la identidad del emisor.
                (
                    version,
                    suite.suite(),
                    peer_fp,
                    tls_cfg
                        .sender_for_cert(certs[0].as_ref())
                        .map(str::to_string),
                )
            });

            let tcp = tokio::net::TcpStream::connect(addr).await.expect("connect");
            connector
                .connect(
                    rustls::pki_types::ServerName::try_from("localhost").expect("server name"),
                    tcp,
                )
                .await
                .expect("tls connect");

            let (version, suite, peer_fp, sender) = server.await.expect("join");
            assert_eq!(version, rustls::ProtocolVersion::TLSv1_3, "sólo TLS 1.3");
            assert_eq!(
                suite,
                rustls::CipherSuite::TLS13_AES_256_GCM_SHA384,
                "sólo AES-256-GCM"
            );
            assert_eq!(peer_fp, fx.monitors[0].fingerprint);
            assert_eq!(sender.as_deref(), Some("emisor-bed-1"));
        });
    }

    /// Sin certificado de cliente el handshake falla: no existe modo anónimo.
    #[test]
    fn mtls_rejects_client_without_certificate() {
        let fx = fixture();
        let acceptor = fx.cfg.acceptor().expect("acceptor");
        let rt = runtime();

        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
        let addr: SocketAddr = probe.local_addr().expect("addr");
        drop(probe);

        let ca_pem = fx.ca_pem.clone();
        let accepted = rt.block_on(async move {
            let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
            let server = tokio::spawn(async move {
                let (tcp, _peer) = listener.accept().await.expect("accept");
                acceptor.accept(tcp).await.is_ok()
            });

            let mut roots = RootCertStore::empty();
            for cert in parse_pem_certs(Cursor::new(ca_pem.as_bytes())).expect("certs") {
                roots.add(cert).expect("root");
            }
            let cfg = ClientConfig::builder_with_provider(MllpTlsConfig::crypto_provider())
                .with_protocol_versions(ALLOWED_PROTOCOLS)
                .expect("protocolos")
                .with_root_certificates(roots)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(Arc::new(cfg));
            let tcp = tokio::net::TcpStream::connect(addr).await.expect("connect");
            // En TLS 1.3 el cliente completa su lado antes de recibir el
            // alert del servidor, así que la brillante prueba es doble:
            // el servidor rechaza el handshake y el primer I/O del cliente
            // falla (no hay ninguna superficie en claro utilizable).
            let mut tls = connector
                .connect(
                    rustls::pki_types::ServerName::try_from("localhost").expect("name"),
                    tcp,
                )
                .await
                .expect("el handshake del cliente puede completarse en TLS 1.3");

            let mut probe = [0u8; 1];
            let io_result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                tokio::io::AsyncReadExt::read(&mut tls, &mut probe),
            )
            .await;
            let client_sees_failure = match io_result {
                Err(_) => false, // el servidor ni siquiera cerró
                Ok(Err(_)) => true,
                Ok(Ok(_)) => false,
            };
            let server_ok = server.await.expect("join");
            assert!(
                !server_ok,
                "el servidor debe rechazar la conexión sin certificado"
            );
            assert!(
                client_sees_failure,
                "el cliente no debe poder leer datos del servidor"
            );
            server_ok
        });

        assert!(
            !accepted,
            "el servidor debe rechazar la conexión sin certificado"
        );
    }

    /// Una identidad fijada a un monitor no autoriza a otro: `MSH.3` se deriva
    /// del certificado, no de lo que diga el mensaje.
    #[test]
    fn identity_is_not_transferable_between_certificates() {
        let fx = fixture();
        let pinned_first = MllpTlsConfig {
            identities: vec![ClientIdentity {
                cert_sha256: fx.monitors[0].fingerprint.clone(),
                msh_sender: "emisor-bed-1".into(),
            }],
            ..fx.cfg.clone()
        };
        // El segundo monitor tiene un certificado válido emitido por la misma
        // CA, pero su fingerprint no está pinneado: no resuelve identidad.
        let second_der_fp = &fx.monitors[1].fingerprint;
        assert_ne!(
            second_der_fp, &fx.monitors[0].fingerprint,
            "los certificados de monitors distintos deben diferir"
        );
        assert_eq!(pinned_first.allowed_fingerprints().len(), 1);
        assert_eq!(
            pinned_first.allowed_senders(),
            vec!["emisor-bed-1".to_string()]
        );
        // Y con el DER real del segundo monitor, `sender_for_cert` no resuelve.
        let second_cert = parse_pem_certs(Cursor::new(fx.monitors[1].cert_pem.as_bytes()))
            .expect("certs")
            .remove(0);
        assert_eq!(pinned_first.sender_for_cert(second_cert.as_ref()), None);
    }

    /// Un certificado de la CA que no aparece en el pin no resuelve emisor:
    /// `serve()` cierra la conexión antes de leer un solo byte de MLLP.
    #[test]
    fn valid_but_unpinned_certificate_resolves_no_sender() {
        let fx = fixture();
        let only_first = MllpTlsConfig {
            identities: vec![ClientIdentity {
                cert_sha256: fx.monitors[0].fingerprint.clone(),
                msh_sender: "emisor-bed-1".into(),
            }],
            ..fx.cfg.clone()
        };
        let second_cert = parse_pem_certs(Cursor::new(fx.monitors[1].cert_pem.as_bytes()))
            .expect("certs")
            .remove(0);
        assert_eq!(
            only_first.sender_for_cert(second_cert.as_ref()),
            None,
            "certificado fuera del pin no autoriza ningún MSH.3"
        );
    }

    /// La clave del servidor debe corresponder al certificado; si no, el
    /// arranque falla en vez de servir con una identidad falsa.
    #[test]
    fn mismatched_server_key_is_rejected() {
        let fx = fixture();
        let other_key = KeyPair::generate().expect("key");
        let bad_key = write_temp(&fx._dir, "wrong.key", &other_key.serialize_pem());
        let cfg = MllpTlsConfig {
            key_path: bad_key,
            ..fx.cfg.clone()
        };
        assert!(
            cfg.acceptor().is_err(),
            "clave y certificado no deben aceptarse"
        );
    }

    /// Configuración a medias (cert + CA sin clave, p.ej.) falla: nunca hay
    /// downgrade silencioso a texto plano.
    #[test]
    fn partial_material_reports_error() {
        let fx = fixture();
        let cfg = MllpTlsConfig {
            key_path: PathBuf::from("/no/existe/server.key"),
            ..fx.cfg.clone()
        };
        assert!(matches!(cfg.acceptor(), Err(TlsError::Io { .. })));
    }
}

/// Pruebas end-to-end del listener MLLP real sobre mTLS.
///
/// Las pruebas de `mtls_integration_tests` validan la negociación TLS aislada.
/// Éstas arrancan `server_ingest::serve` de verdad, que es donde aparecen los
/// errores de integración: resolver la identidad con el material equivocado,
/// no liberar el cupo por IP, o dejar el MLLP accesible sin TLS.
///
/// El material TLS se inyecta con `serve_with_security` en vez de por
/// `DMART_MLLP_*`: esas variables son de proceso, y varias pruebas concurrentes
/// se pisarían el material unas a otras, haciendo el resultado depender del
/// orden de ejecución.
#[cfg(test)]
mod listener_tests {
    use super::*;
    use crate::db::Database;
    use crate::ingest::{IngestConfig, IngestState};
    use crate::server_ingest::{MllpSecurityConfig, serve_with_security};
    use rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType,
    };
    use std::net::SocketAddr;
    use std::sync::Arc;
    use surrealdb::Surreal;
    use surrealdb::engine::local::SurrealKv;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_rustls::TlsConnector;

    struct Ca {
        cert: rcgen::Certificate,
        key: KeyPair,
        pem: String,
    }

    fn make_ca() -> Ca {
        let mut params = CertificateParams::new(Vec::<String>::new()).expect("ca params");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "dmart-monitor-ca");
        params.distinguished_name = dn;
        let key = KeyPair::generate().expect("ca key");
        let cert = params.self_signed(&key).expect("ca cert");
        let pem = cert.pem();
        Ca { cert, key, pem }
    }

    struct Monitor {
        cert_pem: String,
        key_pem: String,
        fingerprint: String,
    }

    fn make_monitor(ca: &Ca, cn: &str) -> Monitor {
        let mut params = CertificateParams::new(vec![cn.to_string()]).expect("monitor params");
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, cn);
        params.distinguished_name = dn;
        params.subject_alt_names = vec![SanType::DnsName(cn.try_into().expect("dns ia5"))];
        let key = KeyPair::generate().expect("monitor key");
        let cert = params
            .signed_by(&key, &ca.cert, &ca.key)
            .expect("firmar monitor");
        Monitor {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
            fingerprint: MllpTlsConfig::fingerprint(cert.der().as_ref()),
        }
    }

    /// Listener MLLP real, con mTLS completo y una base de datos propia.
    struct Fixture {
        addr: SocketAddr,
        ca_pem: String,
        monitors: Vec<Monitor>,
        max_connections_per_ip: u32,
        _dir: tempfile::TempDir,
    }

    impl Fixture {
        async fn start(max_connections_per_ip: u32) -> Self {
            let ca = make_ca();
            let dir = tempfile::tempdir().expect("tempdir");

            let mut sp = CertificateParams::new(vec!["localhost".to_string()]).expect("srv params");
            let mut dn = DistinguishedName::new();
            dn.push(DnType::CommonName, "localhost");
            sp.distinguished_name = dn;
            let server_key = KeyPair::generate().expect("srv key");
            let server_cert = sp
                .signed_by(&server_key, &ca.cert, &ca.key)
                .expect("firmar srv");

            let cert_path = dir.path().join("server.pem");
            std::fs::write(&cert_path, server_cert.pem()).expect("cert");
            let key_path = dir.path().join("server.key");
            std::fs::write(&key_path, server_key.serialize_pem()).expect("key");
            let ca_path = dir.path().join("ca.pem");
            std::fs::write(&ca_path, ca.pem.clone()).expect("ca");

            let monitors = vec![
                make_monitor(&ca, "monitor-bed-1"),
                make_monitor(&ca, "monitor-bed-2"),
            ];
            let identities = monitors
                .iter()
                .enumerate()
                .map(|(i, m)| ClientIdentity {
                    cert_sha256: m.fingerprint.clone(),
                    msh_sender: format!("emisor-bed-{}", i + 1),
                })
                .collect();

            let tls_cfg = MllpTlsConfig {
                cert_path,
                key_path,
                ca_path,
                identities,
            };

            let db_dir = tempfile::tempdir().expect("db dir");
            let db_path = db_dir.path().join("listener.db");
            let db = Surreal::new::<SurrealKv>(db_path.to_str().expect("utf8"))
                .await
                .expect("db");
            db.use_ns("dmart").use_db("icu").await.expect("ns/db");
            crate::migrations::run_migrations(&db)
                .await
                .expect("migrations");
            let db: Database = Arc::new(db);

            // El frame de prueba referencia MRN-TEST; sin este paciente el
            // pipeline respondería `patient_not_found` y la prueba mediría el
            // fallo del pipeline, no el del transporte.
            let mut patient = dmart_shared::models::Patient::new();
            patient.tenant_id = "dmart".into();
            patient.historia_clinica = "MRN-TEST".into();
            patient.nombre = "Test".into();
            patient.apellido = "Paciente".into();
            crate::db::create_patient(&db, patient)
                .await
                .expect("paciente de la fixture");

            let probe = TcpListener::bind("127.0.0.1:0").await.expect("probe");
            let addr = probe.local_addr().expect("addr");
            drop(probe);

            let sec = MllpSecurityConfig {
                // Sin secreto de capa 2: la identidad la fija el certificado, y
                // duplicar el secreto en el cable sólo añadiría otro secreto que
                // proteger.
                auth_secret: None,
                allowed_senders: Vec::new(),
                read_timeout: std::time::Duration::from_secs(5),
                write_timeout: std::time::Duration::from_secs(5),
                max_connections: 64,
                max_connections_per_ip: max_connections_per_ip as usize,
            };

            let state = Arc::new(IngestState::new(IngestConfig::default()));
            tokio::spawn(async move {
                let _ = serve_with_security(addr, db, state, sec, Some(tls_cfg)).await;
            });

            for _ in 0..200 {
                if TcpStream::connect(addr).await.is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }

            Self {
                addr,
                ca_pem: ca.pem,
                monitors,
                max_connections_per_ip,
                _dir: dir,
            }
        }

        /// Tope por IP con el que se configuró el listener.
        fn ip_limit(&self) -> usize {
            self.max_connections_per_ip as usize
        }

        /// Cliente mTLS con el certificado pinneado indicado.
        async fn connect_monitor(&self, idx: usize) -> tokio_rustls::client::TlsStream<TcpStream> {
            let m = self.monitors.get(idx).expect("monitor registrado");
            self.connect_with(&m.cert_pem, &m.key_pem).await
        }

        /// Certificado **no** pinneado: emitido por una CA distinta, que el
        /// listener no acepta.
        async fn connect_stranger(&self) -> tokio_rustls::client::TlsStream<TcpStream> {
            let other_ca = make_ca();
            let stranger = make_monitor(&other_ca, "monitor-ajeno");
            self.connect_with(&stranger.cert_pem, &stranger.key_pem)
                .await
        }

        async fn connect_with(
            &self,
            cert_pem: &str,
            key_pem: &str,
        ) -> tokio_rustls::client::TlsStream<TcpStream> {
            let certs: Vec<rustls_pki_types::CertificateDer<'static>> =
                rustls_pemfile::certs(&mut cert_pem.as_bytes())
                    .collect::<Result<_, _>>()
                    .expect("certs de cliente");
            let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
                .expect("clave de cliente")
                .expect("clave presente");
            let cfg = rustls::ClientConfig::builder_with_provider(MllpTlsConfig::crypto_provider())
                .with_protocol_versions(&[&rustls::version::TLS13])
                .expect("builder")
                .with_root_certificates(roots_from_pem(&self.ca_pem))
                .with_client_auth_cert(certs, key)
                .expect("cfg de cliente");

            let stream = TcpStream::connect(self.addr).await.expect("connect");
            // El servidor valida contra "localhost"; el certificado lo firma la
            // misma CA de pruebas, así que el nombre sí coincide.
            let name = rustls_pki_types::ServerName::try_from("localhost").expect("server name");
            TlsConnector::from(Arc::new(cfg))
                .connect(name, stream)
                .await
                .expect("handshake mTLS")
        }

        /// Lee el ACK MLLP, que termina en `\x1C\r`.
        async fn read_ack(&self, s: &mut tokio_rustls::client::TlsStream<TcpStream>) -> Vec<u8> {
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            for _ in 0..1024 {
                match tokio::time::timeout(std::time::Duration::from_secs(3), s.read(&mut byte))
                    .await
                {
                    Ok(Ok(0)) => break,
                    Ok(Ok(_)) => {
                        buf.push(byte[0]);
                        if buf.ends_with(&[0x1c, b'\r']) {
                            break;
                        }
                    }
                    _ => break,
                }
            }
            buf
        }
    }

    fn roots_from_pem(pem: &str) -> rustls::RootCertStore {
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_pemfile::certs(&mut pem.as_bytes()) {
            roots.add(cert.expect("cert de CA")).expect("añadir CA");
        }
        roots
    }

    /// Frame MLLP `ORU^R01` con el `MSH.3` indicado.
    ///
    /// Tiene que ser un `ORU^R01` real: el parser rechaza cualquier otro tipo
    /// antes de llegar a la comprobación de identidad, y la prueba mide
    /// precisamente esa comprobación.
    fn frame_msh3(sender: &str) -> Vec<u8> {
        let body = format!(
            "MSH|^~\\&|{sender}|DMART|dmart-monitor|HOSP|20260821141031||ORU^R01|1|P|2.5\r\
             PID|||MRN-TEST^^^FAC||TEST^PACIENTE||19600415|M\r\
             OBR|1|||||||20260821141030\r\
             OBX|1|NM|8867-4^Heart rate^LN||210|bpm\r"
        );
        let mut f = vec![0x0b];
        f.extend_from_slice(body.as_bytes());
        f.push(0x1c);
        f.extend_from_slice(b"\r");
        f
    }

    /// Bytes MLLP que el listener no debe contestar sin TLS.
    fn plaintext_probe() -> Vec<u8> {
        let mut f = vec![0x0b];
        f.extend_from_slice(b"MSH|^~\\&|MINDRAY|DMART\r");
        f.push(0x1c);
        f.extend_from_slice(b"\r");
        f
    }

    /// ¿Contiene la sesión un ACK MLLP válido?
    fn has_ack(bytes: &[u8]) -> bool {
        bytes.windows(2).any(|w| w == [0x1c, b'\r'])
    }

    /// ACK de rechazo: `ACKAR^` más el motivo. El servidor responde con un error
    /// explícito en vez de cerrar en silencio, para que el monitor sepa que su
    /// mensaje no entró.
    fn is_rejection(bytes: &[u8], reason: &str) -> bool {
        let text = String::from_utf8_lossy(bytes);
        text.contains("ACKAR^") && text.contains(reason)
    }

    /// El camino completo: mTLS válido, el frame entra al pipeline y el ACK
    /// vuelve por la sesión cifrada.
    #[tokio::test]
    async fn listener_accepts_frame_over_mtls() {
        let fx = Fixture::start(4).await;
        let mut tls = fx.connect_monitor(0).await;

        tls.write_all(&frame_msh3("emisor-bed-1"))
            .await
            .expect("write");
        tls.flush().await.expect("flush");

        let ack = fx.read_ack(&mut tls).await;
        assert!(
            has_ack(&ack),
            "debe llegar ACK por la sesión cifrada: {ack:?}"
        );
    }

    /// Regresión del doble hash: `serve` pasaba a `sender_for_cert` el
    /// fingerprint hexadecimal, que la función volvía a hashear. Con ese bug
    /// ninguna identidad resolvía y **toda** conexión mTLS válida se rechazaba.
    #[tokio::test]
    async fn pinned_monitor_is_accepted() {
        let fx = Fixture::start(4).await;
        let mut tls = fx.connect_monitor(0).await;
        tls.write_all(&frame_msh3("emisor-bed-1"))
            .await
            .expect("write");
        let ack = fx.read_ack(&mut tls).await;
        assert!(
            has_ack(&ack),
            "un monitor pinneado debe llegar al pipeline: {ack:?}"
        );
    }

    /// Ambos monitores pinneados resuelven a su propia identidad.
    #[tokio::test]
    async fn every_pinned_monitor_is_accepted() {
        let fx = Fixture::start(4).await;
        for idx in 0..fx.monitors.len() {
            let sender = format!("emisor-bed-{}", idx + 1);
            let mut tls = fx.connect_monitor(idx).await;
            tls.write_all(&frame_msh3(&sender)).await.expect("write");
            let ack = fx.read_ack(&mut tls).await;
            assert!(has_ack(&ack), "{sender} pinneado debe aceptarse: {ack:?}");
        }
    }

    /// Certificado válido contra la CA pero fuera del pin: se rechaza antes de
    /// procesar el mensaje.
    #[tokio::test]
    async fn unpinned_certificate_is_rejected() {
        let fx = Fixture::start(4).await;
        let mut tls = fx.connect_stranger().await;
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            tls.write_all(&frame_msh3("emisor-bed-1")),
        )
        .await;
        let ack = fx.read_ack(&mut tls).await;
        assert!(
            !has_ack(&ack),
            "un certificado fuera del pin no debe recibir ACK: {ack:?}"
        );
    }

    /// El `MSH.3` del mensaje debe coincidir con la identidad del certificado.
    /// Un certificado pinneado no puede hablar en nombre de otro emisor.
    #[tokio::test]
    async fn msh3_must_match_the_pinned_identity() {
        let fx = Fixture::start(4).await;
        let mut tls = fx.connect_monitor(0).await;

        // El certificado es `emisor-bed-1`; el mensaje dice `emisor-bed-2`.
        tls.write_all(&frame_msh3("emisor-bed-2"))
            .await
            .expect("write");
        let ack = fx.read_ack(&mut tls).await;
        assert!(
            is_rejection(&ack, "sender_not_allowed"),
            "un MSH.3 ajeno al certificado debe recibir ACK de rechazo: {ack:?}"
        );

        // Y el mensaje de la identidad legítima sí se acepta, lo que confirma
        // que el rechazo anterior fue por el MSH.3 y no por el transporte.
        tls.write_all(&frame_msh3("emisor-bed-1"))
            .await
            .expect("write");
        let ok = fx.read_ack(&mut tls).await;
        assert!(
            String::from_utf8_lossy(&ok).contains("ACKAA^"),
            "el MSH.3 del certificado debe aceptarse: {ok:?}"
        );
    }

    /// Sin handshake TLS no hay protocolo MLLP: mandar MLLP en claro no obtiene
    /// respuesta.
    #[tokio::test]
    async fn plaintext_mllp_is_not_answered() {
        let fx = Fixture::start(4).await;
        let mut stream = TcpStream::connect(fx.addr).await.expect("connect");
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.write_all(&plaintext_probe()),
        )
        .await;

        let mut buf = Vec::new();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read_to_end(&mut buf),
        )
        .await;
        assert!(
            !has_ack(&buf),
            "no debe haber ACK sobre una conexión sin TLS: {buf:?}"
        );
    }

    /// Cliente sin certificado: no completa el handshake y no llega a leerse un
    /// solo byte de MLLP.
    #[tokio::test]
    async fn client_without_certificate_is_rejected() {
        let fx = Fixture::start(4).await;
        let mut stream = TcpStream::connect(fx.addr).await.expect("connect");
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.write_all(&frame_msh3("emisor-bed-1")),
        )
        .await;

        let mut buf = Vec::new();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read_to_end(&mut buf),
        )
        .await;
        assert!(!has_ack(&buf), "sin certificado no debe haber ACK: {buf:?}");
    }

    /// Regresión del cupo por IP: cada conexión mTLS válida debe liberar su
    /// hueco al cerrarse. Con el bug, `max_connections_per_ip` conexiones
    /// sucesivas agotaban el límite y bloqueaban al emisor de forma permanente,
    /// sin ninguna ruta de recuperación.
    #[tokio::test]
    async fn repeated_connections_do_not_exhaust_the_ip_limit() {
        const TOPE: u32 = 2;
        let fx = Fixture::start(TOPE).await;
        assert_eq!(
            fx.ip_limit(),
            TOPE as usize,
            "el listener debe recibir el tope configurado"
        );

        // Tres veces el tope: sólo es posible si los huecos se liberan.
        for i in 0..(TOPE * 3) {
            let mut tls = fx.connect_monitor(0).await;
            tls.write_all(&frame_msh3("emisor-bed-1"))
                .await
                .expect("write");
            let ack = fx.read_ack(&mut tls).await;
            assert!(
                has_ack(&ack),
                "conexión {i} de {} debe aceptarse: {ack:?}",
                TOPE * 3
            );
        }
    }
}
