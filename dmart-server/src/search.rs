//! Búsqueda de pacientes por coincidencia parcial y tolerante a erratas.
//!
//! # Por qué no es un índice invertido en claro
//!
//! SPEC-052 cifra la PHI en reposo (AES-256-GCM) y por eso la búsqueda usa
//! *índices ciegos*: HMAC-SHA256 del valor exacto. Un índice de texto en claro
//! (Tantivy/Lucene) escribiría los nombres de los pacientes sin cifrar en disco
//! y desharía esa garantía, así que aquí no se indexa nada legible: se indexan
//! **HMAC de trigramas** y el resto del trabajo se hace en memoria.
//!
//! # Cómo funciona
//!
//! 1. Al sellar un paciente se guardan en `bi_tng` los HMAC de los trigramas de
//!    su nombre completo normalizado (`nombre apellido`).
//! 2. Una consulta se normaliza igual, se saca su trigrama inicial y se
//!    buscan las filas que lo contengan, lo que reduce el conjunto de candidatos
//!    a una fracción antes de descifrar nada.
//! 3. Los candidatos se descifran y se **puntúan en Rust** con
//!    Jaro-Winkler + bonificaciones por prefijo y coincidencia exacta.
//!
//! La PHI nunca sale del sobre cifrado: los trigramas son ciegos y sólo se
//! comparan con HMAC, igual que el resto de índices.
//!
//! # Límite conocido
//!
//! Con menos de [`TRIGRAM_LEN`] caracteres no hay trigrama, así que la búsqueda
//! parcial exige 3 caracteres o más. Por debajo de eso se cae al camino
//! exacto (`bi_nombre`/`bi_hc`/`bi_ced`), que sí encuentra historias clínicas
//! y cédulas completas. Está documentado en el endpoint para no Sorprender.

/// Longitud del n-grama. Tres es el mínimo útil para que la intersección
/// discrimine: con bigramas "ma" aparecería en casi todos los nombres.
pub const TRIGRAM_LEN: usize = 3;

/// Cuántos candidatos se traen de la base antes de puntuar en memoria.
///
/// El ranking es exacto (se calculan todos los candidatos), pero la búsqueda
/// es *top-k*: se limita para que un trigrama muy común ("ero", "ado") no
/// obligue a descifrar medio dataset. Los IDs y las columnas en claro viajan
/// en la fila, así que el descifrado sólo ocurre sobre este recorte.
pub const MAX_CANDIDATOS: usize = 500;

/// Normaliza texto para búsqueda: minúsculas, sin acentos, sin signos de
/// puntuación y con espacios colapsados a uno solo.
///
/// A diferencia de [`crate::crypto::normalize_for_index`] (que **conserva** los
/// acentos porque forman parte del identificador), aquí se pliegan: el usuario
/// escribe "jose" y tiene que encontrar a "José".
pub fn normalize_search(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if ch.is_whitespace() || matches!(ch, '-' | '_' | '.' | '/' | ',' | '\'' | '\u{2019}') {
            // Los separadores se tratan como frontera de palabra, no se tiran:
            // "Gustavo Ortiz" debe generar trigramas distintos por cada nombre.
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        for lower in ch.to_lowercase() {
            match strip_accent(lower) {
                Some(base) => out.push(base),
                // Si al quitar el acento desaparece el carácter entero (ø, đ,
                // ł...), se conserva el original: perderlo haría que dos
                // nombres distintos colisionaran en el mismo índice.
                None => out.push(lower),
            }
        }
    }
    out
}

/// Devuelve la letra base de un carácter acentuado, o `None` si no lo tiene.
///
/// Tabla explícita en vez de descomposición NFD (`unicode-normalization`): son
/// unos pocos caracteres del español y del latín americano, y añadir una
/// dependencia por eso no compensa. Cubre á é í ó ú ü ñ ç ý y sus
/// mayúsculas.
fn strip_accent(ch: char) -> Option<char> {
    Some(match ch {
        'á' | 'Á' => 'a',
        'é' | 'É' => 'e',
        'í' | 'Í' | 'ı' | 'İ' => 'i',
        'ó' | 'Ó' => 'o',
        'ú' | 'Ú' => 'u',
        'ü' | 'Ü' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        'ý' | 'Ý' => 'y',
        _ => return None,
    })
}

/// Trigramas de un texto normalizado, con relleno de espacios en los extremos.
///
/// El relleno hace que "gust" genere `[" gu", "gus", "ust", "st "]`: sin él, el
/// trigrama inicial del nombre ("gus") no existiría en el índice y buscar por
/// las primeras letras fallaría justo en el caso más común.
pub fn trigrams(normalized: &str) -> Vec<String> {
    if normalized.is_empty() {
        return Vec::new();
    }
    let padded = format!(" {normalized} ");
    let chars: Vec<char> = padded.chars().collect();
    if chars.len() <= TRIGRAM_LEN {
        return vec![padded];
    }
    chars
        .windows(TRIGRAM_LEN)
        .map(|w| w.iter().collect())
        .collect()
}

/// Trigrama de la consulta que se usa como filtro en SQL.
///
/// Sólo **uno**, y es el primero. Exigir todos los trigramas de la consulta
/// parece más selectivo, pero rompe justo con lo que la búsqueda debe tolerar:
///
/// - Una errata crea un trigrama que no existe en ningún nombre. "gustxavo"
///   exige `stx`, y el `AND` devuelve cero aunque "gustavo" esté a una
///   pulsación de distancia.
/// - Los trigramas de borde sólo existen si la consulta termina en un límite de
///   palabra, así que "mar" exigiría `"ar "`, que "marcelo rios" no tiene.
///
/// Un único trigrama es peor discriminante pero siempre presente cuando la
/// consulta es correcta, así que deja pasar a todos los candidatos válidos. La
/// precisión la pone el ranking en Rust, que además descarta por debajo de
/// [`PUNTUACION_MINIMA`].
///
/// El coste es traer más filas al descifrado. Lo acota
/// [`MAX_CANDIDATOS`], y el trigrama se elige **del inicio** de la consulta
/// porque lo que el usuario teclea primero es la parte más selectiva
/// ("ort" para "ortiz" descarta más que "tri").
pub fn trigram_filtro(normalizado: &str) -> Option<String> {
    let ventana: String = normalizado.chars().take(TRIGRAM_LEN).collect();
    if ventana.chars().count() < TRIGRAM_LEN {
        return None;
    }
    Some(ventana)
}

/// Puntuación mínima para que un candidato se devuelva.
///
/// El filtro por trigramas entra a ~~candidatos~~ que pueden no parecerse nada
/// ("zaf" entra por el trigrama `zaf`… salvo que exista, pero "ero" sí existe en
/// muchos nombres). El ranking es lo que decide, y este umbral impide que un
/// trigrama genérico devuelva media tabla como si fueran coincidencias.
///
/// 0.55 descarta claramente lo que no se parece ("son" vs "wilmar") y conserva
/// lo que una errata de tecleo produce (Jaro-Winkler de una transposition en
/// un nombre de 7 letras queda en torno a 0.8).
pub const PUNTUACION_MINIMA: f64 = 0.55;

/// Jaro-Winkler: similitud 0..=1 con bonificación por prefijo común.
///
/// Se prefiere a la distancia de edición porque no penaliza de más los nombres
/// cortos y da más peso al inicio de la cadena, que es donde coincide el
/// prefijo que el usuario está escribiendo.
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    let x: Vec<char> = a.chars().collect();
    let y: Vec<char> = b.chars().collect();
    if x.is_empty() && y.is_empty() {
        return 1.0;
    }
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    let jaro = jaro(&x, &y);
    // Winkler: hasta 4 caracteres de prefijo,escalados por 0.1 cada uno.
    let prefix = x
        .iter()
        .zip(y.iter())
        .take(4)
        .take_while(|(p, q)| p == q)
        .count();
    jaro + (0.1 * prefix as f64 * (1.0 - jaro))
}

fn jaro(x: &[char], y: &[char]) -> f64 {
    let max_dist = (x.len().max(y.len()) / 2).saturating_sub(1);
    let mut x_flags = vec![false; x.len()];
    let mut y_flags = vec![false; y.len()];
    let mut matches = 0usize;

    for (i, xc) in x.iter().enumerate() {
        let lo = i.saturating_sub(max_dist);
        let hi = (i + max_dist + 1).min(y.len());
        for j in lo..hi {
            if !y_flags[j] && y[j] == *xc {
                x_flags[i] = true;
                y_flags[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }

    let mut transpositions = 0usize;
    let mut k = 0usize;
    for i in 0..x.len() {
        if !x_flags[i] {
            continue;
        }
        while !y_flags[k] {
            k += 1;
        }
        if x[i] != y[k] {
            transpositions += 1;
        }
        k += 1;
    }

    let m = matches as f64;
    (m / x.len() as f64 + m / y.len() as f64 + (m - transpositions as f64 / 2.0) / m) / 3.0
}

/// Identificadores que una búsqueda debe tratar como coincidencia literal.
///
/// Cédula e historia clínica son códigos: se comparan normalizados pero sin
/// tolerar erratas, porque "HC-100516" y "HC-100S16" no son el mismo paciente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Nombre,
    Identificador,
}

/// Puntúa un candidato contra la consulta. Mayor es mejor.
///
/// El valor **no está acotado a 1**: las bonificaciones se suman sin saturar.
/// Con `clamp(1.0)`, un candidato que empieza por la consulta y otro que sólo la
/// contiene empataban ambos en 1.0 y el ranking perdía el orden, que es
/// justo lo que ordena.
///
/// `nombre` y `apellido` se puntúan por separado además del nombre completo,
/// porque buscar "Ortiz" no debe penalizarse por un nombre de pila corto.
pub fn puntuar(campo: Campo, consulta: &str, nombre: &str, apellido: &str) -> f64 {
    if consulta.is_empty() {
        return 0.0;
    }
    let completo = format!("{nombre} {apellido}");
    let completa_norm = normalize_search(&completo);
    let nombre_norm = normalize_search(nombre);
    let apellido_norm = normalize_search(apellido);
    let c = normalize_search(consulta);

    match campo {
        Campo::Identificador => {
            if completa_norm == c || nombre_norm == c || apellido_norm == c {
                1.0
            } else if completa_norm.contains(&c) {
                0.9
            } else {
                0.0
            }
        }
        Campo::Nombre => {
            let mut mejor = 0.0_f64;
            // El nombre de pila pesa algo más que el apellido: en la práctica
            // se busca a la persona por su nombre, y sin este desempate
            // "mar" empataría a "Marcelo Ríos" con "Carla Márquez" (ambos
            // empiezan por "mar") y el orden dependería de `created_at`.
            for (candidato, peso) in [
                (&completa_norm, 1.0),
                (&nombre_norm, 1.05),
                (&apellido_norm, 1.0),
            ] {
                if candidato.is_empty() {
                    continue;
                }
                let mut s = jaro_winkler(&c, candidato);
                // Prefijo: lo que el usuario está tecleando. Es la señal más
                // fuerte para "pocas letras", y pesa más que la simple
                // subcadena para que "gust" gane a "ust" sobre el mismo nombre.
                if candidato.starts_with(&c) {
                    s += 0.35;
                } else if candidato.contains(&c) {
                    s += 0.15;
                }
                // Token completo: "gustavo ortiz" debe ganar a "gustavo".
                if candidato.split(' ').any(|t| t == c) {
                    s += 0.25;
                }
                mejor = mejor.max(s * peso);
            }
            mejor
        }
    }
}

/// Extrae el token de consulta cuando la búsqueda parece un identificador.
///
/// "HC-100516" o "V-11830897" llevan prefijo reconocible; un nombre no.
pub fn parece_identificador(consulta: &str) -> bool {
    let q = consulta.trim();
    if q.is_empty() {
        return false;
    }
    let upper = q.to_uppercase();
    upper.starts_with("HC-")
        || upper.starts_with("V-")
        || upper.starts_with("E-")
        || q.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_quita_acentos_y_puntuacion() {
        assert_eq!(normalize_search("José"), "jose");
        assert_eq!(
            normalize_search("  María   del  Carmen "),
            "maria del carmen"
        );
        assert_eq!(normalize_search("Muñoz-O'Neill"), "munoz o neill");
        assert_eq!(normalize_search("Ñoño"), "nono");
    }

    #[test]
    fn normalize_conserva_letras_sin_acento_mapeable() {
        // Si al plegar desaparece el carácter entero, se conserva el original
        // en vez de perderlo (que haría colisionar dos nombres distintos).
        assert_eq!(normalize_search("Ø").chars().count(), 1);
        assert_eq!(normalize_search("Łukasz"), "łukasz");
    }

    #[test]
    fn trigrams_incluye_el_prefijo_por_el_relleno() {
        let t = trigrams("gust");
        assert!(t.contains(&" gu".to_string()));
        assert!(t.contains(&"gus".to_string()));
        assert!(t.contains(&"ust".to_string()));
        assert!(t.contains(&"st ".to_string()));
        assert_eq!(t.len(), 4);
    }

    #[test]
    fn trigrams_de_texto_corto_devuelve_el_fragmento() {
        // Con relleno, "ab" sí produce trigramas (ambos de borde), que es lo
        // que hace buscable el inicio y el final del nombre.
        assert_eq!(trigrams("ab"), vec![" ab", "ab "]);
        assert!(trigrams("").is_empty());
    }

    #[test]
    fn el_trigrama_de_filtro_es_el_inicial_y_sin_relleno() {
        assert_eq!(trigram_filtro("gus"), Some("gus".into()));
        // Sin relleno: "or " sólo existiría en la fila si la consulta terminara
        // en un límite de palabra.
        assert_eq!(trigram_filtro("ortiz"), Some("ort".into()));
        assert_eq!(trigram_filtro("or"), None);
        assert_eq!(trigram_filtro(""), None);
    }

    #[test]
    fn jaro_winkler_premia_el_prefijo() {
        let prefijo = jaro_winkler("gustavo", "gustavo ortiz");
        let medio = jaro_winkler("gustavo", "ortiz gustavo");
        assert!(prefijo > medio, "{prefijo} should beat {medio}");
        assert!(jaro_winkler("gustavo", "gustavo") > 0.99);
    }

    #[test]
    fn prefijo_gana_a_similitud_dispersa() {
        let por_prefijo = puntuar(Campo::Nombre, "gust", "Gustavo", "Ortiz");
        let por_medio = puntuar(Campo::Nombre, "ust", "Gustavo", "Ortiz");
        assert!(por_prefijo > por_medio);
    }

    #[test]
    fn apellido_buscado_encuentra_al_paciente() {
        // El fallo original: el apellido no estaba indexado.
        let s = puntuar(Campo::Nombre, "ortiz", "Gustavo", "Ortiz");
        assert!(s > 0.5, "apellido no puntuó: {s}");
    }

    #[test]
    fn tolera_una_errata() {
        let bueno = puntuar(Campo::Nombre, "gustavo", "Gustavo", "Ortiz");
        let errata = puntuar(Campo::Nombre, "gustavo", "Gustabo", "Ortiz");
        assert!(
            errata > 0.7,
            "una errata no debe hundir el resultado: {errata}"
        );
        assert!(errata < bueno);
    }

    #[test]
    fn acento_no_impide_encontrar() {
        assert!(puntuar(Campo::Nombre, "jose", "José", "Pérez") > 0.5);
        assert!(puntuar(Campo::Nombre, "pérez", "José", "Pérez") > 0.5);
    }

    #[test]
    fn identificadores_son_exactos() {
        assert_eq!(parece_identificador("HC-100516"), true);
        assert_eq!(parece_identificador("V-11830897"), true);
        assert_eq!(parece_identificador("100516"), true);
        assert_eq!(parece_identificador("gustavo"), false);
        assert_eq!(parece_identificador(""), false);
    }

    #[test]
    fn identificador_no_puntua_por_similitud() {
        // HC-100516 frente a HC-100S16: mismo prefijo, otro paciente.
        assert_eq!(
            puntuar(Campo::Identificador, "hc100516", "", "hc-100s16"),
            0.0
        );
    }
}
