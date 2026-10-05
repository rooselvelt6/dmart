use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::api::{TenancyAuditReport, TenantListItem};
use crate::components::ui_kit::{ErrorState, LoadingState};
use crate::stores::user_has;

#[component]
pub fn TenantsPage() -> impl IntoView {
    let reload = RwSignal::new(0u32);
    let feedback = RwSignal::new(Option::<(bool, String)>::None);
    let busy = RwSignal::new(false);

    // Alta de tenant: solo admin (`tenants:manage`).
    let can_manage = user_has("tenants:manage");
    let name = RwSignal::new(String::new());
    let slug = RwSignal::new(String::new());
    let form_open = RwSignal::new(false);

    let tenants = LocalResource::new(move || {
        let _ = reload.get();
        async move { api::list_tenants().await }
    });

    // La auditoría de aislamiento responde 409 si el reporte no es sano; el
    // cliente la trata como error legible con el detalle del servidor.
    let tenancy_audit = LocalResource::new(move || {
        let _ = reload.get();
        async move { api::audit_tenancy().await }
    });

    // Slug prellenado desde el slug: solo [a-z0-9-].
    let on_name = move |ev| {
        let v = event_target_value(&ev);
        slug.set(
            v.to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string(),
        );
        name.set(v);
    };

    let submit = move |_| {
        let (n, s) = (name.get(), slug.get());
        if n.trim().len() < 3 || s.len() < 3 {
            feedback.set(Some((false, crate::i18n::tr("tenants-err-min-len", None))));
            return;
        }
        busy.set(true);
        feedback.set(None);
        spawn_local(async move {
            match api::create_tenant(&n, &s).await {
                Ok(t) => {
                    let mut args = std::collections::HashMap::new();
                    args.insert("name".to_string(), t.name.clone());
                    feedback.set(Some((
                        true,
                        crate::i18n::tr("tenants-ok-created", Some(&args)),
                    )));
                    name.set(String::new());
                    slug.set(String::new());
                    form_open.set(false);
                    reload.update(|x| *x += 1);
                }
                Err(e) => feedback.set(Some((false, e))),
            }
            busy.set(false);
        });
    };

    let impersonate = Callback::new(move |slug: String| {
        busy.set(true);
        feedback.set(None);
        spawn_local(async move {
            match api::impersonate_tenant(&slug).await {
                Ok(g) => {
                    let mins = ((g.expires_at - now_secs()) / 60).max(0);
                    let mut args = std::collections::HashMap::new();
                    args.insert("tenant".to_string(), g.tenant_id.clone());
                    args.insert("min".to_string(), mins.to_string());
                    feedback.set(Some((
                        true,
                        crate::i18n::tr("tenants-ok-impersonated", Some(&args)),
                    )));
                }
                Err(e) => feedback.set(Some((false, e))),
            }
            busy.set(false);
        });
    });

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-center justify-between gap-4 mb-6">
                <div>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-building mr-2"></i>{move || crate::i18n::tr("nav-tenants", None)}
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        {move || crate::i18n::tr("tenants-subtitle", None)}
                    </p>
                </div>
                <div class="flex gap-2">
                    <Show when=move || can_manage>
                        <button
                            class="btn-primary px-4 h-10 text-sm"
                            on:click=move |_| form_open.update(|o| *o = !*o)
                        >
                            <i class="fa-solid fa-plus mr-2"></i>{move || crate::i18n::tr("tenants-new", None)}
                        </button>
                    </Show>
                    <button
                        class="px-4 h-10 text-sm rounded-lg"
                        style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                        on:click=move |_| reload.update(|n| *n += 1)
                    >
                        <i class="fa-solid fa-rotate mr-2"></i>{move || crate::i18n::tr("action-refresh", None)}
                    </button>
                </div>
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

            <Show when=move || form_open.get() && can_manage>
                <div class="glass-card p-4 mb-5">
                    <h2 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-text);">
                        <i class="fa-solid fa-building-circle-check mr-2" style="color:#10B981;"></i>{move || crate::i18n::tr("tenants-create-title", None)}
                    </h2>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-3 items-end">
                        <div>
                            <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">
                                {move || crate::i18n::tr("tenants-field-name", None)}
                            </label>
                            <input
                                type="text"
                                class="form-input"
                                style="padding-top:9px; padding-bottom:9px; font-size:14px;"
                                prop:value=move || name.get()
                                on:input=on_name
                            />
                        </div>
                        <div>
                            <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">
                                {move || crate::i18n::tr("tenants-field-slug", None)}
                            </label>
                            <input
                                type="text"
                                class="form-input"
                                style="padding-top:9px; padding-bottom:9px; font-size:14px; font-family:var(--uci-font-mono);"
                                prop:value=move || slug.get()
                                on:input=move |ev| slug.set(event_target_value(&ev))
                            />
                        </div>
                    </div>
                    <div class="flex justify-end gap-2 mt-3">
                        <button
                            class="px-4 h-9 text-sm rounded-lg"
                            style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                            on:click=move |_| form_open.set(false)
                        >
                            {move || crate::i18n::tr("action-cancel", None)}
                        </button>
                        <button
                            class="btn-primary px-4 h-9 text-sm"
                            on:click=submit
                            disabled=move || busy.get()
                        >
                            {move || if busy.get() {
                                crate::i18n::tr("tenants-creating", None)
                            } else {
                                crate::i18n::tr("tenants-create", None)
                            }}
                        </button>
                    </div>
                </div>
            </Show>

            <h2 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-list mr-2" style="color:#3B82F6;"></i>{move || crate::i18n::tr("tenants-list-title", None)}
            </h2>
            <Suspense fallback=move || view! { <LoadingState label=crate::i18n::tr("tenants-loading", None) /> }>
                {move || match tenants.get() {
                    Some(Ok(list)) => view! {
                        <TenantsTable tenants=list on_impersonate=impersonate busy=busy />
                    }
                    .into_any(),
                    Some(Err(e)) => {
                        let mut args = std::collections::HashMap::new();
                        args.insert("error".to_string(), e.to_string());
                        view! {
                        <ErrorState
                            message=crate::i18n::tr("tenants-load-error", Some(&args))
                            on_retry=Some(Callback::new(move |()| reload.update(|n| *n += 1)))
                        />
                    }
                        .into_any()
                    }
                    None => ().into_any(),
                }}
            </Suspense>

            <h2 class="text-sm font-bold uppercase mt-6 mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-shield-halved mr-2" style="color:#8B5CF6;"></i>{move || crate::i18n::tr("tenants-audit-title", None)}
            </h2>
            <Suspense fallback=move || view! { <LoadingState label=crate::i18n::tr("tenants-auditing", None) /> }>
                {move || match tenancy_audit.get() {
                    Some(Ok(report)) => view! { <IsolationReport report=report /> }.into_any(),
                    Some(Err(e)) => view! {
                        <div
                            class="p-4 rounded-xl"
                            style="background:rgba(220,38,38,0.08); border:1px solid rgba(220,38,38,0.35);"
                            role="alert"
                        >
                            <p class="text-sm font-bold" style="color:var(--uci-text);">
                                <i class="fa-solid fa-triangle-exclamation mr-2" style="color:#DC2626;"></i>
                                {move || crate::i18n::tr("tenants-isolation-broken", None)}
                            </p>
                            <p class="text-xs mt-1" style="color:var(--uci-muted);">{e}</p>
                        </div>
                    }
                    .into_any(),
                    None => ().into_any(),
                }}
            </Suspense>
        </div>
    }
}

fn now_secs() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

#[component]
fn TenantsTable(
    tenants: Vec<TenantListItem>,
    on_impersonate: Callback<String>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    if tenants.is_empty() {
        return view! {
            <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                <i class="fa-solid fa-building-circle-exclamation text-2xl mb-2" style="color:#94A3B8;"></i>
                <p>{move || crate::i18n::tr("tenants-empty", None)}</p>
            </div>
        }
        .into_any();
    }

    let tenants = StoredValue::new(tenants);

    view! {
        <div class="glass-card overflow-x-auto">
            <table class="w-full text-sm" style="color:var(--uci-text);">
                <thead>
                    <tr style="border-bottom:1px solid var(--uci-border);">
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("tenants-col-org", None)}
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("tenants-col-slug", None)}
                        </th>
                        <th class="text-right px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("tenants-col-patients", None)}
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("equipment-status", None)}
                        </th>
                        <th class="text-right px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            {move || crate::i18n::tr("tenants-col-action", None)}
                        </th>
                    </tr>
                </thead>
                <tbody>
                    <For each=move || tenants.get_value() key=|t| t.slug.clone() let:t>
                        {
                            let active = t.active;
                            let can_impersonate = t.slug != "default";
                            let slug = StoredValue::new(t.slug.clone());
                            view! {
                        <tr style="border-bottom:1px solid var(--uci-border);">
                            <td class="px-4 py-3 font-semibold">{t.name.clone()}</td>
                            <td
                                class="px-4 py-3 text-xs"
                                style="color:var(--uci-muted); font-family:var(--uci-font-mono);"
                            >
                                {t.slug.clone()}
                            </td>
                            <td class="px-4 py-3 text-right tabular-nums">{t.patient_count}</td>
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
                            <td class="px-4 py-3 text-right">
                                <Show
                                    when=move || can_impersonate
                                    fallback=|| view! {
                                        <span class="text-xs" style="color:var(--uci-muted);">"—"</span>
                                    }
                                >
                                    <button
                                        class="px-3 h-8 text-xs rounded-lg"
                                        style="background:var(--uci-surface); color:var(--uci-accent); border:1px solid var(--uci-border);"
                                        disabled=move || busy.get()
                                        on:click=move |_| on_impersonate.run(slug.get_value())
                                    >
                                        <i class="fa-solid fa-user-secret mr-1"></i>
                                        {move || crate::i18n::tr("tenants-impersonate", None)}
                                    </button>
                                </Show>
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

#[component]
fn IsolationReport(report: TenancyAuditReport) -> impl IntoView {
    let healthy = report.healthy;
    let (color, icon) = if healthy {
        ("#10B981", "fa-circle-check")
    } else {
        ("#DC2626", "fa-triangle-exclamation")
    };
    let total_records = report.total_records;
    let records_at_risk = report.records_at_risk;

    let table_rows = StoredValue::new(report.tables.clone());

    view! {
        <div class="glass-card p-5">
            <div class="flex flex-wrap items-center gap-3 mb-4">
                <span
                    class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold uppercase"
                    style=format!("background:{color}1A; color:{color}; border:1px solid {color}55;")
                >
                    <i class=format!("fa-solid {icon} text-[10px]")></i>
                    {move || if healthy {
                        crate::i18n::tr("tenants-isolation-ok", None)
                    } else {
                        crate::i18n::tr("tenants-isolation-at-risk", None)
                    }}
                </span>
                <span class="text-xs tabular-nums" style="color:var(--uci-muted);">
                    {move || {
                        let mut args = std::collections::HashMap::new();
                        args.insert("count".to_string(), total_records.to_string());
                        crate::i18n::tr("tenants-records-scanned", Some(&args))
                    }}
                </span>
                <Show when=move || records_at_risk != 0>
                    <span class="text-xs tabular-nums" style="color:#DC2626;">
                        {move || {
                            let mut args = std::collections::HashMap::new();
                            args.insert("count".to_string(), records_at_risk.to_string());
                            crate::i18n::tr("tenants-records-at-risk", Some(&args))
                        }}
                    </span>
                </Show>
            </div>

            <div class="overflow-x-auto">
                <table class="w-full text-sm" style="color:var(--uci-text);">
                    <thead>
                        <tr style="border-bottom:1px solid var(--uci-border);">
                            <th class="text-left px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                {move || crate::i18n::tr("tenants-col-table", None)}
                            </th>
                            <th class="text-right px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                {move || crate::i18n::tr("tenants-col-total", None)}
                            </th>
                            <th class="text-right px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                {move || crate::i18n::tr("tenants-col-missing-tenant", None)}
                            </th>
                            <th class="text-left px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                {move || crate::i18n::tr("tenants-col-orphan-ids", None)}
                            </th>
                        </tr>
                    </thead>
                    <tbody>
                        <For each=move || table_rows.get_value() key=|t| t.table.clone() let:t>
                            <tr style="border-bottom:1px solid var(--uci-border);">
                                <td class="px-3 py-2 font-semibold" style="font-family:var(--uci-font-mono); font-size:12px;">
                                    {t.table.clone()}
                                </td>
                                <td class="px-3 py-2 text-right tabular-nums">{t.total}</td>
                                <td
                                    class="px-3 py-2 text-right tabular-nums font-semibold"
                                    style=move || if t.missing_tenant_id != 0 {
                                        "color:#DC2626;"
                                    } else {
                                        "color:var(--uci-muted);"
                                    }
                                >
                                    {t.missing_tenant_id}
                                </td>
                                <td class="px-3 py-2 text-xs" style="color:var(--uci-muted); font-family:var(--uci-font-mono);">
                                    {if t.orphan_tenant_ids.is_empty() {
                                        "—".to_string()
                                    } else {
                                        t.orphan_tenant_ids.join(", ")
                                    }}
                                </td>
                            </tr>
                        </For>
                    </tbody>
                </table>
            </div>
        </div>
    }
}
