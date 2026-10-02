use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::api::{AuditExport, AuditLogEntry, ScoreAuditEntry};
use crate::components::ui_kit::{ErrorState, LoadingState};
use crate::stores::user_has;

/// Color por `AuditAction` (serde usa el nombre de la variante tal cual).
fn action_color(action: &str) -> &'static str {
    match action {
        "LoginFailed" | "LogoutFailed" | "AccessDenied" => "#DC2626",
        "Login" | "Logout" => "#64748B",
        "Create" | "Update" | "Delete" | "DataModification" => "#D97706",
        "Export" => "#8B5CF6",
        "ConfigChange" | "AuthChange" => "#EC4899",
        "Read" | "DataAccess" => "#3B82F6",
        _ => "#94A3B8",
    }
}

fn action_label(action: &str) -> &'static str {
    match action {
        "Login" => "Login",
        "Logout" => "Logout",
        "LoginFailed" => "Login fallido",
        "LogoutFailed" => "Logout fallido",
        "Create" => "Creación",
        "Read" => "Lectura",
        "Update" => "Actualización",
        "Delete" => "Eliminación",
        "Export" => "Exportación",
        "ConfigChange" => "Cambio de config",
        "AuthChange" => "Cambio de auth",
        "AccessDenied" => "Acceso denegado",
        "DataAccess" => "Acceso a datos",
        "DataModification" => "Modificación de datos",
        _ => "Acción",
    }
}

#[component]
pub fn AuditPage() -> impl IntoView {
    let reload = RwSignal::new(0u32);
    let feedback = RwSignal::new(Option::<(bool, String)>::None);
    let busy = RwSignal::new(false);
    let confirm_cleanup = RwSignal::new(false);
    let can_configure = user_has("config:write");

    let critical = LocalResource::new(move || {
        let _ = reload.get();
        async move { api::audit_critical(50).await }
    });

    let scores = LocalResource::new(move || {
        let _ = reload.get();
        async move { api::verify_scores().await }
    });

    // El export se pide bajo demanda (hasta 10k eventos) y se muestra en un
    // modal en lugar de cargarlo siempre.
    let export = RwSignal::new(Option::<AuditExport>::None);
    let load_export = move |_| {
        busy.set(true);
        feedback.set(None);
        spawn_local(async move {
            match api::export_audit(200).await {
                Ok(e) => {
                    feedback.set(Some((
                        true,
                        format!(
                            "Export generado: {} eventos, {} lotes, head {}.",
                            e.logs.len(),
                            e.batches.len(),
                            &e.head_batch_hash.chars().take(12).collect::<String>()
                        ),
                    )));
                    export.set(Some(e));
                }
                Err(e) => feedback.set(Some((false, e))),
            }
            busy.set(false);
        });
    };

    // Destructivo: borra los logs fuera de retención. Doble confirmación.
    let run_cleanup = move |_| {
        if !confirm_cleanup.get() {
            confirm_cleanup.set(true);
            return;
        }
        busy.set(true);
        feedback.set(None);
        spawn_local(async move {
            match api::run_audit_retention_cleanup().await {
                Ok(r) => {
                    feedback.set(Some((
                        true,
                        format!(
                            "Retención aplicada ({} años): {} registros eliminados.",
                            r.retention_years, r.deleted_logs
                        ),
                    )));
                    reload.update(|x| *x += 1);
                }
                Err(e) => feedback.set(Some((false, e))),
            }
            confirm_cleanup.set(false);
            busy.set(false);
        });
    };

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-center justify-between gap-4 mb-6">
                <div>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-clipboard-check mr-2"></i>"Auditoría forense"
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        "Cadena WORM encadenada por hash, eventos críticos y reproducibilidad de scores (SPEC-049 / SPEC-029)"
                    </p>
                </div>
                <div class="flex flex-wrap gap-2">
                    <button
                        class="px-4 h-10 text-sm rounded-lg"
                        style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                        on:click=move |_| reload.update(|n| *n += 1)
                    >
                        <i class="fa-solid fa-rotate mr-2"></i>"Actualizar"
                    </button>
                    <button
                        class="px-4 h-10 text-sm rounded-lg"
                        style="background:var(--uci-surface); color:var(--uci-accent); border:1px solid var(--uci-border);"
                        on:click=load_export
                        disabled=move || busy.get()
                    >
                        <i class="fa-solid fa-file-export mr-2"></i>"Exportar"
                    </button>
                    <Show when=move || can_configure>
                        <button
                            class="px-4 h-10 text-sm rounded-lg"
                            style=if confirm_cleanup.get() {
                                "background:#DC2626; color:#fff; border:1px solid #DC2626;"
                            } else {
                                "background:var(--uci-surface); color:var(--uci-critical); border:1px solid var(--uci-border);"
                            }
                            on:click=run_cleanup
                            disabled=move || busy.get()
                        >
                            <i class="fa-solid fa-broom mr-2"></i>
                            {move || if confirm_cleanup.get() {
                                "¿Confirmar borrado? Pulsa otra vez"
                            } else {
                                "Aplicar retención"
                            }}
                        </button>
                    </Show>
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

            <h2 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-triangle-exclamation mr-2" style="color:#DC2626;"></i>
                "Eventos críticos"
            </h2>
            <Suspense fallback=move || view! { <LoadingState label="Cargando eventos críticos..." /> }>
                {move || match critical.get() {
                    Some(Ok(logs)) => view! { <CriticalTable logs=logs /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudieron cargar los eventos críticos: {}", e)
                            on_retry=Some(Callback::new(move |()| reload.update(|n| *n += 1)))
                        />
                    }
                    .into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>

            <h2 class="text-sm font-bold uppercase mt-6 mb-3" style="color:var(--uci-text);">
                <i class="fa-solid fa-fingerprint mr-2" style="color:#8B5CF6;"></i>
                "Reproducibilidad de scores"
            </h2>
            <Suspense fallback=move || view! { <LoadingState label="Verificando scores..." /> }>
                {move || match scores.get() {
                    Some(Ok(entries)) => view! { <ScoresTable entries=entries /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo verificar la reproducibilidad: {}", e)
                            on_retry=Some(Callback::new(move |()| reload.update(|n| *n += 1)))
                        />
                    }
                    .into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>

            {move || export.get().map(|e| view! { <ExportModal export=e /> })}
        </div>
    }
}

fn rid_flag(log: &AuditLogEntry) -> bool {
    log.resource_id.is_some()
}

fn short_hash(log: &AuditLogEntry) -> String {
    log.content_hash
        .clone()
        .map(|h| h.chars().take(10).collect::<String>())
        .unwrap_or_else(|| "—".into())
}

fn fp_cell(e: &ScoreAuditEntry) -> String {
    if e.fingerprint_guardado == e.fingerprint_recalculado {
        e.fingerprint_guardado.clone()
    } else {
        format!(
            "{} != {}",
            &e.fingerprint_guardado.chars().take(8).collect::<String>(),
            &e.fingerprint_recalculado
                .chars()
                .take(8)
                .collect::<String>()
        )
    }
}

#[component]
fn CriticalTable(logs: Vec<AuditLogEntry>) -> impl IntoView {
    if logs.is_empty() {
        return view! {
            <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                <i class="fa-solid fa-shield-halved text-2xl mb-2" style="color:#10B981;"></i>
                <p>"Sin eventos críticos. Ningún acceso denegado ni fallo de autenticación."</p>
            </div>
        }
        .into_any();
    }

    let logs = StoredValue::new(logs);

    view! {
        <div class="glass-card overflow-x-auto">
            <table class="w-full text-sm" style="color:var(--uci-text);">
                <thead>
                    <tr style="border-bottom:1px solid var(--uci-border);">
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            "Timestamp"
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            "Acción"
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            "Usuario"
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            "Recurso"
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            "IP"
                        </th>
                        <th class="text-left px-4 py-3 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                            "Hash"
                        </th>
                    </tr>
                </thead>
                <tbody>
                    <For each=move || logs.get_value() key=|l| l.uid.clone() let:log>
                        {
                            let has_rid = rid_flag(&log);
                            let rid = log.resource_id.clone().unwrap_or_default();
                            let hash = short_hash(&log);
                            view! {
                        <tr style="border-bottom:1px solid var(--uci-border);">
                            <td class="px-4 py-3 text-xs tabular-nums" style="color:var(--uci-muted); white-space:nowrap;">
                                {log.timestamp.clone()}
                            </td>
                            <td class="px-4 py-3">
                                <span
                                    class="inline-flex px-2 py-0.5 rounded text-[10px] font-bold uppercase"
                                    style=format!(
                                        "background:color-mix(in srgb, {} 15%, transparent); color:{};",
                                        action_color(&log.action),
                                        action_color(&log.action),
                                    )
                                >
                                    {action_label(&log.action)}
                                </span>
                            </td>
                            <td class="px-4 py-3 text-xs">
                                {log.username.clone().unwrap_or_else(|| "—".into())}
                            </td>
                            <td class="px-4 py-3 text-xs" style="color:var(--uci-muted);">
                                {log.resource.clone()}
                                <Show when=move || has_rid>
                                    <span style="font-family:var(--uci-font-mono);">
                                        "/"
                                        {rid.clone()}
                                    </span>
                                </Show>
                            </td>
                            <td class="px-4 py-3 text-xs tabular-nums" style="color:var(--uci-muted);">
                                {log.ip_address.clone().unwrap_or_else(|| "—".into())}
                            </td>
                            <td
                                class="px-4 py-3 text-[10px]"
                                style="color:var(--uci-muted); font-family:var(--uci-font-mono);"
                            >
                                {hash.clone()}
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
fn ScoresTable(entries: Vec<ScoreAuditEntry>) -> impl IntoView {
    let (ok_count, bad_count) = {
        let ok = entries.iter().filter(|e| e.reproducible).count();
        (ok, entries.len() - ok)
    };

    let has_rows = !entries.is_empty();
    let entries = StoredValue::new(entries);

    view! {
        <div class="glass-card p-5">
            <div class="flex flex-wrap items-center gap-3 mb-4">
                <span
                    class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold uppercase"
                    style="background:rgba(16,185,129,0.14); color:#10B981; border:1px solid rgba(16,185,129,0.4);"
                >
                    <i class="fa-solid fa-circle-check text-[10px]"></i>
                    <span class="tabular-nums">{ok_count}</span>
                    " reproducibles"
                </span>
                <Show when=move || bad_count != 0>
                    <span
                        class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold uppercase"
                        style="background:rgba(220,38,38,0.14); color:#DC2626; border:1px solid rgba(220,38,38,0.4);"
                    >
                        <i class="fa-solid fa-circle-exclamation text-[10px]"></i>
                        <span class="tabular-nums">{bad_count}</span>
                        " no reproducibles"
                    </span>
                </Show>
                <span class="text-xs ml-auto" style="color:var(--uci-muted);">
                    "Se recalcula el fingerprint desde los datos crudos y se compara con el almacenado."
                </span>
            </div>

            <Show
                when=move || has_rows
                fallback=|| view! {
                    <p class="text-sm py-4 text-center" style="color:var(--uci-muted);">
                        "No hay mediciones registradas para verificar."
                    </p>
                }
            >
                <div class="overflow-x-auto max-h-96 overflow-y-auto">
                    <table class="w-full text-sm" style="color:var(--uci-text);">
                        <thead>
                            <tr style="border-bottom:1px solid var(--uci-border);">
                                <th class="text-left px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                    "Paciente"
                                </th>
                                <th class="text-left px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                    "Timestamp"
                                </th>
                                <th class="text-left px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                    "Algoritmo"
                                </th>
                                <th class="text-left px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                    "Fingerprint"
                                </th>
                                <th class="text-left px-3 py-2 text-xs font-bold uppercase" style="color:var(--uci-muted);">
                                    "Estado"
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            <For each=move || entries.get_value() key=|e| e.measurement_id.clone() let:e>
                                <tr style="border-bottom:1px solid var(--uci-border);">
                                    <td
                                        class="px-3 py-2 text-xs truncate"
                                        style="font-family:var(--uci-font-mono); max-width:160px;"
                                    >
                                        {e.patient_id.clone()}
                                    </td>
                                    <td class="px-3 py-2 text-xs tabular-nums" style="color:var(--uci-muted); white-space:nowrap;">
                                        {e.timestamp.clone()}
                                    </td>
                                    <td class="px-3 py-2 text-xs" style="color:var(--uci-muted);">
                                        {e.algorithm_version.clone()}
                                    </td>
                                    <td class="px-3 py-2 text-[10px]" style="font-family:var(--uci-font-mono); color:var(--uci-muted);">
                                        {fp_cell(&e)}
                                    </td>
                                    <td class="px-3 py-2">
                                        <Show
                                            when=move || e.reproducible
                                            fallback=|| view! { <span class="severity-critical">"Alterado"</span> }
                                        >
                                            <span class="severity-low">"Íntegro"</span>
                                        </Show>
                                    </td>
                                </tr>
                            </For>
                        </tbody>
                    </table>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn ExportModal(export: AuditExport) -> impl IntoView {
    let close = RwSignal::new(false);
    let payload = serde_json::json!({
        "generated_at": export.generated_at,
        "retention_years": export.retention_years,
        "head_batch_hash": export.head_batch_hash,
        "batches": export.batches.len(),
        "logs": export.logs.len(),
    })
    .to_string();
    let payload = StoredValue::new(payload);
    let copy = move |_| {
        let text = payload.get_value();
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(&text);
        }
    };

    view! {
        <Show when=move || !close.get() fallback=|| ()>
            <div
                style="position:fixed; inset:0; z-index:50; display:flex; align-items:center; justify-content:center; background:rgba(0,0,0,0.55);"
                role="dialog"
                aria-modal="true"
                aria-label="Export de auditoría"
            >
                <div class="glass-card" style="max-width:720px; width:92%; padding:24px;">
                    <h3 class="text-lg font-bold mb-1" style="color:var(--uci-text);">
                        <i class="fa-solid fa-file-export mr-2"></i>"Export de auditoría"
                    </h3>
                    <p class="text-xs mb-4" style="color:var(--uci-muted);">
                        "Snapshot encadenado de la cadena WORM para custodia externa."
                    </p>
                    <dl class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
                        <Stat label="Generado" value=export.generated_at.clone() />
                        <Stat label="Retención (años)" value=export.retention_years.to_string() />
                        <Stat label="Eventos" value=export.logs.len().to_string() />
                        <Stat label="Lotes" value=export.batches.len().to_string() />
                    </dl>
                    <div class="mb-4">
                        <div class="text-[10px] uppercase font-bold mb-1" style="color:var(--uci-muted);">
                            "Head batch hash"
                        </div>
                        <div
                            class="text-[11px] break-all p-2 rounded"
                            style="background:var(--uci-bg); color:var(--uci-text); font-family:var(--uci-font-mono);"
                        >
                            {export.head_batch_hash.clone()}
                        </div>
                    </div>
                    <div class="flex justify-end gap-2">
                        <button
                            class="px-4 h-9 text-sm rounded-lg"
                            style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                            on:click=copy
                        >
                            <i class="fa-solid fa-copy mr-1"></i>"Copiar resumen"
                        </button>
                        <button
                            class="btn-primary px-4 h-9 text-sm"
                            on:click=move |_| close.set(true)
                        >
                            "Cerrar"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}

#[component]
fn Stat(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="p-3 rounded-lg" style="background:var(--uci-bg); border:1px solid var(--uci-border);">
            <dt class="text-[10px] uppercase font-bold" style="color:var(--uci-muted);">
                {label}
            </dt>
            <dd class="text-sm font-semibold mt-0.5 truncate tabular-nums" style="color:var(--uci-text);">
                {value}
            </dd>
        </div>
    }
}
