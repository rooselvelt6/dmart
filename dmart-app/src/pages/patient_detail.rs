use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use crate::api;
use crate::components::severity_badge::SeverityBadge;
use crate::components::chart::EvolutionChart;

fn show_alert(msg: &str) {
    web_sys::window()
        .and_then(|w| w.alert_with_message(msg).ok());
}

#[component]
pub fn PatientDetailPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.get().get("id").unwrap_or_default();

    let patient_res = LocalResource::new(move || {
        let pid = id();
        async move { api::get_patient(&pid).await }
    });

    let measurements_res = LocalResource::new(move || {
        let pid = id();
        async move { api::get_measurements(&pid).await }
    });

    let show_delete_modal = RwSignal::new(false);
    let deleting = RwSignal::new(false);
    let navigate = use_navigate();

    let do_delete = StoredValue::new(move || {
        let pid = id();
        deleting.set(true);
        let nav = navigate.clone();
        spawn_local(async move {
            match api::delete_patient(&pid).await {
                Ok(_) => {
                    deleting.set(false);
                    nav("/patients", Default::default());
                },
                Err(e) => {
                    deleting.set(false);
                    show_delete_modal.set(false);
                    show_alert(&format!("Error al eliminar: {}", e));
                }
            }
        });
    });

    view! {
        <div class="page-enter">
            <Suspense fallback=move || view! { <div class="p-10 text-uci-muted">"Cargando detalles..."</div> }>
                {move || patient_res.get().map(|res_wrapper| match &*res_wrapper {
                    Ok(p_wrapper) => {
                        let p = p_wrapper.clone();
                        Either::Left(view! {
                            <div class="max-w-5xl mx-auto">
                                // Header with main info and actions
                                <div class="flex justify-between items-start mb-8">
                                    <div>
                                        <div class="flex items-center gap-4 mb-2">
                                            <h1 class="text-3xl font-extrabold text-uci-text">{p.nombre_completo()}</h1>
                                            <SeverityBadge level=p.estado_gravedad.clone() />
                                        </div>
                                        <div class="flex gap-6 text-sm text-uci-muted font-medium">
                                            <span>"CI: " {p.cedula.clone()}</span>
                                            <span>"HC: " {p.historia_clinica.clone()}</span>
                                            <span>"Ingreso UCI: " {p.fecha_ingreso_uci[..10].to_string()}</span>
                                        </div>
                                    </div>
                                    <div class="flex gap-3">
                                        <a href=format!("/patients/{}/edit", p.patient_id) class="btn-outline flex items-center gap-2">
                                            "✏️ Editar"
                                        </a>
                                        <a href=format!("/api/patients/{}/export/pdf", p.patient_id) target="_blank" class="btn-outline flex items-center gap-2">
                                            "📄 PDF"
                                        </a>
                                        <a href=format!("/api/patients/{}/export/csv", p.patient_id) class="btn-outline flex items-center gap-2">
                                            "📊 CSV"
                                        </a>
                                        <button on:click=move |_| show_delete_modal.set(true) class="btn-danger flex items-center gap-2">
                                            "🗑️ Eliminar"
                                        </button>
                                        <div class="flex flex-wrap gap-2">
                                        <a href=format!("/patients/{}/measure?escala=apache", p.patient_id) class="btn-primary text-xs py-2 px-3">
                                            "🏥 APACHE II"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=gcs", p.patient_id) class="btn-outline text-xs py-2 px-3">
                                            "🧠 GCS"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=saps3", p.patient_id) class="btn-outline text-xs py-2 px-3">
                                            "📊 SAPS III"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=news2", p.patient_id) class="btn-outline text-xs py-2 px-3">
                                            "🚨 NEWS2"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=sofa", p.patient_id) class="btn-outline text-xs py-2 px-3">
                                            "🫀 SOFA"
                                        </a>
                                    </div>
                                    </div>
                                </div>

                                <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-10">
                                    // Patient info Card
                                    <div class="glass-card p-6 md:col-span-1">
                                        <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest mb-4">"Información Personal"</h3>
                                        <InfoItem label="Sexo" value=format!("{:?}", p.sexo) />
                                        <InfoItem label="Nacionalidad" value=format!("{:?}", p.nacionalidad) />
                                        <InfoItem label="Piel" value=p.color_piel.label().to_string() />
                                        <InfoItem label="Lugar Nac." value=p.lugar_nacimiento.clone() />
                                        <InfoItem label="Dirección" value=p.direccion.clone() />
                                        <InfoItem label="Familiar Responsable" value=p.familiar_encargado.clone() />
                                    </div>

                                    // Clinical Status Card
                                    <div class="glass-card p-6 md:col-span-2">
                                        <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest mb-4">"Estado Clínico de Ingreso"</h3>
                                        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                                            <div>
                                                <InfoItem label="Diag. Hospital" value=p.diagnostico_hospital.clone() />
                                                <InfoItem label="Diag. UCI" value=p.diagnostico_uci.clone() />
                                                <InfoItem label="Admisión" value=format!("{:?}", p.tipo_admision) />
                                            </div>
                                            <div>
                                                <InfoItem label="Ventilación" value=if p.ventilacion_mecanica { "Sí".into() } else { "No".into() } />
                                                <InfoItem label="Procedencia" value=if p.migracion_otro_centro { p.centro_origen.clone().unwrap_or("Otro centro".into()) } else { "Directa".into() } />
                                                <div class="mb-4">
                                                    <label class="form-label">"Procesos Invasivos"</label>
                                                    <div class="flex flex-wrap gap-2 mt-1">
                                                        {p.procesos_invasivos.iter().map(|pr| {
                                                            let pr_str = pr.to_string();
                                                            view! {
                                                                <span class="px-2 py-0.5 bg-uci-surface border border-uci-border rounded text-[11px] text-uci-text">
                                                                    {pr_str}
                                                                </span>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="mt-4 pt-4 border-t border-uci-border">
                                            <InfoItem label="Antecedentes" value=p.antecedentes.clone() />
                                        </div>
                                    </div>
                                </div>

                                // Evolution Chart Card
                                <div class="glass-card p-8 mb-10">
                                    <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest mb-6">"Evolución del Score Apache II"</h3>
                                    <Suspense fallback=move || view! { <div class="h-40 flex items-center justify-center text-uci-muted">"Cargando evolución..."</div> }>
                                        {move || measurements_res.get().map(|res_wrapper| match &*res_wrapper {
                                            Ok(ms) if ms.is_empty() => Either::Left(view! { 
                                                <div class="h-40 flex flex-col items-center justify-center text-uci-muted bg-uci-bg/30 rounded-xl">
                                                    <span class="text-2xl mb-2">"📉"</span>
                                                    "No hay suficientes datos para generar la gráfica"
                                                </div>
                                            }),
                                            Ok(ms) => Either::Right(Either::Left(view! { <EvolutionChart measurements=ms.clone() height=250 /> })),
                                            Err(_) => Either::Right(Either::Right(view! { <div class="text-uci-critical">"Error cargando mediciones"</div> })),
                                        })}
                                    </Suspense>
                                </div>

                                // History Table
                                <div class="glass-card overflow-hidden">
                                    <div class="px-6 py-4 border-bottom border-uci-border">
                                        <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest">"Historial de Mediciones"</h3>
                                    </div>
                                    <div class="overflow-x-auto">
                                        <table class="w-full text-left border-collapse">
                                            <thead class="bg-uci-surface/50 text-[11px] font-bold text-uci-muted uppercase tracking-wider">
                                                <tr>
                                                    <th class="px-6 py-4 border-b border-uci-border">"Fecha / Hora"</th>
                                                    <th class="px-6 py-4 border-b border-uci-border">"Apache II"</th>
                                                    <th class="px-6 py-4 border-b border-uci-border">"GCS"</th>
                                                    <th class="px-6 py-4 border-b border-uci-border">"Estado"</th>
                                                    <th class="px-6 py-4 border-b border-uci-border">"Riesgo"</th>
                                                    <th class="px-6 py-4 border-b border-uci-border">"Notas"</th>
                                                </tr>
                                            </thead>
                                            <tbody class="text-sm">
                                                <Suspense>
                                                    {move || measurements_res.get().map(|res_wrapper| match &*res_wrapper {
                                                        Ok(ms) => Either::Left(ms.iter().map(|m| {
                                                            let timestamp = m.timestamp.clone();
                                                            let score = m.apache_score;
                                                            let gcs = m.gcs_score;
                                                            let severity = m.severity.clone();
                                                            let risk = m.mortality_risk;
                                                            let notas = m.notas.clone();
                                                            view! {
                                                                <tr class="hover:bg-uci-accent/5 border-b border-uci-border transition-colors">
                                                                    <td class="px-6 py-4 font-mono text-xs text-uci-text">{timestamp[..16].to_string()}</td>
                                                                    <td class="px-6 py-4 font-extrabold text-uci-text">{score}</td>
                                                                    <td class="px-6 py-4 font-bold text-uci-accent">{gcs} "/15"</td>
                                                                    <td class="px-6 py-4">
                                                                        <SeverityBadge level=severity />
                                                                    </td>
                                                                    <td class="px-6 py-4 font-bold text-uci-severe">{format!("{:.1}%", risk)}</td>
                                                                    <td class="px-6 py-4 text-xs text-uci-muted italic max-w-xs truncate">{notas}</td>
                                                                </tr>
                                                            }
                                                        }).collect_view()),
                                                        Err(_) => Either::Right(view! { <tr><td colspan="6" class="p-10 text-center text-uci-critical">"Error"</td></tr> }),
                                                    })}
                                                </Suspense>
                                            </tbody>
                                        </table>
                                    </div>
                                </div>
                            </div>
                        })
                    },
                    _ => Either::Right(view! { <div class="text-center p-20 text-uci-critical">"Paciente no encontrado"</div> }),
                })}
            </Suspense>

            // Delete Confirmation Modal
            <Show when=move || show_delete_modal.get()>
                <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 backdrop-blur-sm">
                    <div class="glass-card p-8 max-w-md mx-4">
                        <h3 class="text-xl font-bold text-uci-text mb-4">"Confirmar Eliminación"</h3>
                        <p class="text-uci-muted mb-6">"¿Está seguro de eliminar este paciente? Esta acción no se puede deshacer y se eliminarán todas las mediciones asociadas."</p>
                        <div class="flex gap-3 justify-end">
                            <button 
                                on:click=move |_| show_delete_modal.set(false)
                                class="btn-outline"
                                disabled=deleting
                            >
                                "Cancelar"
                            </button>
                            <button 
                                on:click=move |_| do_delete.with_value(|f| f())
                                class="btn-danger"
                                disabled=deleting
                            >
                                {move || if deleting.get() { "Eliminando..." } else { "Eliminar Paciente" }}
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn InfoItem(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="mb-4">
            <label class="form-label">{label}</label>
            <div class="text-[14px] text-uci-text font-medium">{if value.is_empty() { "—".into() } else { value }}</div>
        </div>
    }
}
