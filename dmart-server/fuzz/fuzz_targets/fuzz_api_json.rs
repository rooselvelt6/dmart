//! Fuzzing de endpoints API que aceptan JSON

#![no_main]

use dmart_shared::models::Patient;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(json_str) = std::str::from_utf8(data) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
            let _ = serde_json::from_value::<Patient>(value);
        }
    }
});
