use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::api::{Diagnostic, SupportEvent, SupportSystem};
use crate::components::ui_kit::{ErrorState, LoadingState};

/// Acciones del runbook expuestas por el backend (`support::ACTIONS`).
const ACTIONS: &[(&str, &str, &str)] = &[
    ("ingest_retry", "Reintentar ingest", "fa-rotate-right"),
    ("circuit_reset", "Reiniciar circuit breakers", "fa-bolt"),
    ("backup", "Ejecutar backup", "fa-database"),
    (
        "audit_retention",
        "Aplicar retención de auditoría",
        "fa-clock-rotate-left",
    ),
    ("model_swap", "Reactivar modelo ML", "fa-microchip"),
    (
        "verify_fingerprints",
        "Verificar fingerprints",
        "fa-fingerprint",
    ),
];

fn action_label(action: &str) -> &'static str {
    ACTIONS
        .iter()
        .find(|(key, _, _)| *key == action)
        .map(|(_, label, _)| *label)
        .unwrap_or("Acción")
}

fn system_label(key: &str) -> &'static str {
    match key {
        "db" => "Base de datos",
        "ingest" => "Ingest MLLP/HL7",
        "realtime" => "Realtime SSE",
        "ml" => "Serving ML",
        "monitores" => "Monitores",
        "audit" => "Auditoría",
        "backup" => "Backup",
        _ => "Desconocido",
    }
}

fn status_meta(status: &str) -> (&'static str, &'static str) {
    match status {
        "ok" => ("#10B981", "OK"),
        "degraded" => ("#F59E0B", "Degradado"),
        "error" => ("#EF4444", "Error"),
        other => ("#6B7280", other),
    }
}

fn fmt_opt_secs(secs: Option<f64>) -> String {
    match secs {
        Some(s) if s < 60.0 => format!("hace {s:.0} s"),
        Some(s) if s < 3600.0 => format!("hace {:.0} min", s / 60.0),
        Some(s) => format!("hace {:.1} h", s / 3600.0),
        None => "—".to_string(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Systems,
    Diagnostics,
    History,
}

#[component]
pub fn SupportConsole() -> impl IntoView {
    let refresh = RwSignal::new(0u32);
    let active_tab = RwSignal::new(Tab::Systems);
    let model = RwSignal::new("ews".to_string());
    let version = RwSignal::new(String::new());
    let pending = RwSignal::new(Option::<String>::None);
    let running = RwSignal::new(false);
    let feedback = RwSignal::new(Option::<(bool, String)>::None);

    let systems = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::get_support_systems().await }
    });
    let diagnostics = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::get_support_diagnostics().await }
    });
    let history = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::get_support_history(50).await }
    });

    let confirm = Callback::new(move |action: String| pending.set(Some(action)));

    let execute = Callback::new(move |action: String| {
        running.set(true);
        feedback.set(None);
        let m = model.get();
        let v = version.get();
        spawn_local(async move {
            let result = match api::run_support_action(&action, &m, &v).await {
                Ok(r) => (r.success, r.message),
                Err(e) => (false, e),
            };
            feedback.set(Some(result));
            running.set(false);
            pending.set(None);
            refresh.update(|n| *n += 1);
        });
    });

    let tab_class = move |tab: Tab| {
        if active_tab.get() == tab {
            "px-4 py-2 text-sm font-semibold rounded-lg"
        } else {
            "px-4 py-2 text-sm rounded-lg"
        }
    };

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-center justify-between gap-4 mb-6">
                <div>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-screwdriver-wrench mr-2"></i>"Consola de Soporte"
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        "Estado, diagnóstico y runbook de la plataforma UCI"
                    </p>
                </div>
                <button
                    class="btn-primary px-4 h-10 text-sm"
                    on:click=move |_| refresh.update(|n| *n += 1)
                    disabled=move || running.get()
                >
                    <i class="fa-solid fa-rotate mr-2"></i>"Actualizar"
                </button>
            </div>

            {move || feedback.get().map(|(ok, msg)| {
                let color = if ok { "#10B981" } else { "#EF4444" };
                let icon = if ok { "fa-circle-check" } else { "fa-circle-exclamation" };
                view! {
                    <div
                        class="flex items-start gap-3 p-4 rounded-xl mb-4"
                        style=format!("background:{color}1A; border:1px solid {color}66; color:var(--uci-text);")
                        role="status"
                    >
                        <i class=format!("fa-solid {icon} mt-0.5") style=format!("color:{color};")></i>
                        <span class="text-sm">{msg}</span>
                    </div>
                }
            })}

            <div class="flex flex-wrap gap-2 mb-4">
                <button
                    class=move || tab_class(Tab::Systems)
                    style=move || if active_tab.get() == Tab::Systems { "background:var(--uci-accent); color:white;" } else { "background:var(--uci-surface); color:var(--uci-text);" }
                    on:click=move |_| active_tab.set(Tab::Systems)
                >
                    <i class="fa-solid fa-server mr-2"></i>"Sistemas"
                </button>
                <button
                    class=move || tab_class(Tab::Diagnostics)
                    style=move || if active_tab.get() == Tab::Diagnostics { "background:var(--uci-accent); color:white;" } else { "background:var(--uci-surface); color:var(--uci-text);" }
                    on:click=move |_| active_tab.set(Tab::Diagnostics)
                >
                    <i class="fa-solid fa-stethoscope mr-2"></i>"Diagnóstico"
                </button>
                <button
                    class=move || tab_class(Tab::History)
                    style=move || if active_tab.get() == Tab::History { "background:var(--uci-accent); color:white;" } else { "background:var(--uci-surface); color:var(--uci-text);" }
                    on:click=move |_| active_tab.set(Tab::History)
                >
                    <i class="fa-solid fa-clock-rotate-left mr-2"></i>"Historial"
                </button>
            </div>

            <div class="mb-6">
                {move || match active_tab.get() {
                    Tab::Systems => view! {
                        <Suspense fallback=move || view! { <LoadingState label="Consultando subsistemas..." /> }>
                            {move || match systems.get() {
                                Some(Ok(list)) => view! { <SystemsTable systems=list refresh=refresh /> }.into_any(),
                                Some(Err(e)) => view! {
                                    <ErrorState
                                        message=format!("No se pudo consultar los subsistemas: {}", e)
                                        on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                                    />
                                }.into_any(),
                                None => view! {}.into_any(),
                            }}
                        </Suspense>
                    }.into_any(),
                    Tab::Diagnostics => view! {
                        <Suspense fallback=move || view! { <LoadingState label="Generando diagnóstico..." /> }>
                            {move || match diagnostics.get() {
                                Some(Ok(list)) => view! { <DiagnosticsList diagnostics=list /> }.into_any(),
                                Some(Err(e)) => view! {
                                    <ErrorState
                                        message=format!("No se pudo generar el diagnóstico: {}", e)
                                        on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                                    />
                                }.into_any(),
                                None => view! {}.into_any(),
                            }}
                        </Suspense>
                    }.into_any(),
                    Tab::History => view! {
                        <Suspense fallback=move || view! { <LoadingState label="Cargando historial..." /> }>
                            {move || match history.get() {
                                Some(Ok(events)) => view! { <HistoryTable events=events /> }.into_any(),
                                Some(Err(e)) => view! {
                                    <ErrorState
                                        message=format!("No se pudo cargar el historial: {}", e)
                                        on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                                    />
                                }.into_any(),
                                None => view! {}.into_any(),
                            }}
                        </Suspense>
                    }.into_any(),
                }}
            </div>

            <ActionsPanel
                model=model
                version=version
                running=running
                on_action=confirm
            />
        </div>

        {move || pending.get().map(|action| {
            let act = action.clone();
            let on_exec = execute;
            view! {
                <div
                    style="position:fixed; inset:0; z-index:50; display:flex; align-items:center; justify-content:center; background:rgba(0,0,0,0.55);"
                    role="dialog"
                    aria-modal="true"
                    aria-label="Confirmar acción de soporte"
                >
                    <div class="glass-card" style="max-width:440px; width:90%; padding:24px;">
                        <h3 class="text-lg font-bold mb-2" style="color:var(--uci-text);">
                            <i class="fa-solid fa-triangle-exclamation mr-2" style="color:#F59E0B;"></i>
                            "Confirmar acción"
                        </h3>
                        <p class="text-sm mb-1" style="color:var(--uci-text);">
                            {action_label(&act)}
                        </p>
                        <p class="text-xs mb-6" style="color:var(--uci-muted);">
                            "Se ejecutará en el servidor y quedará registrado en el historial y la auditoría."
                        </p>
                        <div class="flex justify-end gap-3">
                            <button
                                class="px-4 h-10 text-sm rounded-lg"
                                style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                on:click=move |_| pending.set(None)
                                disabled=move || running.get()
                            >
                                "Cancelar"
                            </button>
                            <button
                                class="btn-primary px-4 h-10 text-sm"
                                on:click=move |_| on_exec.run(act.clone())
                                disabled=move || running.get()
                            >
                                {move || if running.get() { "Ejecutando..." } else { "Ejecutar" }}
                            </button>
                        </div>
                    </div>
                </div>
            }
        })}
    }
}

#[component]
fn SystemsTable(systems: Vec<SupportSystem>, refresh: RwSignal<u32>) -> impl IntoView {
    let rows = systems
        .into_iter()
        .map(|s| {
            let (color, label) = status_meta(&s.status);
            let latency = s
                .latency_ms
                .map(|l| format!("{l:.1} ms"))
                .unwrap_or_else(|| "—".to_string());
            let freshness = fmt_opt_secs(s.freshness_secs);
            view! {
                <tr class="hover:bg-black/5">
                    <td class="px-4 py-3 text-sm font-bold" style="color:var(--uci-text);">
                        {s.name}
                    </td>
                    <td class="px-4 py-3">
                        <span
                            class="px-2 py-1 rounded-full text-xs font-semibold"
                            style=format!("background:{color}1A; color:{color};")
                        >
                            {label}
                        </span>
                    </td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{latency}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{s.ok_count}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{s.error_count}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{freshness}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{s.details}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <table class="w-full">
                <thead style="background:var(--uci-bg);">
                    <tr>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Subsistema"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Estado"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Latencia"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"OK"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Errores"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Frescura"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Detalle"</th>
                    </tr>
                </thead>
                <tbody class="divide-y" style="border-color:var(--uci-border);">
                    {rows}
                </tbody>
            </table>
            <div class="p-3 text-right">
                <button
                    class="text-xs px-3 py-1 rounded-lg"
                    style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                    on:click=move |_| refresh.update(|n| *n += 1)
                >
                    <i class="fa-solid fa-rotate mr-1"></i>"Reconsultar"
                </button>
            </div>
        </div>
    }
}

#[component]
fn DiagnosticsList(diagnostics: Vec<Diagnostic>) -> impl IntoView {
    let cards = diagnostics
        .into_iter()
        .map(|d| {
            let indicators = d
                .indicators
                .iter()
                .map(|ind| {
                    let color = if ind.ok { "#10B981" } else { "#EF4444" };
                    let icon = if ind.ok { "fa-circle-check" } else { "fa-circle-xmark" };
                    view! {
                        <div class="flex items-center justify-between py-1">
                            <span class="text-sm" style="color:var(--uci-muted);">
                                <i class=format!("fa-solid {icon} mr-2") style=format!("color:{color};")></i>
                                {ind.label.clone()}
                            </span>
                            <span class="text-sm font-semibold" style="color:var(--uci-text);">
                                {ind.value.clone()}
                                <span class="text-xs ml-2" style="color:var(--uci-muted);">
                                    {format!("(objetivo {})", ind.target)}
                                </span>
                            </span>
                        </div>
                    }
                })
                .collect_view();

            let suggested = d
                .suggested_actions
                .iter()
                .filter(|a| a.as_str() != "none")
                .map(|a| action_label(a).to_string())
                .collect::<Vec<_>>();

            view! {
                <div class="glass-card p-5">
                    <h3 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-text);">
                        {system_label(&d.system)}
                    </h3>
                    <div class="space-y-1">{indicators}</div>
                    <div class="mt-3 pt-3 text-xs" style="border-top:1px solid var(--uci-border); color:var(--uci-muted);">
                        {if suggested.is_empty() {
                            "Sin acciones sugeridas".to_string()
                        } else {
                            format!("Runbook: {}", suggested.join(", "))
                        }}
                    </div>
                    <div class="text-xs mt-1" style="color:var(--uci-muted);">{d.notes.clone()}</div>
                </div>
            }
        })
        .collect_view();

    view! { <div class="grid grid-cols-1 md:grid-cols-2 gap-4">{cards}</div> }
}

#[component]
fn HistoryTable(events: Vec<SupportEvent>) -> impl IntoView {
    if events.is_empty() {
        return view! {
            <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                <i class="fa-solid fa-clock-rotate-left text-2xl mb-2"></i>
                <p>"Sin eventos registrados"</p>
            </div>
        }.into_any();
    }

    let rows = events
        .into_iter()
        .map(|e| {
            let auto = e.origin == "auto";
            let origin_label = if auto { "Auto" } else { "Manual" };
            let origin_color = if auto { "#3B82F6" } else { "#8B5CF6" };
            let ok_color = if e.success { "#10B981" } else { "#EF4444" };
            let actor = e
                .username
                .clone()
                .or_else(|| e.ip_address.clone())
                .unwrap_or_else(|| "sistema".to_string());
            view! {
                <tr class="hover:bg-black/5">
                    <td class="px-4 py-3 text-xs whitespace-nowrap" style="color:var(--uci-muted);">{e.timestamp}</td>
                    <td class="px-4 py-3">
                        <span class="px-2 py-1 rounded-full text-xs font-semibold" style=format!("background:{origin_color}1A; color:{origin_color};")>
                            {origin_label}
                        </span>
                    </td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-text);">{system_label(&e.subsystem)}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-text);">{e.action}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{actor}</td>
                    <td class="px-4 py-3">
                        <i class=format!("fa-solid {}", if e.success { "fa-check" } else { "fa-xmark" }) style=format!("color:{ok_color};")></i>
                    </td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{e.message}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <table class="w-full">
                <thead style="background:var(--uci-bg);">
                    <tr>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Fecha"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Origen"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Subsistema"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Acción"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Actor"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">""</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Mensaje"</th>
                    </tr>
                </thead>
                <tbody class="divide-y" style="border-color:var(--uci-border);">
                    {rows}
                </tbody>
            </table>
        </div>
    }.into_any()
}

#[component]
fn ActionsPanel(
    #[prop(into)] model: RwSignal<String>,
    #[prop(into)] version: RwSignal<String>,
    #[prop(into)] running: RwSignal<bool>,
    #[prop(into)] on_action: Callback<String>,
) -> impl IntoView {
    let buttons = ACTIONS
        .iter()
        .map(|(key, label, icon)| {
            let key = key.to_string();
            view! {
                <button
                    class="flex flex-col items-center justify-center gap-2 p-4 rounded-xl text-sm font-medium"
                    style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                    on:click=move |_| on_action.run(key.clone())
                    disabled=move || running.get()
                >
                    <i class=format!("fa-solid {icon} text-lg")></i>
                    {*label}
                </button>
            }
        })
        .collect_view();

    view! {
        <div class="glass-card p-5">
            <h3 class="text-sm font-bold uppercase mb-1" style="color:var(--uci-text);">
                <i class="fa-solid fa-bolt mr-2"></i>"Runbook de corrección"
            </h3>
            <p class="text-xs mb-4" style="color:var(--uci-muted);">
                "Cada acción es idempotente y queda auditada. Se solicita confirmación antes de ejecutar."
            </p>

            <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
                {buttons}
            </div>

            <div class="flex flex-wrap items-end gap-3 mt-5 pt-4" style="border-top:1px solid var(--uci-border);">
                <div>
                    <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">
                        "Modelo (model_swap)"
                    </label>
                    <input
                        type="text"
                        prop:value=move || model.get()
                        on:input=move |ev| model.set(event_target_value(&ev))
                        class="px-3 h-9 text-sm rounded-lg"
                        style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                        placeholder="ews"
                    />
                </div>
                <div>
                    <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">
                        "Versión (opcional)"
                    </label>
                    <input
                        type="text"
                        prop:value=move || version.get()
                        on:input=move |ev| version.set(event_target_value(&ev))
                        class="px-3 h-9 text-sm rounded-lg"
                        style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                        placeholder="v1.0.0"
                    />
                </div>
                <span class="text-xs" style="color:var(--uci-muted);">
                    "Solo aplica a «Reactivar modelo ML»."
                </span>
            </div>
        </div>
    }
}
