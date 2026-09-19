/// Almacén de tiempo real: conexión SSE (`EventSource`) y notificaciones toast.
///
/// - `subscribe_realtime`: mantiene un `EventSource` al backend y dispara un
///   callback con cada evento `measurement` publicado.
/// - Sistema global de toasts para avisar al usuario de nuevos scores sin
///   recargar la página.
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

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

    let Some(token) = LocalStorage::get::<String>("dmart_auth").ok() else {
        return event;
    };

    let source = web_sys::EventSource::new(&format!("/api/realtime/stream?token={}", token))
        .expect("EventSource creation failed");

    let onmessage = Closure::wrap(Box::new(move |ev: web_sys::MessageEvent| {
        if let Some(data) = ev.data().as_string()
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data)
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
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    source.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

    // Mantener el closure y el source vivos mientras la app esté viva.
    std::mem::forget(onmessage);
    std::mem::forget(source);

    event
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
