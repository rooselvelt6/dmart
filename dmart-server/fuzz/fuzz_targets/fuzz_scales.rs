//! Fuzzing de cálculo de escalas clínicas

#![no_main]

use dmart_shared::models::ApacheIIData;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data)
        && let Ok(apache) = serde_json::from_str::<ApacheIIData>(s)
    {
        let _ = dmart_shared::scales::calculate_apache_ii_score(&apache);
    }
});
