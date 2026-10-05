use leptos::prelude::*;

use crate::api;
use crate::api::{QualityIssue, QualitySummary};
use crate::components::ui_kit::{ErrorState, LoadingState};

/// Args de interpolación para los mensajes de error de la API.
fn error_args(error: &str) -> std::collections::HashMap<String, String> {
    let mut args = std::collections::HashMap::new();
    args.insert("error".to_string(), error.to_string());
    args
}

fn severity_meta(sev: &str) -> (&'static str, String) {
    match sev {
        "high" => ("#EF4444", crate::i18n::tr("dq-sev-high", None)),
        "medium" => ("#F59E0B", crate::i18n::tr("dq-sev-medium", None)),
        "low" => ("#3B82F6", crate::i18n::tr("dq-sev-low", None)),
        _ => ("#6B7280", crate::i18n::tr("dq-sev-unknown", None)),
    }
}

#[component]
pub fn DataQualityPage() -> impl IntoView {
    let refresh = RwSignal::new(0u32);

    let summary = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::get_quality_summary().await }
    });
    let report = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::get_quality_report().await }
    });

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-center justify-between gap-4 mb-6">
                <div>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-shield-heart mr-2"></i>{crate::i18n::tr("dq-title", None)}
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        {crate::i18n::tr("dq-subtitle", None)}
                    </p>
                </div>
                <button
                    class="px-4 h-10 text-sm rounded-lg"
                    style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                    on:click=move |_| refresh.update(|n| *n += 1)
                >
                    <i class="fa-solid fa-rotate mr-2"></i>{crate::i18n::tr("dq-refresh", None)}
                </button>
            </div>

            <Suspense fallback=move || view! { <LoadingState label=crate::i18n::tr("dq-loading-summary", None) /> }>
                {move || match summary.get() {
                    Some(Ok(s)) => view! { <Summary summary=s /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=crate::i18n::tr("dq-summary-error", Some(&error_args(&e.to_string())))
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => ().into_any(),
                }}
            </Suspense>

            <h2 class="text-sm font-bold uppercase mt-6 mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-list-check mr-2"></i>{crate::i18n::tr("dq-latest-issues", None)}
            </h2>
            <Suspense fallback=move || view! { <LoadingState label=crate::i18n::tr("dq-loading-issues", None) /> }>
                {move || match report.get() {
                    Some(Ok(issues)) => view! { <IssuesTable issues=issues /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=crate::i18n::tr("dq-report-error", Some(&error_args(&e.to_string())))
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => ().into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn Summary(summary: QualitySummary) -> impl IntoView {
    let sevs = summary
        .by_severity
        .iter()
        .map(|s| {
            let (color, label) = severity_meta(&s.severity);
            view! {
                <div class="glass-card p-4">
                    <div class="text-xs uppercase font-semibold" style=format!("color:{color};")>{label}</div>
                    <div class="text-2xl font-bold mt-1" style="color:var(--uci-text);">{s.count}</div>
                </div>
            }
        })
        .collect_view();

    let codes = summary
        .by_code
        .iter()
        .map(|c| {
            view! {
                <div class="flex items-center justify-between py-1">
                    <span class="text-sm font-mono" style="color:var(--uci-muted);">{c.code.clone()}</span>
                    <span class="text-sm font-semibold" style="color:var(--uci-text);">{c.count}</span>
                </div>
            }
        })
        .collect_view();

    view! {
        <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div class="glass-card p-4">
                <div class="text-xs uppercase font-semibold" style="color:var(--uci-muted);">{crate::i18n::tr("dq-total", None)}</div>
                <div class="text-2xl font-bold mt-1" style="color:var(--uci-text);">{summary.total}</div>
            </div>
            {sevs}
        </div>
        <div class="glass-card p-5 mt-4">
            <h3 class="text-xs font-bold uppercase mb-2" style="color:var(--uci-text);">{crate::i18n::tr("dq-by-code", None)}</h3>
            <div class="space-y-1">{codes}</div>
        </div>
    }
}

#[component]
fn IssuesTable(issues: Vec<QualityIssue>) -> impl IntoView {
    if issues.is_empty() {
        return view! {
            <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                <i class="fa-solid fa-circle-check text-2xl mb-2" style="color:#10B981;"></i>
                <p>{crate::i18n::tr("dq-no-issues", None)}</p>
            </div>
        }.into_any();
    }

    let rows = issues
        .into_iter()
        .map(|i| {
            let (color, label) = severity_meta(&i.severity);
            view! {
                <tr class="hover:bg-black/5">
                    <td class="px-4 py-3 text-xs whitespace-nowrap" style="color:var(--uci-muted);">{i.timestamp}</td>
                    <td class="px-4 py-3">
                        <span class="px-2 py-1 rounded-full text-xs font-semibold" style=format!("background:{color}1A; color:{color};")>
                            {label}
                        </span>
                    </td>
                    <td class="px-4 py-3 text-sm font-mono" style="color:var(--uci-text);">{i.code}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-text);">{i.patient_ref}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{i.detail}</td>
                    <td class="px-4 py-3 text-xs" style="color:var(--uci-muted);">{i.message_id}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="rounded-xl overflow-x-auto" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <table class="w-full">
                <thead style="background:var(--uci-bg);">
                    <tr>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{crate::i18n::tr("dq-col-date", None)}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{crate::i18n::tr("dq-col-severity", None)}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{crate::i18n::tr("dq-col-code", None)}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{crate::i18n::tr("dq-col-patient", None)}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{crate::i18n::tr("dq-col-detail", None)}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{crate::i18n::tr("dq-col-message", None)}</th>
                    </tr>
                </thead>
                <tbody class="divide-y" style="border-color:var(--uci-border);">
                    {rows}
                </tbody>
            </table>
        </div>
    }.into_any()
}
