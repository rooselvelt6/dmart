use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::api::{CdsEvaluation, CdsPlanSummary};
use crate::components::ui_kit::{ErrorState, LoadingState};
use crate::stores::user_has;

/// Icono + color por `ActionKind` (serde `lowercase` en el servidor).
/// La etiqueta se traduce: llámala dentro del `move ||` que la renderiza.
fn action_meta(kind: &str) -> (&'static str, &'static str, String) {
    match kind {
        "alert" => (
            "fa-triangle-exclamation",
            "#EF4444",
            crate::i18n::tr("cds-action-alert", None),
        ),
        "order" => (
            "fa-file-prescription",
            "#3B82F6",
            crate::i18n::tr("cds-action-order", None),
        ),
        "notification" => (
            "fa-bell",
            "#F59E0B",
            crate::i18n::tr("cds-action-notification", None),
        ),
        "protocol" => (
            "fa-list-check",
            "#8B5CF6",
            crate::i18n::tr("cds-action-protocol", None),
        ),
        "referral" => (
            "fa-hand-point-right",
            "#14B8A6",
            crate::i18n::tr("cds-action-referral", None),
        ),
        _ => (
            "fa-circle",
            "#94A3B8",
            crate::i18n::tr("cds-action-other", None),
        ),
    }
}

#[component]
pub fn CdsPage() -> impl IntoView {
    let reload = RwSignal::new(0u32);
    let feedback = RwSignal::new(Option::<(bool, String)>::None);
    let can_evaluate = user_has("patients:read");

    let plans = LocalResource::new(move || {
        let _ = reload.get();
        async move { api::list_cds_plans().await }
    });

    // Paciente a evaluar + resultado de la última evaluación.
    let selected = RwSignal::new(String::new());
    let evaluating = RwSignal::new(false);
    let result = RwSignal::new(Option::<CdsEvaluation>::None);
    let patient_query = RwSignal::new(String::new());
    let patients = LocalResource::new(move || {
        let _ = reload.get();
        async move { api::list_patients(None, None).await }
    });

    let evaluate = move |_| {
        let pid = selected.get();
        if pid.is_empty() {
            feedback.set(Some((
                false,
                crate::i18n::tr("cds-err-select-patient", None),
            )));
            return;
        }
        evaluating.set(true);
        feedback.set(None);
        spawn_local(async move {
            match api::evaluate_cds(&pid).await {
                Ok(r) => {
                    let triggered = r.results.iter().filter(|x| x.triggered).count();
                    let mut args = std::collections::HashMap::new();
                    args.insert("triggered".to_string(), triggered.to_string());
                    args.insert("total".to_string(), r.results.len().to_string());
                    feedback.set(Some((
                        true,
                        crate::i18n::tr("cds-ok-evaluated", Some(&args)),
                    )));
                    result.set(Some(r));
                }
                Err(e) => feedback.set(Some((false, e))),
            }
            evaluating.set(false);
        });
    };

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-center justify-between gap-4 mb-6">
                <div>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-clipboard-list mr-2"></i>{move || crate::i18n::tr("cds-title", None)}
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        {move || crate::i18n::tr("cds-subtitle", None)}
                    </p>
                </div>
                <button
                    class="px-4 h-10 text-sm rounded-lg"
                    style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                    on:click=move |_| reload.update(|n| *n += 1)
                >
                    <i class="fa-solid fa-rotate mr-2"></i>{move || crate::i18n::tr("action-refresh", None)}
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

            <Show when=move || can_evaluate fallback=|| ()>
                <div class="glass-card p-4 mb-5">
                    <h2 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-text);">
                        <i class="fa-solid fa-stethoscope mr-2" style="color:#3B82F6;"></i>{move || crate::i18n::tr("cds-evaluate-title", None)}
                    </h2>
                    <div class="grid grid-cols-1 md:grid-cols-[1fr_auto] gap-3 items-end">
                        <div>
                            <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">
                                {move || crate::i18n::tr("cds-search-patient", None)}
                            </label>
                            <input
                                type="text"
                                class="form-input"
                                style="padding-top:9px; padding-bottom:9px; font-size:14px;"
                                placeholder=move || crate::i18n::tr("cds-search-placeholder", None)
                                prop:value=move || patient_query.get()
                                on:input=move |ev| patient_query.set(event_target_value(&ev))
                            />
                        </div>
                        <button
                            class="btn-primary h-10 px-5 text-sm"
                            on:click=evaluate
                            disabled=move || evaluating.get()
                        >
                            {move || if evaluating.get() {
                                crate::i18n::tr("cds-evaluating", None)
                            } else {
                                crate::i18n::tr("cds-evaluate", None)
                            }}
                        </button>
                    </div>

                    <Show when=move || !patient_query.get().is_empty() fallback=|| ()>
                        <div class="mt-3 max-h-56 overflow-y-auto">
                            <Suspense fallback=move || view! { <LoadingState label=crate::i18n::tr("cds-searching", None) /> }>
                                {move || match patients.get() {
                                    Some(Ok(list)) => {
                                        let q = patient_query.get().to_lowercase();
                                        let hits: Vec<_> = list
                                            .into_iter()
                                            .filter(|p| {
                                                q.is_empty()
                                                    || p.nombre_completo.to_lowercase().contains(&q)
                                                    || p.cedula.to_lowercase().contains(&q)
                                            })
                                            .take(20)
                                            .collect();
                                        if hits.is_empty() {
                                            view! {
                                                <p class="text-sm py-3" style="color:var(--uci-muted);">
                                                    {move || crate::i18n::tr("cds-no-matches", None)}
                                                </p>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <ul>
                                                    <For
                                                        each=move || hits.clone()
                                                        key=|p| p.id.clone()
                                                        let:p
                                                    >
                                                        {
                                                            let pid_a = p.id.clone();
                                                            let pid_b = StoredValue::new(p.id.clone());
                                                            let label = StoredValue::new(format!(
                                                                "{} — {}",
                                                                p.nombre_completo, p.cedula
                                                            ));
                                                            view! {
                                                        <li>
                                                            <button
                                                                class="w-full text-left px-3 py-2 rounded-lg text-sm"
                                                                style=move || if selected.get() == pid_a {
                                                                    "background:var(--uci-accent-glow); color:var(--uci-accent);"
                                                                } else {
                                                                    "color:var(--uci-text);"
                                                                }
                                                                on:click=move |_| {
                                                                    selected.set(pid_b.get_value());
                                                                    patient_query.set(label.get_value());
                                                                }
                                                            >
                                                                <span class="font-semibold">{p.nombre_completo.clone()}</span>
                                                                <span class="ml-2 text-xs" style="color:var(--uci-muted);">
                                                                    {p.cedula.clone()}
                                                                </span>
                                                            </button>
                                                        </li>
                                                    }
                                                        }
                                                    </For>
                                                </ul>
                            }.into_any()
                                        }
                                    }
                                    Some(Err(e)) => view! {
                                        <p class="text-sm py-3" style="color:#EF4444;">{e}</p>
                                    }
                                    .into_any(),
                                    None => view! {}.into_any(),
                                }}
                            </Suspense>
                        </div>
                    </Show>
                </div>

                {move || result.get().map(|r| view! { <EvaluationResult ev=r /> })}
            </Show>

            <h2 class="text-sm font-bold uppercase mt-6 mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-layer-group mr-2" style="color:#8B5CF6;"></i>{move || crate::i18n::tr("cds-plans-title", None)}
            </h2>
            <Suspense fallback=move || view! { <LoadingState label=crate::i18n::tr("cds-loading-plans", None) /> }>
                {move || match plans.get() {
                    Some(Ok(list)) => view! { <PlansTable plans=list /> }.into_any(),
                    Some(Err(e)) => {
                        let mut args = std::collections::HashMap::new();
                        args.insert("error".to_string(), e.to_string());
                        view! {
                        <ErrorState
                            message=crate::i18n::tr("cds-load-error", Some(&args))
                            on_retry=Some(Callback::new(move |()| reload.update(|n| *n += 1)))
                        />
                    }
                        .into_any()
                    }
                    None => view! {}.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn EvaluationResult(ev: CdsEvaluation) -> impl IntoView {
    let triggered: Vec<_> = ev.results.iter().filter(|r| r.triggered).cloned().collect();
    let clean: Vec<_> = ev
        .results
        .iter()
        .filter(|r| !r.triggered)
        .cloned()
        .collect();
    let triggered_count = triggered.len();
    let clean_count = clean.len();

    let triggered = StoredValue::new(triggered);
    let clean = StoredValue::new(clean);

    view! {
        <div class="glass-card p-5 mb-5">
            <div class="flex flex-wrap items-center gap-3 mb-4">
                <h2 class="text-sm font-bold uppercase" style="color:var(--uci-text);">
                    <i class="fa-solid fa-microscope mr-2" style="color:#EC4899;"></i>{move || crate::i18n::tr("cds-result-title", None)}
                </h2>
                <Show when=move || triggered_count != 0>
                    <span
                        class="px-3 py-1 rounded-full text-xs font-bold uppercase"
                        style="background:rgba(220,38,38,0.14); color:#DC2626; border:1px solid rgba(220,38,38,0.35);"
                    >
                        <i class="fa-solid fa-triangle-exclamation mr-1"></i>
                        {move || {
                            let mut args = std::collections::HashMap::new();
                            args.insert("count".to_string(), triggered_count.to_string());
                            crate::i18n::tr("cds-triggered-plans", Some(&args))
                        }}
                    </span>
                </Show>
            </div>

            <Show
                when=move || triggered_count != 0
                fallback=|| view! {
                    <div class="p-6 text-center">
                        <i class="fa-solid fa-circle-check text-2xl mb-2" style="color:#10B981;"></i>
                        <p style="color:var(--uci-text); font-weight:600;">
                            {move || crate::i18n::tr("cds-none-triggered", None)}
                        </p>
                        <p class="text-xs mt-1" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("cds-thresholds-not-met", None)}
                        </p>
                    </div>
                }
            >
                <div class="flex flex-col gap-3">
                    <For each=move || triggered.get_value() key=|r| r.plan_id.clone() let:plan>
                        <div
                            class="rounded-xl p-4"
                            style="background:rgba(220,38,38,0.06); border:1px solid rgba(220,38,38,0.25);"
                        >
                            <div class="flex items-center gap-2 mb-3">
                                <i class="fa-solid fa-fire" style="color:#DC2626;"></i>
                                <span class="font-bold" style="color:var(--uci-text);">
                                    {plan.plan_id.clone()}
                                </span>
                                <span class="text-[10px] px-2 py-0.5 rounded" style="background:rgba(0,0,0,0.08); color:var(--uci-muted);">
                                    "v"
                                    {plan.plan_version.clone()}
                                </span>
                            </div>
                            <For
                                each=move || StoredValue::new(plan.actions.clone()).get_value()
                                key=|a| a.activity_id.clone()
                                let:action
                            >
                                {
                                    let (icon, color, _) = action_meta(&action.kind);
                                    let kind = StoredValue::new(action.kind.clone());
                                    let has_desc = !action.description.is_empty();
                                    view! {
                                        <div class="flex items-start gap-3 py-2">
                                            <span
                                                class="shrink-0 inline-flex items-center gap-1 px-2 py-1 rounded text-[10px] font-bold uppercase"
                                                style=format!("background:{color}22; color:{color};")
                                            >
                                                <i class=format!("fa-solid {icon} text-[9px]")></i>
                                                {move || action_meta(&kind.get_value()).2}
                                            </span>
                                            <div class="min-w-0">
                                                <p class="text-sm font-semibold" style="color:var(--uci-text);">
                                                    {action.title.clone()}
                                                </p>
                                                <Show when=move || has_desc>
                                                    <p class="text-xs mt-0.5" style="color:var(--uci-muted);">
                                                        {action.description.clone()}
                                                    </p>
                                                </Show>
                                            </div>
                                        </div>
                                    }
                                }
                            </For>
                        </div>
                    </For>
                </div>
            </Show>

            <Show when=move || clean_count != 0>
                <details class="mt-4">
                    <summary class="text-xs cursor-pointer" style="color:var(--uci-muted);">
                        {move || {
                            let mut args = std::collections::HashMap::new();
                            args.insert("count".to_string(), clean_count.to_string());
                            crate::i18n::tr("cds-clean-summary", Some(&args))
                        }}
                    </summary>
                    <ul class="mt-2 flex flex-wrap gap-2">
                        <For each=move || clean.get_value() key=|r| r.plan_id.clone() let:plan>
                            <li
                                class="px-2 py-1 rounded text-[11px]"
                                style="background:var(--uci-surface); border:1px solid var(--uci-border); color:var(--uci-muted);"
                            >
                                {plan.plan_id.clone()}
                            </li>
                        </For>
                    </ul>
                </details>
            </Show>

            <Show when=move || ev.care_plan.is_some()>
                <div
                    class="mt-4 pt-3 text-xs"
                    style="border-top:1px solid var(--uci-border); color:var(--uci-muted);"
                >
                    <i class="fa-solid fa-file-circle-check mr-1" style="color:#10B981;"></i>
                    {move || crate::i18n::tr("cds-careplan-note", None)}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn PlansTable(plans: Vec<CdsPlanSummary>) -> impl IntoView {
    if plans.is_empty() {
        return view! {
            <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                <i class="fa-solid fa-folder-open text-2xl mb-2" style="color:#94A3B8;"></i>
                <p>{move || crate::i18n::tr("cds-plans-empty", None)}</p>
            </div>
        }
        .into_any();
    }

    let plans = StoredValue::new(plans);

    view! {
        <div class="glass-card overflow-x-auto">
            <table class="w-full text-sm" style="color:var(--uci-text);">
                <thead>
                    <tr style="border-bottom:1px solid var(--uci-border);">
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("cds-col-plan", None)}
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("cds-col-version", None)}
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("equipment-status", None)}
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            "Fingerprint"
                        </th>
                    </tr>
                </thead>
                <tbody>
                    <For each=move || plans.get_value() key=|p| p.plan_id.clone() let:plan>
                        {
                            let has_meta = !plan.meta.is_empty();
                            let active = plan.active;
                            view! {
                                <tr style="border-bottom:1px solid var(--uci-border);">
                                    <td class="px-4 py-3">
                                        <span class="font-semibold">{plan.plan_id.clone()}</span>
                                        <Show when=move || has_meta>
                                            <p class="text-xs" style="color:var(--uci-muted);">
                                                {plan.meta.clone()}
                                            </p>
                                        </Show>
                                    </td>
                                    <td class="px-4 py-3 tabular-nums">{plan.version.clone()}</td>
                                    <td class="px-4 py-3">
                                        <Show
                                            when=move || active
                                            fallback=|| view! {
                                                <span class="severity-moderate">
                                                    {move || crate::i18n::tr("status-inactive", None)}
                                                </span>
                                            }
                                        >
                                            <span class="severity-low">
                                                {move || crate::i18n::tr("status-active", None)}
                                            </span>
                                        </Show>
                                    </td>
                                    <td
                                        class="px-4 py-3 text-[11px] truncate"
                                        style="color:var(--uci-muted); font-family:var(--uci-font-mono); max-width:180px;"
                                    >
                                        {plan.fingerprint.clone()}
                                    </td>
                                </tr>
                            }
                        }
                    </For>
                </tbody>
            </table>
        </div>
    }
    .into_any()
}
