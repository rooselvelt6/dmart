//! Detección del entorno de despliegue.
//!
//! Centraliza la pregunta "¿esto es producción?" que necesitan varios módulos
//! para aplicar políticas fail-closed. Estaba duplicada y con dos nombres de
//! variable distintos (`DMART_ENV` en el listener MLLP, `APP_ENV` en el
//! cifrado de PHI), lo que hacía que un despliegue de producción pudiera
//! arrancar con mTLS exigido pero clave de cifrado efímera. Ahora `DMART_ENV` es
//! la fuente única y `APP_ENV` se acepta como alias por compatibilidad.

/// Entornos que exigen configuración de seguridad explícita.
const PRODUCTION_NAMES: [&str; 3] = ["production", "prod", "PRODUCTION"];

/// `true` si el despliegue está declarado como productivo.
///
/// Se evalúa en cada llamada y no se cachea: los tests manipulan las variables
/// de entorno en runtime dentro del mismo proceso.
pub fn is_production() -> bool {
    ["DMART_ENV", "APP_ENV"].iter().any(|var| {
        std::env::var(var)
            .map(|v| {
                let v = v.trim();
                PRODUCTION_NAMES.iter().any(|p| v.eq_ignore_ascii_case(p))
            })
            .unwrap_or(false)
    })
}

/// `true` si una bandera de entorno está activa (`1`, `true`, `yes`, `on`).
pub fn flag_enabled(var: &str) -> bool {
    std::env::var(var)
        .map(|v| {
            let v = v.trim();
            v == "1"
                || v.eq_ignore_ascii_case("true")
                || v.eq_ignore_ascii_case("yes")
                || v.eq_ignore_ascii_case("on")
        })
        .unwrap_or(false)
}

/// Lock que serializa a los tests que manipulan variables de entorno.
///
/// Las variables de entorno son globales al proceso, y los tests de este crate
/// corren en paralelo dentro del mismo binario: sin serializar, un test que
/// fija `DMART_ENV=production` contamina a otro que espera desarrollo.
#[cfg(test)]
pub(crate) fn tests_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Guarda el valor previo de variables de entorno y lo restaura al soltar.
#[cfg(test)]
pub(crate) struct EnvGuard(Vec<(String, Option<String>)>);

#[cfg(test)]
impl EnvGuard {
    /// Fija cada variable a `Some(valor)` o la elimina si es `None`.
    pub(crate) fn new<K: AsRef<str>, V: AsRef<str>>(pairs: &[(K, Option<V>)]) -> Self {
        let mut saved = Vec::new();
        for (k, v) in pairs {
            let k = k.as_ref().to_string();
            saved.push((k.clone(), std::env::var(&k).ok()));
            // SAFETY: los llamantes de este crate toman `tests_lock`, que
            // serializa el acceso a variables de entorno entre tests.
            unsafe {
                match v {
                    Some(val) => std::env::set_var(&k, val.as_ref()),
                    None => std::env::remove_var(k),
                }
            }
        }
        Self(saved)
    }
}

#[cfg(test)]
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.0 {
            // SAFETY: ver `new`.
            unsafe {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EnvGuard, flag_enabled, is_production, tests_lock};

    #[test]
    fn production_is_detected_from_dmart_env() {
        let _l = tests_lock();
        for name in ["production", "prod", "PRODUCTION", "Production"] {
            let _g = EnvGuard::new(&[("DMART_ENV", Some(name)), ("APP_ENV", None)]);
            assert!(
                is_production(),
                "DMART_ENV={name} debe contar como producción"
            );
        }
    }

    #[test]
    fn non_production_names_are_not_production() {
        let _l = tests_lock();
        for name in ["development", "staging", "dev", "", "prod-like"] {
            let _g = EnvGuard::new(&[("DMART_ENV", Some(name)), ("APP_ENV", None)]);
            assert!(!is_production(), "DMART_ENV={name} no es producción");
        }
    }

    /// Sin variable de entorno se asume no producción, para no romper el
    /// desarrollo local ni los tests.
    #[test]
    fn unset_is_not_production() {
        let _l = tests_lock();
        let _g = EnvGuard::new(&[("DMART_ENV", None::<&str>), ("APP_ENV", None::<&str>)]);
        assert!(!is_production());
    }

    /// `APP_ENV` se acepta como alias, para despliegues que ya la usaban.
    #[test]
    fn app_env_is_accepted_as_alias() {
        let _l = tests_lock();
        let _g = EnvGuard::new(&[("DMART_ENV", None), ("APP_ENV", Some("production"))]);
        assert!(is_production());
    }

    /// `DMART_ENV` tiene prioridad: si está en producción, lo es aunque el
    /// alias diga lo contrario.
    #[test]
    fn dmart_env_wins_over_alias() {
        let _l = tests_lock();
        let _g = EnvGuard::new(&[
            ("DMART_ENV", Some("production")),
            ("APP_ENV", Some("development")),
        ]);
        assert!(is_production());
    }

    #[test]
    fn flags_accept_common_truthy_spellings() {
        let _l = tests_lock();
        for v in ["1", "true", "TRUE", "yes", "on"] {
            let _g = EnvGuard::new(&[("DMART_TEST_FLAG", Some(v))]);
            assert!(
                flag_enabled("DMART_TEST_FLAG"),
                "{v} debería activar la bandera"
            );
        }
        for v in ["0", "false", "no", "off", ""] {
            let _g = EnvGuard::new(&[("DMART_TEST_FLAG", Some(v))]);
            assert!(
                !flag_enabled("DMART_TEST_FLAG"),
                "{v} no debería activar la bandera"
            );
        }
    }
}
