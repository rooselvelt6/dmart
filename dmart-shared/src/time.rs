// Time utilities - platform agnostic
// Uses chrono on native, web-time/js-sys on WASM

#[cfg(not(target_arch = "wasm32"))]
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(target_arch = "wasm32")]
pub fn now_rfc3339() -> String {
    use js_sys::Date;
    let now = Date::now();
    let date = Date::new(&wasm_bindgen::JsValue::from_f64(now));
    date.to_iso_string().as_string().unwrap_or_else(|| {
        // Fallback if toISOString fails
        let secs = (now / 1000.0) as i64;
        let millis = (now % 1000.0) as u32;
        format!("{}.{:03}Z", chrono_fallback::format_utc(secs), millis)
    })
}

#[cfg(target_arch = "wasm32")]
pub fn now_date_string() -> String {
    use js_sys::Date;
    let date = Date::new(&wasm_bindgen::JsValue::from_f64(Date::now()));
    let year = date.get_utc_full_year();
    let month = date.get_utc_month() + 1;
    let day = date.get_utc_date();
    format!("{:04}-{:02}-{:02}", year, month, day)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_date_string() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

// Fallback for WASM when chrono is not available
#[cfg(target_arch = "wasm32")]
mod chrono_fallback {
    pub fn format_utc(secs: i64) -> String {
        // Minimal UTC formatting without chrono
        let days = secs / 86400;
        let secs_of_day = secs % 86400;
        if secs_of_day < 0 {
            return "1970-01-01T00:00:00".to_string();
        }

        let hours = secs_of_day / 3600;
        let mins = (secs_of_day % 3600) / 60;
        let secs = secs_of_day % 60;

        // Very rough date calculation (good enough for fallback)
        let year = 1970 + (days / 365) as i64;
        let day_of_year = days % 365;
        let month = (day_of_year / 30) + 1;
        let day = (day_of_year % 30) + 1;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            year, month, day, hours, mins, secs
        )
    }
}
