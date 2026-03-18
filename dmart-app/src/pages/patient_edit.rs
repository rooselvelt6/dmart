use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::either::Either;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use crate::api;

#[component]
pub fn PatientEditPage() -> impl IntoView {
    let params = use_params_map();
    let patient_id = move || params.get().get("id").unwrap_or_default();

    let patient_res = LocalResource::new(move || {
        let pid = patient_id();
        async move { api::get_patient(&pid).await }
    });

    let saving = RwSignal::new(false);
    let error_msg = RwSignal::new(String::new());
    let navigate = use_navigate();

    view! {
        <div class="page-enter">
            <Show when=move || !error_msg.get().is_empty()>
                <div style="background:rgba(239,68,68,0.1); padding:20px; border-radius:10px; color:#EF4444; margin-bottom:20px;">
                    {error_msg.get()}
                </div>
            </Show>

            <Suspense fallback=move || view! { <div style="text-align:center; padding:60px; color:#64748B;">"Cargando..."</div> }>
                {move || patient_res.get().map(|res_wrapper| match &*res_wrapper {
                    Ok(p) => {
                        let patient = RwSignal::new(p.clone());
                        let pid_val = patient_id();
                        let saving = saving;
                        let error_msg = error_msg;
                        let navigate = navigate.clone();
                        let do_submit = move |ev: leptos::ev::SubmitEvent| {
                            ev.prevent_default();
                            saving.set(true);
                            let data = patient.get();
                            let pid = patient_id();
                            let navigate = navigate.clone();
                            spawn_local(async move {
                                match api::update_patient(&pid, &data).await {
                                    Ok(_) => {
                                        saving.set(false);
                                        navigate(&format!("/patients/{}", pid), Default::default());
                                    },
                                    Err(e) => {
                                        saving.set(false);
                                        error_msg.set(e);
                                    },
                                }
                            });
                        };
                        Either::Left(view! {
                            <div style="max-width:1200px; margin:0 auto;">
                                <a href=format!("/patients/{}", pid_val) style="color:#64748B; font-size:14px; text-decoration:none; display:inline-block; margin-bottom:16px;">
                                    "<- Volver al detalle"
                                </a>
                                
                                <h1 style="font-size:28px; font-weight:800; color:#E2E8F0; margin:0 0 8px;">"Editar Paciente"</h1>
                                <p style="color:#64748B; font-size:14px; margin:0 0 24px;">"Actualice los datos del paciente"</p>

                                <form on:submit=do_submit>
                                    <div style="display:grid; grid-template-columns:repeat(4,1fr); gap:16px; margin-bottom:24px;">
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">"Identificacion"</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"NOMBRE"</label>
                                                    <input type="text" required=true value=patient.get().nombre on:input=move |ev| patient.update(|x| x.nombre = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"APELLIDO"</label>
                                                    <input type="text" required=true value=patient.get().apellido on:input=move |ev| patient.update(|x| x.apellido = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"CEDULA"</label>
                                                    <input type="text" required=true value=patient.get().cedula on:input=move |ev| patient.update(|x| x.cedula = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"HC"</label>
                                                    <input type="text" required=true value=patient.get().historia_clinica on:input=move |ev| patient.update(|x| x.historia_clinica = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                            </div>
                                        </div>

                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">"Ubicacion"</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"PAIS"</label>
                                                    <input type="text" value=patient.get().pais on:input=move |ev| patient.update(|x| x.pais = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"ESTADO"</label>
                                                    <input type="text" value=patient.get().estado on:input=move |ev| patient.update(|x| x.estado = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"CIUDAD"</label>
                                                    <input type="text" value=patient.get().ciudad on:input=move |ev| patient.update(|x| x.ciudad = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"FAMILIAR"</label>
                                                    <input type="text" value=patient.get().familiar_encargado on:input=move |ev| patient.update(|x| x.familiar_encargado = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0;" />
                                                </div>
                                            </div>
                                        </div>

                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">"Diagnostico"</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"DIAG. HOSPITAL"</label>
                                                    <textarea rows=4 on:input=move |ev| patient.update(|x| x.diagnostico_hospital = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0; resize:vertical;">{patient.get().diagnostico_hospital}</textarea>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"DIAG. UCI"</label>
                                                    <textarea rows=4 on:input=move |ev| patient.update(|x| x.diagnostico_uci = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0; resize:vertical;">{patient.get().diagnostico_uci}</textarea>
                                                </div>
                                            </div>
                                        </div>

                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">"Antecedentes"</h3>
                                            <div>
                                                <label style="font-size:11px; color:#64748B; display:block; margin-bottom:4px;">"ANTECEDENTES"</label>
                                                <textarea rows=10 on:input=move |ev| patient.update(|x| x.antecedentes = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; border:1px solid #334155; background:#1E293B; color:#E2E8F0; resize:vertical;">{patient.get().antecedentes}</textarea>
                                            </div>
                                        </div>
                                    </div>

                                    <div style="display:flex; justify-content:flex-end; gap:12px; padding-bottom:40px;">
                                        <a href=format!("/patients/{}", pid_val) style="padding:12px 24px; border-radius:8px; border:1px solid #334155; color:#94A3B8; text-decoration:none; font-weight:600;">
                                            "Cancelar"
                                        </a>
                                        <button type="submit" style="padding:12px 32px; border-radius:8px; background:#3B82F6; color:white; border:none; font-weight:600; cursor:pointer;">
                                            "Guardar Cambios"
                                        </button>
                                    </div>
                                </form>
                            </div>
                        })
                    },
                    _ => Either::Right(view! { <div style="color:#EF4444;">"Error cargando paciente"</div> }),
                })}
            </Suspense>
        </div>
    }
}
