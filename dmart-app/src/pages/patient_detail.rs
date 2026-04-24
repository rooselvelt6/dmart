use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use crate::api;
use crate::components::severity_badge::SeverityBadge;
use crate::components::chart::EvolutionChart;

fn show_alert(msg: &str) {
    web_sys::window()
        .and_then(|w| w.alert_with_message(msg).ok());
}

fn get_initials(nombre: &str) -> String {
    nombre.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
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
                {move || patient_res.get().map(|res_wrapper| match res_wrapper {
                    Ok(p_wrapper) => {
                        let p = p_wrapper.clone();
                        Either::Left(view! {
                            <div class="max-w-6xl mx-auto">
                                <div class="glass-card p-6 mb-6">
                                    <div class="flex items-center justify-between">
                                        <div class="flex items-center gap-5">
                                            <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-uci-accent to-uci-primary flex items-center justify-center shadow-lg">
                                                <span class="text-2xl font-bold text-white">{get_initials(&p.nombre_completo())}</span>
                                            </div>
                                            <div>
                                                <div class="flex items-center gap-3 mb-1">
                                                    <h1 class="text-2xl font-extrabold text-uci-text">{p.nombre_completo()}</h1>
                                                    <SeverityBadge level=p.estado_gravedad.clone() />
                                                </div>
                                                <div class="flex items-center gap-4 text-sm text-uci-muted">
                                                    <span class="flex items-center gap-1"><i class="fa-solid fa-id-card text-xs"></i>{p.cedula.clone()}</span>
                                                    <span class="flex items-center gap-1"><i class="fa-solid fa-folder text-xs"></i>{p.historia_clinica.clone()}</span>
                                                    <span class="flex items-center gap-1"><i class="fa-solid fa-hospital text-xs"></i>{p.fecha_ingreso_uci[..10].to_string()}</span>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="flex items-center gap-2">
                                            <a href=format!("/patients/{}/edit", p.patient_id) class="btn-outline p-2" title="Editar">
                                                <i class="fa-solid fa-pen"></i>
                                            </a>
                                            <a href=format!("/api/patients/{}/export/pdf", p.patient_id) target="_blank" class="btn-outline p-2" title="PDF">
                                                <i class="fa-solid fa-file-pdf"></i>
                                            </a>
                                            <a href=format!("/api/patients/{}/export/csv", p.patient_id) class="btn-outline p-2" title="CSV">
                                                <i class="fa-solid fa-file-csv"></i>
                                            </a>
                                            <button on:click=move |_| show_delete_modal.set(true) class="btn-danger p-2" title="Eliminar">
                                                <i class="fa-solid fa-trash"></i>
                                            </button>
                                        </div>
                                    </div>
                                    <div class="flex flex-wrap gap-2 mt-4 pt-4 border-t border-uci-border">
                                        <a href=format!("/patients/{}/measure?escala=apache", p.patient_id) class="scale-chip">
                                            <i class="fa-solid fa-heart-pulse"></i>"APACHE II"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=gcs", p.patient_id) class="scale-chip">
                                            <i class="fa-solid fa-brain"></i>"GCS"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=saps3", p.patient_id) class="scale-chip">
                                            <i class="fa-solid fa-chart-line"></i>"SAPS III"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=news2", p.patient_id) class="scale-chip">
                                            <i class="fa-solid fa-bell"></i>"NEWS2"
                                        </a>
                                        <a href=format!("/patients/{}/measure?escala=sofa", p.patient_id) class="scale-chip">
                                            <i class="fa-solid fa-lungs"></i>"SOFA"
                                        </a>
                                    </div>
                                </div>

                                <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
                                    <div class="glass-card p-4 text-center">
                                        <i class="fa-solid fa-heart-pulse text-rose-500 text-xl mb-2"></i>
                                        <div class="text-2xl font-bold text-uci-text">"APACHE"</div>
                                        <div class="text-xs text-uci-muted">"Score de Severidad"</div>
                                        <div class="text-lg font-bold text-rose-500 mt-1">{p.ultimo_apache_score.map(|s| s.to_string()).unwrap_or("-".into())}</div>
                                    </div>
                                    <div class="glass-card p-4 text-center">
                                        <i class="fa-solid fa-brain text-purple-500 text-xl mb-2"></i>
                                        <div class="text-2xl font-bold text-uci-text">"GCS"</div>
                                        <div class="text-xs text-uci-muted">"Escala de Coma"</div>
                                        <div class="text-lg font-bold text-purple-500 mt-1">{p.ultimo_gcs_score.map(|s| s.to_string()).unwrap_or("-".into())}"/15"</div>
                                    </div>
                                    <div class="glass-card p-4 text-center">
                                        <i class="fa-solid fa-lungs text-emerald-500 text-xl mb-2"></i>
                                        <div class="text-2xl font-bold text-uci-text">"SOFA"</div>
                                        <div class="text-xs text-uci-muted">"Falla Orgánica"</div>
                                        <div class="text-lg font-bold text-emerald-500 mt-1">{p.ultimo_sofa_score.map(|s| s.to_string()).unwrap_or("-".into())}</div>
                                    </div>
                                    <div class="glass-card p-4 text-center">
                                        <i class="fa-solid fa-skull text-red-600 text-xl mb-2"></i>
                                        <div class="text-2xl font-bold text-uci-text">"Riesgo"</div>
                                        <div class="text-xs text-uci-muted">"Mortalidad UCI"</div>
                                        <div class="text-lg font-bold text-red-600 mt-1">{p.mortality_risk.map(|m| format!("{:.0}%", m)).unwrap_or("-".into())}</div>
                                    </div>
                                </div>

                                <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
                                    <div class="glass-card p-5">
                                        <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest mb-4 flex items-center gap-2">
                                            <i class="fa-solid fa-user"></i>"Datos del Paciente"
                                        </h3>
                                        <div class="space-y-3">
                                            <InfoRow icon="fa-venus-mars" label="Sexo" value=format!("{:?}", p.sexo) />
                                            <InfoRow icon="fa-flag" label="Nacionalidad" value=format!("{:?}", p.nacionalidad) />
                                            <InfoRow icon="fa-palette" label="Color Piel" value=p.color_piel.label().to_string() />
                                            <InfoRow icon="fa-location-dot" label="Lugar de Nac." value=p.lugar_nacimiento.clone() />
                                            <InfoRow icon="fa-house" label="Dirección" value=p.direccion.clone() />
                                            <InfoRow icon="fa-user-shield" label="Familiar" value=p.familiar_encargado.clone() />
                                        </div>
                                    </div>

                                    <div class="glass-card p-5">
                                        <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest mb-4 flex items-center gap-2">
                                            <i class="fa-solid fa-stethoscope"></i>"Estado Clínico"
                                        </h3>
                                        <div class="space-y-3">
                                            <InfoRow icon="fa-hospital" label="Diag. Hospital" value=p.diagnostico_hospital.clone() />
                                            <InfoRow icon="fa-truck-medical" label="Diag. UCI" value=p.diagnostico_uci.clone() />
                                            <InfoRow icon="fa-door-open" label="Admisión" value=format!("{:?}", p.tipo_admision) />
                                            <InfoRow icon="fa-mask-ventilator" label="Ventilación" value=if p.ventilacion_mecanica { "Sí".into() } else { "No".into() } />
                                            <InfoRow icon="fa-building-arrow-turn-right" label="Procedencia" value=if p.migracion_otro_centro { p.centro_origen.clone().unwrap_or("Otro centro".into()) } else { "Directa".into() } />
                                        </div>
                                        <div class="mt-4 pt-3 border-t border-uci-border">
                                            <div class="text-xs text-uci-muted mb-2">"Procesos Invasivos"</div>
                                            <div class="flex flex-wrap gap-1">
                                                {p.procesos_invasivos.iter().map(|pr| {
                                                    let pr_str = pr.to_string();
                                                    view! {
                                                        <span class="px-2 py-0.5 bg-uci-surface border border-uci-border rounded text-[11px] text-uci-text">
                                                            {pr_str}
                                                        </span>
                                                    }
                                                }).collect_view()}
                                                {if p.procesos_invasivos.is_empty() {
                                                    Either::Left(view! { <span class="text-xs text-uci-muted">"Ninguno"</span> })
                                                } else { Either::Right(view! { "" }) }}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div class="glass-card p-5 mb-6">
                                    <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest mb-4 flex items-center gap-2">
                                        <i class="fa-solid fa-notes-medical"></i>"Antecedentes Clínicos"
                                    </h3>
                                    <p class="text-sm text-uci-text leading-relaxed">{if p.antecedentes.is_empty() { "Sin antecedentes registrados".into() } else { p.antecedentes.clone() }}</p>
                                </div>

                                <div class="glass-card p-6 mb-6">
                                    <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest mb-5 flex items-center gap-2">
                                        <i class="fa-solid fa-chart-line"></i>"Evolución del APACHE II"
                                    </h3>
                                    <Suspense fallback=move || view! { <div class="h-48 flex items-center justify-center text-uci-muted"><i class="fa-solid fa-spinner fa-spin text-xl"></i>" Cargando..."</div> }>
                                        {move || measurements_res.get().map(|res_wrapper| match res_wrapper {
                                            Ok(ms) if ms.is_empty() => Either::Left(view! { 
                                                <div class="h-48 flex flex-col items-center justify-center text-uci-muted bg-uci-bg/30 rounded-xl">
                                                    <i class="fa-solid fa-chart-simple text-3xl mb-3 opacity-50"></i>
                                                    "No hay suficientes datos para la gráfica"
                                                </div>
                                            }),
                                            Ok(ms) => Either::Right(Either::Left(view! { <EvolutionChart measurements=ms.clone() height=220 /> })),
                                            Err(_) => Either::Right(Either::Right(view! { <div class="text-uci-critical">"Error cargando mediciones"</div> })),
                                        })}
                                    </Suspense>
                                </div>

                                <div class="glass-card overflow-hidden">
                                    <div class="px-6 py-4 border-b border-uci-border flex items-center justify-between">
                                        <h3 class="text-xs font-bold text-uci-accent uppercase tracking-widest flex items-center gap-2">
                                            <i class="fa-solid fa-list"></i>"Historial de Mediciones"
                                        </h3>
                                        <span class="text-xs text-uci-muted">"registros"</span>
                                    </div>
                                    <div class="overflow-x-auto">
                                        <table class="w-full text-left border-collapse">
                                            <thead class="bg-uci-surface/50 text-[11px] font-bold text-uci-muted uppercase tracking-wider">
                                                <tr>
                                                    <th class="px-5 py-3 border-b border-uci-border">"Fecha / Hora"</th>
                                                    <th class="px-5 py-3 border-b border-uci-border">"APACHE"</th>
                                                    <th class="px-5 py-3 border-b border-uci-border">"GCS"</th>
                                                    <th class="px-5 py-3 border-b border-uci-border">"Estado"</th>
                                                    <th class="px-5 py-3 border-b border-uci-border">"Riesgo"</th>
                                                    <th class="px-5 py-3 border-b border-uci-border">"Notas"</th>
                                                </tr>
                                            </thead>
                                            <tbody class="text-sm">
                                                <Suspense>
                                                    {move || measurements_res.get().map(|res_wrapper| match res_wrapper {
                                                        Ok(ms) => Either::Left(ms.iter().map(|m| {
                                                            let timestamp = m.timestamp.clone();
                                                            let score = m.apache_score;
                                                            let gcs = m.gcs_score;
                                                            let severity = m.severity.clone();
                                                            let risk = m.mortality_risk;
                                                            let notas = m.notas.clone();
                                                            view! {
                                                                <tr class="hover:bg-uci-accent/5 border-b border-uci-border transition-colors">
                                                                    <td class="px-5 py-3 font-mono text-xs text-uci-text">{timestamp[..16].to_string()}</td>
                                                                    <td class="px-5 py-3 font-extrabold text-rose-500">{score}</td>
                                                                    <td class="px-5 py-3 font-bold text-purple-500">{gcs}"/15"</td>
                                                                    <td class="px-5 py-3">
                                                                        <SeverityBadge level=severity />
                                                                    </td>
                                                                    <td class="px-5 py-3 font-bold text-red-600">{format!("{:.1}%", risk)}</td>
                                                                    <td class="px-5 py-3 text-xs text-uci-muted italic max-w-xs truncate">{notas}</td>
                                                                </tr>
                                                            }
                                                        }).collect_view()),
                                                        Err(_) => Either::Right(view! { <tr><td colspan="6" class="p-10 text-center text-uci-critical">"Error cargando datos"</td></tr> }),
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

            <Show when=move || show_delete_modal.get()>
                <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 backdrop-blur-sm">
                    <div class="glass-card p-8 max-w-md mx-4">
                        <h3 class="text-xl font-bold text-uci-text mb-4 flex items-center gap-2">
                            <i class="fa-solid fa-triangle-exclamation text-amber-500"></i>"Confirmar Eliminación"
                        </h3>
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
fn InfoRow(icon: &'static str, label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between">
            <span class="text-xs text-uci-muted flex items-center gap-1"><i class={format!("fa-solid {}", icon)}></i>{label}</span>
            <span class="text-sm text-uci-text font-medium text-right">{if value.is_empty() { "—".into() } else { value }}</span>
        </div>
    }
}