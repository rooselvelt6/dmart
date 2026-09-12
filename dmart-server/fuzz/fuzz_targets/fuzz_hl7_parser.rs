//! Fuzzing del parser HL7 v2

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = dmart_server::hl7::parser::parse_oru_message(s);
    }
});
