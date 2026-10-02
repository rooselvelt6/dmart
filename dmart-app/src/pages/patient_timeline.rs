use leptos::prelude::*;
use leptos_router::hooks::*;

use crate::api;
use crate::api::TimelineEvent;
use crate::components::ui_kit::{ErrorState, LoadingState};

fn fmt_ts(ms: i64) -> String {
    if ms <= 0 {
        return "—".to_string();
    }
    match chrono::DateTime::from_timestamp(ms / 1000, 0) {
        Some(d) => d.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => "—".to_string(),
    }
}

/// Icono + color por tipo de evento (serde `PascalCase` en el servidor).
fn event_meta(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "Admission" => ("fa-door-open", "#10B981", "Ingreso"),
        "VitalSigns" => ("fa-heart-pulse", "#EF4444", "Signos vitales"),
        "ScoreCalculated" => ("fa-calculator", "#3B82F6", "Score calculado"),
        "Intervention" => ("fa-hand-holding-medical", "#8B5CF6", "Intervención"),
        "Medication" => ("fa-pills", "#14B8A6", "Medicamento"),
        "Procedure" => ("fa-stethoscope", "#0EA5E9", "Procedimiento"),
        "Note" => ("fa-note-sticky", "#64748B", "Nota"),
        "Discharge" => ("fa-door-open", "#6B7280", "Egreso"),
        "Alert" => ("fa-triangle-exclamation", "#F59E0B", "Alerta"),
        "CdsAction" => ("fa-clipboard-list", "#EC4899", "Acción CDS"),
        _ => ("fa-circle", "#94A3B8", "Evento"),
    }
}

fn severity_meta(sev: &str) -> (&'static str, &'static str) {
    match sev {
        "critical" => ("#DC2626", "Crítica"),
        "warning" => ("#F59E0B", "Advertencia"),
        _ => ("#64748B", "Informativa"),
    }
}

/// Resumen legible del `payload` sin AssumeShape: el servidor guarda un JSON
/// libre por tipo de evento (SPEC-015).
fn payload_summary(payload: &serde_json::Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(obj) = payload.as_object() else {
        if let Some(s) = payload.as_str() {
            out.push(("detalle".to_string(), s.to_string()));
        }
        return out;
    };
    // Orden estable: primero los scores, luego el resto.
    let preferred = [
        "apache",
        "gcs",
        "news2",
        "sofa",
        "saps3",
        "apache_ii",
        "mortality_risk",
        "las",
        "los",
    ];
    for key in preferred {
        if let Some(v) = obj.get(key).and_then(|v| v.as_f64()) {
            out.push((key.to_string(), fmt_num(v)));
        }
    }
    for (k, v) in obj {
        if preferred.contains(&k.as_str()) {
            continue;
        }
        let text = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => continue,
            other => other.to_string(),
        };
        if text.len() > 120 {
            continue;
        }
        out.push((k.clone(), text));
        if out.len() >= 10 {
            break;
        }
    }
    out
}

fn fmt_num(v: f64) -> String {
    if v.fract().abs() < 0.005 {
        format!("{}", v as i64)
    } else {
        format!("{:.2}", v)
    }
}

#[component]
pub fn PatientTimelinePage() -> impl IntoView {
    let params = use_params_map();
    let pid = move || params.get().get("id").unwrap_or_default();

    let filter_type = RwSignal::new(String::new());
    let filter_sev = RwSignal::new(String::new());
    let reload = RwSignal::new(0u32);

    let events = LocalResource::new(move || {
        let _ = reload.get();
        let p = pid();
        async move {
            api::patient_timeline(
                &p,
                Some(filter_type.get().as_str()),
                Some(filter_sev.get().as_str()),
                None,
                None,
                None,
                Some(100),
            )
            .await
        }
    });

    let apply_filters = move |_| reload.update(|n| *n += 1);

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-start justify-between gap-4 mb-6">
                <div>
                    <a
                        href=move || format!("/patients/{}", pid())
                        class="inline-block mb-2 text-sm"
                        style="color:var(--uci-muted); text-decoration:none;"
                    >
                        <i class="fa-solid fa-arrow-left mr-1"></i>"Volver al detalle"
                    </a>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-clock-rotate-left mr-2"></i>"Timeline del paciente"
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        "Historia clínica append-only con fingerprint SHA-256 por evento (SPEC-015)"
                    </p>
                </div>
                <button
                    class="px-4 h-10 text-sm rounded-lg"
                    style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                    on:click=apply_filters
                >
                    <i class="fa-solid fa-rotate mr-2"></i>"Actualizar"
                </button>
            </div>

            <div class="glass-card p-4 mb-5">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <div>
                        <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">
                            "Tipo de evento"
                        </label>
                        <select
                            class="form-select"
                            style="padding-top:8px; padding-bottom:8px; font-size:14px;"
                            prop:value=move || filter_type.get()
                            on:change=move |ev| {
                                filter_type.set(event_target_value(&ev));
                                reload.update(|n| *n += 1);
                            }
                        >
                            <option value="">"Todos"</option>
                            <option value="Admission">"Ingreso"</option>
                            <option value="VitalSigns">"Signos vitales"</option>
                            <option value="ScoreCalculated">"Score calculado"</option>
                            <option value="Intervention">"Intervención"</option>
                            <option value="Medication">"Medicamento"</option>
                            <option value="Procedure">"Procedimiento"</option>
                            <option value="Note">"Nota"</option>
                            <option value="Discharge">"Egreso"</option>
                            <option value="Alert">"Alerta"</option>
                            <option value="CdsAction">"Acción CDS"</option>
                        </select>
                    </div>
                    <div>
                        <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">
                            "Severidad"
                        </label>
                        <select
                            class="form-select"
                            style="padding-top:8px; padding-bottom:8px; font-size:14px;"
                            prop:value=move || filter_sev.get()
                            on:change=move |ev| {
                                filter_sev.set(event_target_value(&ev));
                                reload.update(|n| *n += 1);
                            }
                        >
                            <option value="">"Todas"</option>
                            <option value="info">"Informativa"</option>
                            <option value="warning">"Advertencia"</option>
                            <option value="critical">"Crítica"</option>
                        </select>
                    </div>
                </div>
            </div>

            <Suspense fallback=move || view! { <LoadingState label="Cargando timeline..." /> }>
                {move || match events.get() {
                    Some(Ok(resp)) => {
                        if resp.events.is_empty() {
                            view! {
                                <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                                    <i class="fa-solid fa-inbox text-2xl mb-2" style="color:#94A3B8;"></i>
                                    <p>"Sin eventos para los filtros seleccionados"</p>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <TimelineList events=resp.events has_more=resp.has_more />
                            }.into_any()
                        }
                    }
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo cargar el timeline: {}", e)
                            on_retry=Some(Callback::new(move |()| reload.update(|n| *n += 1)))
                        />
                    }
                    .into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn TimelineList(events: Vec<TimelineEvent>, has_more: bool) -> impl IntoView {
    let list = StoredValue::new(events);
    view! {
        <div class="relative pl-6">
            <div
                class="absolute left-[7px] top-2 bottom-2 w-px"
                style="background:var(--uci-border);"
            ></div>
            <For each=move || list.get_value().clone() key=|e| e.fingerprint.clone() let:event>
                {
                    let (icon, color, label) = event_meta(&event.event_type);
                    let (sev_color, sev_label) = severity_meta(&event.severity);
                    let summary = payload_summary(&event.payload);
                    let has_summary = !summary.is_empty();
                    let summary = StoredValue::new(summary);
                    let fp = StoredValue::new(event.fingerprint.chars().take(16).collect::<String>());
                    let source = event.source.clone();
                    view! {
                        <div class="relative mb-3">
                            <span
                                class="absolute -left-[22px] top-3 flex items-center justify-center rounded-full"
                                style=format!(
                                    "width:16px; height:16px; background:{color}; box-shadow:0 0 0 3px var(--uci-bg);"
                                )
                            ></span>
                            <div class="glass-card p-4">
                                <div class="flex flex-wrap items-center gap-2 mb-2">
                                    <span
                                        class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase"
                                        style=format!(
                                            "background:color-mix(in srgb, {color} 16%, transparent); color:{color}; border:1px solid color-mix(in srgb, {color} 35%, transparent);"
                                        )
                                    >
                                        <i class=format!("fa-solid {icon} text-[9px]")></i>
                                        {label}
                                    </span>
                                    <span
                                        class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-bold uppercase"
                                        style=format!(
                                            "background:color-mix(in srgb, {sev_color} 14%, transparent); color:{sev_color};"
                                        )
                                    >
                                        {sev_label}
                                    </span>
                                    <span class="text-xs ml-auto tabular-nums" style="color:var(--uci-muted);">
                                        {fmt_ts(event.occurred_at)}
                                    </span>
                                </div>
                                <Show
                                    when=move || has_summary
                                    fallback=|| view! {
                                        <p class="text-xs" style="color:var(--uci-muted);">
                                            "Sin payload"
                                        </p>
                                    }
                                >
                                    <dl class="grid grid-cols-2 md:grid-cols-3 gap-x-4 gap-y-1">
                                        <For
                                            each=move || summary.get_value()
                                            key=|(k, _)| k.clone()
                                            let:kv
                                        >
                                            <div class="flex gap-2 min-w-0">
                                                <dt class="text-[11px] uppercase shrink-0" style="color:var(--uci-muted);">
                                                    {kv.0.clone()}
                                                </dt>
                                                <dd
                                                    class="text-[11px] font-semibold truncate"
                                                    style="color:var(--uci-text);"
                                                >
                                                    {kv.1.clone()}
                                                </dd>
                                            </div>
                                        </For>
                                    </dl>
                                </Show>
                                <div
                                    class="flex items-center gap-2 mt-3 pt-2 text-[10px]"
                                    style="border-top:1px solid var(--uci-border); color:var(--uci-muted);"
                                >
                                    <i class="fa-solid fa-fingerprint"></i>
                                    <span class="truncate" style="font-family:var(--uci-font-mono);">
                                        {fp.get_value()}
                                        "..."
                                    </span>
                                    <span class="ml-auto shrink-0">{source.clone()}</span>
                                </div>
                            </div>
                        </div>
                    }
                }
            </For>
            <Show when=move || has_more>
                <p class="text-xs pl-1" style="color:var(--uci-muted);">
                    <i class="fa-solid fa-circle-info mr-1"></i>
                    "Hay más eventos. Ajusta los filtros o reduce el límite para acotar."
                </p>
            </Show>
        </div>
    }
}
