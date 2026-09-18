use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::api::{Escalation, EscalationPolicy, SetPolicyRequest};
use crate::components::ui_kit::{ErrorState, LoadingState};
use crate::stores::user_has;

fn severity_meta(sev: &str) -> (&'static str, &'static str) {
    match sev {
        "critical" => ("#DC2626", "Crítica"),
        "high" => ("#EF4444", "Alta"),
        "medium" => ("#F59E0B", "Media"),
        "low" => ("#3B82F6", "Baja"),
        _ => ("#6B7280", "Desconocida"),
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "created" => "Creada",
        "acknowledged" => "Acusada",
        "escalated" => "Escalada",
        "resolved" => "Resuelta",
        _ => "Desconocido",
    }
}

#[component]
pub fn EscalationPage() -> impl IntoView {
    let refresh = RwSignal::new(0u32);
    let feedback = RwSignal::new(Option::<(bool, String)>::None);
    let running = RwSignal::new(Option::<String>::None);
    let can_act = user_has("escalation:act");
    let can_config = user_has("config:write");

    let active = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::list_active_escalations().await }
    });
    let policies = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::list_escalation_policies().await }
    });

    let editing = RwSignal::new(Option::<EscalationPolicy>::None);
    let e_max = RwSignal::new(0u32);
    let e_timeout = RwSignal::new(0u32);
    let e_role = RwSignal::new(String::new());
    let e_enabled = RwSignal::new(true);

    let open_edit = Callback::new(move |p: EscalationPolicy| {
        e_max.set(p.max_response_minutes);
        e_timeout.set(p.timeout_minutes);
        e_role.set(p.target_role.clone());
        e_enabled.set(p.enabled);
        editing.set(Some(p));
    });

    let save_policy = move |_| {
        let Some(p) = editing.get() else { return };
        let req = SetPolicyRequest {
            severity: p.severity.clone(),
            max_response_minutes: Some(e_max.get()),
            timeout_minutes: Some(e_timeout.get()),
            target_role: Some(e_role.get()),
            enabled: Some(e_enabled.get()),
        };
        running.set(Some("policy".to_string()));
        feedback.set(None);
        spawn_local(async move {
            let result = match api::set_escalation_policy(req).await {
                Ok(_) => (true, "Política actualizada.".to_string()),
                Err(e) => (false, e),
            };
            feedback.set(Some(result));
            running.set(None);
            editing.set(None);
            refresh.update(|n| *n += 1);
        });
    };

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-center justify-between gap-4 mb-6">
                <div>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-bell mr-2"></i>"Alertas y Escalamiento"
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        "Ciclo de vida de alertas clínicas y políticas de escalamiento por severidad"
                    </p>
                </div>
                <button
                    class="px-4 h-10 text-sm rounded-lg"
                    style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                    on:click=move |_| refresh.update(|n| *n += 1)
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

            <h2 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-triangle-exclamation mr-2" style="color:#EF4444;"></i>"Escalaciones activas"
            </h2>
            <Suspense fallback=move || view! { <LoadingState label="Cargando escalaciones..." /> }>
                {move || match active.get() {
                    Some(Ok(list)) => view! { <ActiveTable escalations=list can_act=can_act refresh=refresh running=running feedback=feedback /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo cargar las escalaciones: {}", e)
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>

            <h2 class="text-sm font-bold uppercase mt-6 mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-sliders mr-2"></i>"Políticas por severidad"
            </h2>
            <Suspense fallback=move || view! { <LoadingState label="Cargando políticas..." /> }>
                {move || match policies.get() {
                    Some(Ok(list)) => view! { <PoliciesTable policies=list can_config=can_config on_edit=open_edit /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo cargar las políticas: {}", e)
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>
        </div>

        {move || editing.get().map(|p| {
            let (color, label) = severity_meta(&p.severity);
            view! {
                <div
                    style="position:fixed; inset:0; z-index:50; display:flex; align-items:center; justify-content:center; background:rgba(0,0,0,0.55);"
                    role="dialog"
                    aria-modal="true"
                    aria-label="Editar política de escalamiento"
                >
                    <div class="glass-card" style="max-width:480px; width:92%; padding:24px;">
                        <h3 class="text-lg font-bold mb-1" style="color:var(--uci-text);">
                            "Política "
                            <span style=format!("color:{color};")>{label}</span>
                        </h3>
                        <p class="text-xs mb-4" style="color:var(--uci-muted);">
                            "Upsert idempotente por severidad; los cambios quedan auditados."
                        </p>
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                            <div>
                                <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">"Respuesta máx. (min)"</label>
                                <input type="number" min="0"
                                    class="w-full px-3 h-9 text-sm rounded-lg"
                                    style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                    prop:value=move || e_max.get().to_string()
                                    on:input=move |ev| e_max.set(event_target_value(&ev).parse().unwrap_or(0))
                                />
                            </div>
                            <div>
                                <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">"Timeout (min)"</label>
                                <input type="number" min="0"
                                    class="w-full px-3 h-9 text-sm rounded-lg"
                                    style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                    prop:value=move || e_timeout.get().to_string()
                                    on:input=move |ev| e_timeout.set(event_target_value(&ev).parse().unwrap_or(0))
                                />
                            </div>
                            <div>
                                <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">"Rol destino"</label>
                                <input type="text"
                                    class="w-full px-3 h-9 text-sm rounded-lg"
                                    style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                    prop:value=move || e_role.get()
                                    on:input=move |ev| e_role.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="flex items-center gap-2 mt-5">
                                <input type="checkbox"
                                    prop:checked=move || e_enabled.get()
                                    on:change=move |ev| e_enabled.set(event_target_checked(&ev))
                                />
                                <label class="text-sm" style="color:var(--uci-text);">"Habilitada"</label>
                            </div>
                        </div>
                        <div class="flex justify-end gap-3 mt-6">
                            <button
                                class="px-4 h-10 text-sm rounded-lg"
                                style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                on:click=move |_| editing.set(None)
                            >
                                "Cancelar"
                            </button>
                            <button class="btn-primary px-4 h-10 text-sm" on:click=save_policy
                                disabled=move || running.get().is_some()>
                                {move || if running.get().as_deref() == Some("policy") { "Guardando..." } else { "Guardar" }}
                            </button>
                        </div>
                    </div>
                </div>
            }
        })}
    }
}

#[component]
fn ActiveTable(
    escalations: Vec<Escalation>,
    can_act: bool,
    refresh: RwSignal<u32>,
    running: RwSignal<Option<String>>,
    feedback: RwSignal<Option<(bool, String)>>,
) -> impl IntoView {
    if escalations.is_empty() {
        return view! {
            <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                <i class="fa-solid fa-circle-check text-2xl mb-2" style="color:#10B981;"></i>
                <p>"Sin escalaciones activas"</p>
            </div>
        }.into_any();
    }

    let rows = escalations
        .into_iter()
        .map(|e| {
            let (color, label) = severity_meta(&e.severity);
            let id_ack = StoredValue::new(e.id.clone());
            let id_esc = StoredValue::new(e.id.clone());
            view! {
                <tr class="hover:bg-black/5">
                    <td class="px-4 py-3">
                        <span class="px-2 py-1 rounded-full text-xs font-semibold" style=format!("background:{color}1A; color:{color};")>
                            {label}
                        </span>
                    </td>
                    <td class="px-4 py-3 text-sm font-bold" style="color:var(--uci-text);">{e.patient_id}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{e.alert_type}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{status_label(&e.status)}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{format!("N{}", e.level)}</td>
                    <td class="px-4 py-3 text-xs whitespace-nowrap" style="color:var(--uci-muted);">{e.created_at}</td>
                    <Show when=move || can_act>
                        <td class="px-4 py-3 text-right whitespace-nowrap">
                            <button
                                class="text-xs px-3 py-1 rounded-lg mr-2"
                                style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                disabled=move || running.get().is_some()
                                on:click=move |_| {
                                    let id = id_ack.get_value();
                                    running.set(Some(id.clone()));
                                    feedback.set(None);
                                    spawn_local(async move {
                                        let r = match api::ack_escalation(&id).await {
                                            Ok(_) => (true, "Alerta acusada.".to_string()),
                                            Err(e) => (false, e),
                                        };
                                        feedback.set(Some(r));
                                        running.set(None);
                                        refresh.update(|n| *n += 1);
                                    });
                                }
                            >
                                <i class="fa-solid fa-check mr-1"></i>"Acusar"
                            </button>
                            <button
                                class="text-xs px-3 py-1 rounded-lg"
                                style="background:rgba(239,68,68,0.12); color:#EF4444; border:1px solid rgba(239,68,68,0.35);"
                                disabled=move || running.get().is_some()
                                on:click=move |_| {
                                    let id = id_esc.get_value();
                                    running.set(Some(id.clone()));
                                    feedback.set(None);
                                    spawn_local(async move {
                                        let r = match api::escalate_escalation(&id).await {
                                            Ok(_) => (true, "Alerta escalada.".to_string()),
                                            Err(e) => (false, e),
                                        };
                                        feedback.set(Some(r));
                                        running.set(None);
                                        refresh.update(|n| *n += 1);
                                    });
                                }
                            >
                                <i class="fa-solid fa-arrow-up-right-dots mr-1"></i>"Escalar"
                            </button>
                        </td>
                    </Show>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="rounded-xl overflow-x-auto" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <table class="w-full">
                <thead style="background:var(--uci-bg);">
                    <tr>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Severidad"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Paciente"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Tipo"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Estado"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Nivel"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Creada"</th>
                        <Show when=move || can_act>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">""</th>
                        </Show>
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
fn PoliciesTable(
    policies: Vec<EscalationPolicy>,
    can_config: bool,
    on_edit: Callback<EscalationPolicy>,
) -> impl IntoView {
    let rows = policies
        .into_iter()
        .map(|p| {
            let (color, label) = severity_meta(&p.severity);
            let editable = StoredValue::new(p.clone());
            let enabled = p.enabled;
            view! {
                <tr class="hover:bg-black/5">
                    <td class="px-4 py-3">
                        <span class="px-2 py-1 rounded-full text-xs font-semibold" style=format!("background:{color}1A; color:{color};")>
                            {label}
                        </span>
                    </td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-text);">{format!("{} min", p.max_response_minutes)}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-text);">{format!("{} min", p.timeout_minutes)}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{p.target_role.clone()}</td>
                    <td class="px-4 py-3">
                        <i class=format!("fa-solid {}", if enabled { "fa-circle-check" } else { "fa-circle-xmark" })
                            style=format!("color:{};", if enabled { "#10B981" } else { "#6B7280" })></i>
                    </td>
                    <Show when=move || can_config>
                        <td class="px-4 py-3 text-right">
                            <button
                                class="text-xs px-3 py-1 rounded-lg"
                                style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                on:click=move |_| on_edit.run(editable.get_value())
                            >
                                <i class="fa-solid fa-pen mr-1"></i>"Editar"
                            </button>
                        </td>
                    </Show>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="rounded-xl overflow-x-auto" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <table class="w-full">
                <thead style="background:var(--uci-bg);">
                    <tr>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Severidad"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Respuesta máx."</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Timeout"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Rol destino"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Habilitada"</th>
                        <Show when=move || can_config>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">""</th>
                        </Show>
                    </tr>
                </thead>
                <tbody class="divide-y" style="border-color:var(--uci-border);">
                    {rows}
                </tbody>
            </table>
        </div>
    }
}
