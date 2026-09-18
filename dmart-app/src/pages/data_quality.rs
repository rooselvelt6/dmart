use leptos::prelude::*;

use crate::api;
use crate::api::{QualityIssue, QualitySummary};
use crate::components::ui_kit::{ErrorState, LoadingState};

fn severity_meta(sev: &str) -> (&'static str, &'static str) {
    match sev {
        "high" => ("#EF4444", "Alta"),
        "medium" => ("#F59E0B", "Media"),
        "low" => ("#3B82F6", "Baja"),
        _ => ("#6B7280", "Desconocida"),
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
                        <i class="fa-solid fa-shield-heart mr-2"></i>"Calidad de Datos"
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        "Issues de ingest HL7/MLLP: valores fuera de rango, timestamps y secuencia"
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

            <Suspense fallback=move || view! { <LoadingState label="Calculando resumen de calidad..." /> }>
                {move || match summary.get() {
                    Some(Ok(s)) => view! { <Summary summary=s /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo cargar el resumen: {}", e)
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>

            <h2 class="text-sm font-bold uppercase mt-6 mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-list-check mr-2"></i>"Últimos issues"
            </h2>
            <Suspense fallback=move || view! { <LoadingState label="Cargando issues..." /> }>
                {move || match report.get() {
                    Some(Ok(issues)) => view! { <IssuesTable issues=issues /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo cargar el reporte: {}", e)
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => view! {}.into_any(),
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
                <div class="text-xs uppercase font-semibold" style="color:var(--uci-muted);">"Total"</div>
                <div class="text-2xl font-bold mt-1" style="color:var(--uci-text);">{summary.total}</div>
            </div>
            {sevs}
        </div>
        <div class="glass-card p-5 mt-4">
            <h3 class="text-xs font-bold uppercase mb-2" style="color:var(--uci-text);">"Por código"</h3>
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
                <p>"Sin issues de calidad registrados"</p>
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
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Fecha"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Severidad"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Código"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Paciente"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Detalle"</th>
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
