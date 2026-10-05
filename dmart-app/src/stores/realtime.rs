/// Almacén de tiempo real: conexión SSE y notificaciones toast.
///
/// - `subscribe_realtime`: abre un stream SSE al backend (con `fetch` +
///   `Authorization: Bearer`; `EventSource` no permite cabeceras, así que el
///   token nunca viaja en la URL) y dispara un callback con cada evento
///   `measurement` publicado.
/// - Sistema global de toasts para avisar al usuario de nuevos scores sin
///   recargar la página.
use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{RequestCredentials, Response};

/// Evento de medición publicado por el backend (SSE `measurement`).
#[derive(Debug, Clone, Deserialize)]
pub struct MeasurementEvent {
    pub patient_id: String,
    pub measurement_id: String,
    pub apache_score: u32,
    pub gcs_score: u8,
    pub severity: String,
    pub mortality_risk: f32,
    pub timestamp: String,
}

impl MeasurementEvent {
    /// Nivel de alerta clínica basado en los scores recibidos.
    pub fn is_critical(&self) -> bool {
        self.apache_score >= 25 || self.gcs_score <= 8 || self.severity == "Crítico"
    }
}

/// Conexión SSE al backend. Devuelve una señal con el último evento.
/// Se desconecta automáticamente al desmontar el componente.
pub fn use_realtime() -> RwSignal<Option<MeasurementEvent>> {
    let event = RwSignal::new(None::<MeasurementEvent>);

    if crate::stores::session::access_token().is_none() {
        return event;
    }

    spawn_local(realtime_loop(event));
    event
}

/// Fin de una pasada del stream: `Stop` (no reintentar, sesión inválida) o
/// `Retry` (reconectar con backoff).
enum StreamResult {
    Stop,
    Retry,
}

/// Bucle de reconexión del stream SSE. Toma el access token fresco en cada
/// pasada (por si el refresh lo rotó) y se detiene si la sesión se cierra.
async fn realtime_loop(event: RwSignal<Option<MeasurementEvent>>) {
    let mut backoff_ms: u32 = 500;
    loop {
        let Some(token) = crate::stores::session::access_token() else {
            return;
        };
        match read_sse_stream(&token, &event).await {
            StreamResult::Stop => return,
            StreamResult::Retry => {
                gloo_timers::future::TimeoutFuture::new(backoff_ms).await;
                backoff_ms = (backoff_ms * 2).min(15_000);
            }
        }
    }
}

/// Lee una conexión SSE completa hasta que el servidor la cierra. Devuelve el
/// estado para el bucle de reconexión.
async fn read_sse_stream(token: &str, event: &RwSignal<Option<MeasurementEvent>>) -> StreamResult {
    let window = web_sys::window().expect("window");

    let headers = match web_sys::Headers::new() {
        Ok(h) => h,
        Err(_) => return StreamResult::Stop,
    };
    headers
        .append("Authorization", &format!("Bearer {}", token))
        .ok();
    headers.append("Accept", "text/event-stream").ok();

    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_headers(&headers);
    init.set_credentials(RequestCredentials::Include);

    let request = match web_sys::Request::new_with_str_and_init("/api/realtime/stream", &init) {
        Ok(r) => r,
        Err(_) => return StreamResult::Stop,
    };

    let resp_js = match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(v) => v,
        Err(_) => return StreamResult::Retry,
    };
    let response: Response = match resp_js.dyn_into() {
        Ok(r) => r,
        Err(_) => return StreamResult::Retry,
    };

    if response.status() == 401 || response.status() == 403 {
        // Credencial inválida: reintentar no tiene sentido.
        return StreamResult::Stop;
    }
    if response.status() != 200 {
        return StreamResult::Retry;
    }

    let body = match response.body() {
        Some(b) => b,
        None => return StreamResult::Retry,
    };
    let reader: web_sys::ReadableStreamDefaultReader = match body.get_reader().dyn_into() {
        Ok(r) => r,
        Err(_) => return StreamResult::Retry,
    };

    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let chunk = match JsFuture::from(reader.read()).await {
            Ok(v) => v,
            Err(_) => return StreamResult::Retry,
        };
        let done = js_sys::Reflect::get(&chunk, &"done".into())
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false);
        if done {
            // Cierre normal: reconectar (el servidor hace drop por keep-alive).
            break;
        }
        let value = js_sys::Reflect::get(&chunk, &"value".into()).unwrap_or(JsValue::UNDEFINED);
        if value.is_undefined() {
            continue;
        }
        let array: js_sys::Uint8Array = value.unchecked_into();
        buffer.extend_from_slice(&array.to_vec());
        drain_frames(&mut buffer, event);
        if crate::stores::session::access_token().is_none() {
            break;
        }
    }

    let _ = JsFuture::from(reader.cancel()).await;
    StreamResult::Retry
}

/// Extrae y procesa los frames SSE completos (`data: ...\n\n`) del buffer.
fn drain_frames(buffer: &mut Vec<u8>, event: &RwSignal<Option<MeasurementEvent>>) {
    while let Some(pos) = buffer.windows(2).position(|w| w == *b"\n\n") {
        let frame = String::from_utf8_lossy(&buffer[..pos]).into_owned();
        buffer.drain(..=pos + 1);
        handle_frame(&frame, event);
    }
}

/// Procesa un frame SSE: une las líneas `data:`, y si el evento es
/// `measurement`, actualiza la señal y muestra el toast.
fn handle_frame(frame: &str, event: &RwSignal<Option<MeasurementEvent>>) {
    let data: String = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return;
    }
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data)
        && let Some(event_type) = parsed.get("type").and_then(|t| t.as_str())
        && event_type == "measurement"
        && let Ok(me) = serde_json::from_value::<MeasurementEvent>(
            parsed
                .get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )
    {
        event.set(Some(me.clone()));
        show_toast(&me);
    }
}

/// Mensaje de toast global.
#[derive(Debug, Clone)]
pub struct Toast {
    pub id: String,
    pub title: String,
    pub message: String,
    pub is_critical: bool,
}

static TOASTS: std::sync::OnceLock<RwSignal<Vec<Toast>>> = std::sync::OnceLock::new();

fn toasts() -> RwSignal<Vec<Toast>> {
    *TOASTS.get_or_init(|| RwSignal::new(Vec::new()))
}

/// Crea la señal de toasts fuera de todo owner reactivo. Ver
/// `i18n::init_lang_signal`: una señal global cacheada en un `static` pero
/// creada dentro de un componente queda destruida en cuanto ese componente se
/// desmonta, y el `spawn_local` del `TimeoutFuture` (6 s después) hace panic.
pub fn init_toasts_signal() {
    let _ = toasts();
}

/// Muestra una notificación (toast) en la esquina inferior derecha.
pub fn show_toast(me: &MeasurementEvent) {
    let critical = me.is_critical();
    let msg = format!(
        "APACHE {} · GCS {}/15 · mortalidad {:.0}%",
        me.apache_score, me.gcs_score, me.mortality_risk
    );
    let toast = Toast {
        id: uuid::Uuid::new_v4().to_string(),
        title: if critical {
            "⚠️ Deterioro clínico crítico".to_string()
        } else {
            "Actualización de score".to_string()
        },
        message: msg,
        is_critical: critical,
    };
    toasts().update(|t| {
        t.push(toast.clone());
        if t.len() > 4 {
            t.remove(0);
        }
    });
    spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(6_000).await;
        toasts().update(|t| t.retain(|item| item.id != toast.id));
    });
}

/// Observed toast container (rendered once, at the app root).
#[component]
pub fn ToastContainer() -> impl IntoView {
    let toasts = toasts();

    view! {
        <div
            aria-live="polite"
            role="status"
            style="position:fixed; bottom:20px; right:20px; z-index:9999; display:flex; flex-direction:column; gap:10px; max-width:360px;"
        >
            <For
                each=move || toasts.get()
                key=|t| t.id.clone()
                children=move |t| {
                    view! {
                        <div
                            style=move || format!(
                                "background:var(--uci-surface); border:1px solid {}; border-left:5px solid {}; border-radius:10px; padding:12px 16px; box-shadow:0 8px 24px rgba(0,0,0,0.25);",
                                if t.is_critical { "#EF4444" } else { "#10B981" },
                                if t.is_critical { "#EF4444" } else { "#10B981" },
                            )
                        >
                            <div style="font-weight:700; font-size:13px; color:var(--uci-text);">{t.title.clone()}</div>
                            <div style="font-size:12px; color:var(--uci-muted); margin-top:2px;">{t.message.clone()}</div>
                        </div>
                    }
                }
            />
        </div>
    }
}
