use crate::api;
use crate::components::skin_picker::SkinPicker;
use crate::components::toggle::Toggle;
use crate::i18n::trs;
use dmart_shared::models::*;
use leptos::either::Either;
use leptos::prelude::*;
use leptos_router::hooks::*;
use wasm_bindgen_futures::spawn_local;

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

            <Suspense fallback=move || view! { <crate::components::ui_kit::LoadingState label={trs("ped-loading")} /> }>
                {move || patient_res.get().map(|res_wrapper| match res_wrapper {
                    Ok(p) => {
                        let patient = RwSignal::new(p.clone());
                        let pid_val = patient_id();
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
                            <div style="max-width:1400px; margin:0 auto;">
                                <a href=format!("/patients/{}", pid_val) style="color:var(--uci-muted); font-size:14px; text-decoration:none; display:inline-block; margin-bottom:16px;">
                                    {trs("ped-back-link")}
                                </a>

                                <h1 style="font-size:28px; font-weight:800; color:var(--uci-text); margin:0 0 8px;">{trs("ped-page-title")}</h1>
                                <p style="color:var(--uci-muted); font-size:14px; margin:0 0 24px;">{trs("ped-page-subtitle")}</p>

                                <form on:submit=do_submit>
                                    <div style="display:grid; grid-template-columns:repeat(4,1fr); gap:16px; margin-bottom:24px;">
                                        // 1. Identificación
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-identificacion")}</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-nombre")}</label>
                                                    <input type="text" required=true value=patient.get().nombre on:input=move |ev| patient.update(|x| x.nombre = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-apellido")}</label>
                                                    <input type="text" required=true value=patient.get().apellido on:input=move |ev| patient.update(|x| x.apellido = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-cedula")}</label>
                                                    <input type="text" required=true value=patient.get().cedula on:input=move |ev| patient.update(|x| x.cedula = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-historia-clinica")}</label>
                                                    <input type="text" required=true value=patient.get().historia_clinica on:input=move |ev| patient.update(|x| x.historia_clinica = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-sexo")}</label>
                                                    <select prop:value=format!("{:?}", patient.get().sexo) on:change=move |ev| {
                                                        let v = event_target_value(&ev);
                                                        patient.update(|x| x.sexo = match v.as_str() {
                                                            "Femenino" => Sexo::Femenino,
                                                            _ => Sexo::Masculino,
                                                        });
                                                    } style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);">
                                                        <option value="Masculino">{trs("ped-sex-masculino")}</option>
                                                        <option value="Femenino">{trs("ped-sex-femenino")}</option>
                                                    </select>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-color-piel")}</label>
                                                    <SkinPicker
                                                        value=Signal::derive(move || patient.get().color_piel.clone())
                                                        on_change=move |v| patient.update(|x| x.color_piel = v)
                                                    />
                                                </div>
                                            </div>
                                        </div>

                                        // 2. Datos Personales
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-datos-personales")}</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-nacionalidad")}</label>
                                                    <select prop:value=format!("{:?}", patient.get().nacionalidad) on:change=move |ev| {
                                                        let v = event_target_value(&ev);
                                                        patient.update(|x| x.nacionalidad = match v.as_str() {
                                                            "Extranjero" => Nacionalidad::Extranjero,
                                                            _ => Nacionalidad::Venezolano,
                                                        });
                                                    } style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);">
                                                        <option value="Venezolano">{trs("ped-nac-venezolano")}</option>
                                                        <option value="Extranjero">{trs("ped-nac-extranjero")}</option>
                                                    </select>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-fecha-nacimiento")}</label>
                                                    <input type="date" value=patient.get().fecha_nacimiento on:input=move |ev| patient.update(|x| x.fecha_nacimiento = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-lugar-nacimiento")}</label>
                                                    <input type="text" value=patient.get().lugar_nacimiento on:input=move |ev| patient.update(|x| x.lugar_nacimiento = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-familiar-encargado")}</label>
                                                    <input type="text" value=patient.get().familiar_encargado on:input=move |ev| patient.update(|x| x.familiar_encargado = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-direccion")}</label>
                                                    <input type="text" value=patient.get().direccion on:input=move |ev| patient.update(|x| x.direccion = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                            </div>
                                        </div>

                                        // 3. Ubicación
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-ubicacion")}</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-pais")}</label>
                                                    <input type="text" value=patient.get().pais on:input=move |ev| patient.update(|x| x.pais = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-estado")}</label>
                                                    <input type="text" value=patient.get().estado on:input=move |ev| patient.update(|x| x.estado = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-ciudad")}</label>
                                                    <input type="text" value=patient.get().ciudad on:input=move |ev| patient.update(|x| x.ciudad = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                            </div>
                                        </div>

                                        // 4. Ingreso Hospitalario
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-ingreso")}</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-tipo-admision")}</label>
                                                    <select prop:value=format!("{:?}", patient.get().tipo_admision) on:change=move |ev| {
                                                        let v = event_target_value(&ev);
                                                        patient.update(|x| x.tipo_admision = match v.as_str() {
                                                            "Electiva" => TipoAdmision::Electiva,
                                                            _ => TipoAdmision::Urgente,
                                                        });
                                                    } style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);">
                                                        <option value="Urgente">{trs("ped-adm-urgente")}</option>
                                                        <option value="Electiva">{trs("ped-adm-electiva")}</option>
                                                    </select>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-fecha-ingreso-hospital")}</label>
                                                    <input type="datetime-local" value=patient.get().fecha_ingreso_hospital[..16].to_string() on:input=move |ev| patient.update(|x| x.fecha_ingreso_hospital = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-fecha-ingreso-uci")}</label>
                                                    <input type="datetime-local" value=patient.get().fecha_ingreso_uci[..16].to_string() on:input=move |ev| patient.update(|x| x.fecha_ingreso_uci = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                </div>
                                                <div class="flex items-center justify-between p-3 bg-uci-surface rounded-xl border border-uci-border">
                                                    <div class="text-xs text-uci-muted">{trs("ped-label-migracion")}</div>
                                                    <Toggle
                                                        value=Signal::derive(move || patient.get().migracion_otro_centro)
                                                        on_change=move |v| patient.update(|x| x.migracion_otro_centro = v) />
                                                </div>
                                                <Show when=move || patient.get().migracion_otro_centro>
                                                    <div>
                                                        <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-centro-origen")}</label>
                                                        <input type="text" value=patient.get().centro_origen.clone().unwrap_or_default() on:input=move |ev| patient.update(|x| x.centro_origen = Some(event_target_value(&ev))) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border);" />
                                                    </div>
                                                </Show>
                                                <div class="flex items-center justify-between p-3 bg-uci-surface rounded-xl border border-uci-border">
                                                    <div class="text-xs text-uci-muted">{trs("ped-label-ventilacion")}</div>
                                                    <Toggle
                                                        value=Signal::derive(move || patient.get().ventilacion_mecanica)
                                                        on_change=move |v| patient.update(|x| x.ventilacion_mecanica = v) />
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    // Segunda fila
                                    <div style="display:grid; grid-template-columns:repeat(2,1fr); gap:16px; margin-bottom:24px;">
                                        // 5. Diagnóstico
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-diagnostico")}</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-descripcion-ingreso")}</label>
                                                    <textarea rows=3 on:input=move |ev| patient.update(|x| x.descripcion_ingreso = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;">{patient.get().descripcion_ingreso}</textarea>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-diagnostico-hospital")}</label>
                                                    <textarea rows=3 on:input=move |ev| patient.update(|x| x.diagnostico_hospital = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;">{patient.get().diagnostico_hospital}</textarea>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-diagnostico-uci")}</label>
                                                    <textarea rows=3 on:input=move |ev| patient.update(|x| x.diagnostico_uci = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;">{patient.get().diagnostico_uci}</textarea>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-resumen-ingreso")}</label>
                                                    <textarea rows=3 on:input=move |ev| patient.update(|x| x.resumen_ingreso = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;">{patient.get().resumen_ingreso}</textarea>
                                                </div>
                                            </div>
                                        </div>

                                        // 6. Examen Físico
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-examen-fisico")}</h3>
                                            <div style="display:flex; flex-direction:column; gap:12px;">
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-examen-hospital")}</label>
                                                    <textarea rows=4 on:input=move |ev| patient.update(|x| x.examen_fisico_hospital = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;">{patient.get().examen_fisico_hospital}</textarea>
                                                </div>
                                                <div>
                                                    <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-examen-uci")}</label>
                                                    <textarea rows=4 on:input=move |ev| patient.update(|x| x.examen_fisico_uci = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;">{patient.get().examen_fisico_uci}</textarea>
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    // Tercera fila
                                    <div style="display:grid; grid-template-columns:repeat(2,1fr); gap:16px; margin-bottom:24px;">
                                        // 7. Antecedentes
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-antecedentes")}</h3>
                                            <div>
                                                <label style="font-size:11px; color:var(--uci-muted); display:block; margin-bottom:4px;">{trs("ped-label-antecedentes")}</label>
                                                <textarea rows=8 on:input=move |ev| patient.update(|x| x.antecedentes = event_target_value(&ev)) style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;">{patient.get().antecedentes}</textarea>
                                            </div>
                                        </div>

                                        // 8. Procesos Invasivos
                                        <div class="glass-card" style="padding:20px;">
                                            <h3 style="font-size:12px; color:#3B82F6; text-transform:uppercase; margin:0 0 16px;">{trs("ped-section-procesos-invasivos")}</h3>
                                            <p style="font-size:11px; color:var(--uci-muted); margin-bottom:12px;">{trs("ped-procesos-hint")}</p>
                                            <div>
                                                <textarea
                                                    rows=8
                                                    prop:value=patient.get().procesos_invasivos.join(", ")
                                                    on:input=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        let vec: Vec<String> = val.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                                                        patient.update(|x| x.procesos_invasivos = vec);
                                                    }
                                                    placeholder={trs("ped-procesos-placeholder")}
                                                    style="width:100%; padding:10px; border-radius:8px; background:var(--uci-input-bg); color:var(--uci-text); border:1px solid var(--uci-input-border); resize:vertical;"
                                                ></textarea>
                                            </div>
                                        </div>
                                    </div>

                                    <div style="display:flex; justify-content:flex-end; gap:12px; padding-bottom:40px;">
                                        <a href=format!("/patients/{}", pid_val) style="padding:12px 24px; border-radius:8px; border:1px solid var(--uci-input-border); color:var(--uci-muted); text-decoration:none; font-weight:600;">
                                            {trs("ped-btn-cancel")}
                                        </a>
                                        <button type="submit" style="padding:12px 32px; border-radius:8px; background:#3B82F6; color:white; border:none; font-weight:600; cursor:pointer;">
                                            {trs("ped-btn-save")}
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
