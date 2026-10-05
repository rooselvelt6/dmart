use crate::api;
use crate::components::{location_picker::LocationPicker, skin_picker::SkinPicker, toggle::Toggle};
use crate::i18n::trs;
use dmart_shared::models::*;
use leptos::either::Either;
use leptos::prelude::*;
use leptos_router::hooks::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn RegisterPage() -> impl IntoView {
    let patient = RwSignal::new(Patient::new());
    let saving = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let navigate = use_navigate();
    let cama_disponible = RwSignal::new(None::<(String, u8, String)>);
    let sin_camas = RwSignal::new(false);
    let camas_listas = RwSignal::new(false);
    let equipos_disponibles = RwSignal::new(Vec::<Equipo>::new());
    let equipos_seleccionados = RwSignal::new(Vec::<String>::new());

    spawn_local(async move {
        let resp: Result<serde_json::Value, _> = api::get("/admin/check-camas").await;
        if let Ok(data) = resp
            && let Some(disponible) = data.get("disponible").and_then(|v| v.as_bool())
        {
            if disponible {
                let cama_id = data
                    .get("cama_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let numero = data.get("numero").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                cama_disponible.set(Some((cama_id, numero, "General".to_string())));
            } else {
                sin_camas.set(true);
            }
        }
        // Cargar equipos disponibles
        if let Ok(equipos) = api::get_equipos_disponibles().await {
            equipos_disponibles.set(equipos);
        }
        // A partir de aquí ya se sabe si hay cama o no: el submit se habilita.
        // Sin esto, un usuario (o un test) que rellene el formulario rápido
        // puede pulsar "Registrar" antes de que responda `check-camas`, y el
        // handler aborta con "Debe esperar a que una cama esté disponible"
        // aunque sí haya camas.
        camas_listas.set(true);
    });

    let edad_calculada = Memo::new(move |_| {
        let p = patient.get();
        if p.fecha_nacimiento.is_empty() {
            return 0u8;
        }
        if let Ok(dob) = chrono::NaiveDate::parse_from_str(&p.fecha_nacimiento, "%Y-%m-%d") {
            let today = chrono::Utc::now().date_naive();
            today.years_since(dob).unwrap_or(0).min(150) as u8
        } else {
            0
        }
    });

    let tiempo_estadia = Memo::new(move |_| {
        let p = patient.get();
        if p.fecha_ingreso_hospital.is_empty() || p.fecha_ingreso_uci.is_empty() {
            return String::new();
        }
        if let (Ok(h), Ok(u)) = (
            chrono::DateTime::parse_from_rfc3339(&p.fecha_ingreso_hospital),
            chrono::DateTime::parse_from_rfc3339(&p.fecha_ingreso_uci),
        ) {
            let diff = u.signed_duration_since(h);
            let d = diff.num_days();
            let h2 = diff.num_hours() % 24;
            format!("{} días, {} horas", d, h2)
        } else {
            "—".into()
        }
    });

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let sin = sin_camas.get();
        if sin {
            error.set(Some(
                "No hay camas disponibles. No se puede registrar el paciente.".to_string(),
            ));
            return;
        }

        saving.set(true);
        error.set(None);
        let p = patient.get();
        let nav = navigate.clone();
        let equipos_ids = equipos_seleccionados.get();

        if let Some((cama_id, cama_num, _tipo)) = cama_disponible.get() {
            let mut paciente_con_cama = p;
            paciente_con_cama.cama_id = Some(cama_id);
            paciente_con_cama.cama_numero = Some(cama_num);

            spawn_local(async move {
                match api::create_patient_with_equipos(&paciente_con_cama, equipos_ids).await {
                    Ok(created) => {
                        saving.set(false);
                        nav(
                            &format!("/patients/{}", created.patient_id),
                            Default::default(),
                        );
                    }
                    Err(e) => {
                        saving.set(false);
                        error.set(Some(e));
                    }
                }
            });
        } else {
            saving.set(false);
            error.set(Some(
                "Debe esperar a que una cama esté disponible.".to_string(),
            ));
        }
    };

    let validate_cedula = move |v: &str| {
        // Cédula venezolana: V/E/J- seguido de 5 a 8 dígitos.
        let mut chars = v.chars();
        match chars.next() {
            Some(c) if matches!(c, 'V' | 'E' | 'J' | 'v' | 'e' | 'j') => {}
            _ => return false,
        }
        match chars.next() {
            Some('-') => {}
            _ => return false,
        }
        let digits: String = chars.collect();
        (5..=8).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit())
    };

    let validate_hc = move |v: &str| {
        // Historia clínica: HC- seguido de 3 a 6 dígitos.
        match v.strip_prefix("HC-") {
            Some(n) => (3..=6).contains(&n.len()) && n.chars().all(|c| c.is_ascii_digit()),
            None => false,
        }
    };

    let cedula_valid = Memo::new(move |_| validate_cedula(&patient.get().cedula));
    let hc_valid = Memo::new(move |_| validate_hc(&patient.get().historia_clinica));

    view! {
        <div class="page-enter w-full max-w-5xl mx-auto px-4 py-6 md:py-8 lg:py-10">
            <div class="mb-8 md:mb-10 text-center">
                <a href="/patients" class="text-xs md:text-sm flex items-center justify-center gap-2 mb-4 md:mb-6 no-underline font-medium" style="color:var(--uci-muted);" onmouseenter="this.style.color='var(--uci-accent)'" onmouseleave="this.style.color='var(--uci-muted)'">
                    <i class="fa-solid fa-chevron-left"></i>
                    {trs("reg-link-back-to-list")}
                </a>
                <h1 class="text-2xl md:text-3xl lg:text-4xl font-black tracking-tight" style="color:var(--uci-text);">{trs("reg-title")}</h1>
                <p class="text-sm md:text-base mt-2" style="color:var(--uci-muted);">{trs("reg-subtitle")}</p>
            </div>

            {move || sin_camas.get().then(|| view! {
                <div class="p-4 md:p-5 rounded-xl mb-6 md:mb-8 text-sm font-semibold flex items-center gap-3" style="background:rgba(234,179,8,0.1); border:1px solid rgba(234,179,8,0.3); color:#CA8A04;">
                    <i class="fa-solid fa-bed-empty text-lg"></i>
                    {trs("reg-alert-no-beds")}
                </div>
            })}

            {move || cama_disponible.get().map(|(_cama_id, cama_num, _tipo)| view! {
                <div class="p-4 rounded-xl mb-6 md:mb-8 text-sm flex items-center gap-3" style="background:rgba(34,197,94,0.1); border:1px solid rgba(34,197,94,0.3); color:#16A34A;">
                    <i class="fa-solid fa-bed text-lg"></i>
                    <span>{trs("reg-info-bed-assigned")}</span>
                </div>
            })}

            {move || error.get().map(|e| view! {
                <div class="p-4 md:p-5 rounded-xl mb-6 md:mb-8 text-sm font-semibold flex items-center gap-3" style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                    <i class="fa-solid fa-triangle-exclamation text-lg"></i>
                    {e}
                </div>
            })}

            <form on:submit=on_submit class="space-y-6 md:space-y-8">
                <FormSection title={trs("reg-section-identification")} icon=view! { <i class="fa-solid fa-id-card"></i> }.into_any()>
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 md:gap-6 lg:gap-8">
                        <FormField label={trs("reg-field-first-name")} icon=view! { <i class="fa-solid fa-user"></i> }.into_any()>
                            <input class="form-input" type="text" placeholder={trs("reg-placeholder-first-name")} required
                                prop:value=move || patient.get().nombre
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.nombre = v); } />
                        </FormField>
                        <FormField label={trs("reg-field-last-name")} icon=view! { <i class="fa-solid fa-user-group"></i> }.into_any()>
                            <input class="form-input" type="text" placeholder={trs("reg-placeholder-last-name")} required
                                prop:value=move || patient.get().apellido
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.apellido = v); } />
                        </FormField>

                        <FormField label={trs("reg-field-id-number")} icon=view! { <i class="fa-solid fa-address-card"></i> }.into_any()>
                            <input class=move || format!("form-input transition-all {}", if cedula_valid.get() { "border-emerald-500/50 bg-emerald-500/5" } else if !patient.get().cedula.is_empty() { "border-rose-500/50 bg-rose-500/5" } else { "" })
                                type="text" placeholder={trs("reg-placeholder-id-number")} required maxlength="10"
                                prop:value=move || patient.get().cedula
                                on:input=move |ev| {
                                    let mut v = event_target_value(&ev).to_uppercase();
                                    if !v.is_empty() && !v.starts_with('V') && !v.starts_with('E') {
                                        v = format!("V-{}", v);
                                    }
                                    if v.len() <= 10 {
                                        patient.update(|p| p.cedula = v);
                                    }
                                } />
                            <p class="text-[10px] mt-1 font-bold tracking-wide flex justify-between"
                                class:text-emerald-600=move || cedula_valid.get()
                                class:text-rose-500=move || !cedula_valid.get() && !patient.get().cedula.is_empty()>
                                <span>
                                    {move || if cedula_valid.get() { trs("reg-valid-format") } else if !patient.get().cedula.is_empty() { trs("reg-invalid-id-format") } else { trs("reg-required") }}
                                </span>
                                <span style="color:var(--uci-muted);">
                                    {move || format!("{}/10", patient.get().cedula.len())}
                                </span>
                            </p>
                        </FormField>

                        <FormField label={trs("reg-field-medical-record")} icon=view! { <i class="fa-solid fa-folder-open"></i> }.into_any()>
                            <input class=move || format!("form-input transition-all {}", if hc_valid.get() { "border-emerald-500/50 bg-emerald-500/5" } else if !patient.get().historia_clinica.is_empty() { "border-rose-500/50 bg-rose-500/5" } else { "" })
                                type="text" placeholder={trs("reg-placeholder-medical-record")} required maxlength="9"
                                prop:value=move || patient.get().historia_clinica
                                on:input=move |ev| {
                                    let mut v = event_target_value(&ev).to_uppercase();
                                    if !v.is_empty() && !v.starts_with("HC") {
                                        v = format!("HC-{}", v);
                                    }
                                    if v.len() <= 9 {
                                        patient.update(|p| p.historia_clinica = v);
                                    }
                                } />
                            <p class="text-[10px] mt-1 font-bold tracking-wide flex justify-between"
                                class:text-emerald-600=move || hc_valid.get()
                                class:text-rose-500=move || !hc_valid.get() && !patient.get().historia_clinica.is_empty()>
                                <span>
                                    {move || if hc_valid.get() { trs("reg-valid-format") } else if !patient.get().historia_clinica.is_empty() { trs("reg-invalid-hc-format") } else { trs("reg-required") }}
                                </span>
                                <span style="color:var(--uci-muted);">
                                    {move || format!("{}/9", patient.get().historia_clinica.len())}
                                </span>
                            </p>
                        </FormField>

                        <FormField label={trs("reg-field-sex")} icon=view! { <i class="fa-solid fa-venus-mars"></i> }.into_any()>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.sexo = if v == "Masculino" { Sexo::Masculino } else { Sexo::Femenino });
                                }>
                                <option value="Masculino">{trs("reg-option-male")}</option>
                                <option value="Femenino">{trs("reg-option-female")}</option>
                            </select>
                        </FormField>

                        <FormField label={trs("reg-field-birth-date")} icon=view! { <i class="fa-solid fa-calendar-day"></i> }.into_any()>
                            <input class="form-input" r#type="date" required
                                prop:value=move || patient.get().fecha_nacimiento
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.fecha_nacimiento = v); } />
                            <div class="mt-2 flex items-center gap-2 text-xs font-bold rounded-lg" style="background:rgba(59,130,246,0.1); color:var(--uci-accent); padding:8px 12px;">
                                <i class="fa-solid fa-circle-info"></i>
                                "Edad: " {move || { let e = edad_calculada.get(); if e > 0 { format!("{} años", e) } else { "—".into() } }}
                            </div>
                        </FormField>
                    </div>

                    <div class="mt-6 md:mt-8 p-4 md:p-6 rounded-2xl border" style="background:var(--uci-surface); border-color:var(--uci-border);">
                        <label class="form-label mb-3 md:mb-5 flex items-center gap-2 text-sm">
                            <i class="fa-solid fa-palette" style="color:var(--uci-accent);"></i>
                            {trs("reg-label-skin-color")}
                        </label>
                        <SkinPicker
                            value=Signal::derive(move || patient.get().color_piel)
                            on_change=move |v| patient.update(|p| p.color_piel = v)
                        />
                    </div>
                </FormSection>

                <FormSection title={trs("reg-section-origin-contact")} icon=view! { <i class="fa-solid fa-location-dot"></i> }.into_any()>
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 md:gap-6 lg:gap-8">
                        <FormField label={trs("reg-field-nationality")} icon=view! { <i class="fa-solid fa-flag"></i> }.into_any()>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| {
                                        if v == "Venezolano" {
                                            p.nacionalidad = Nacionalidad::Venezolano;
                                            p.pais = "Venezuela".into();
                                            p.estado = String::new();
                                            p.ciudad = String::new();
                                        } else {
                                            p.nacionalidad = Nacionalidad::Extranjero;
                                        }
                                    });
                                }>
                                <option value="Venezolano">{trs("reg-option-venezuelan")}</option>
                                <option value="Extranjero">{trs("reg-option-foreigner")}</option>
                            </select>
                        </FormField>

                        <div class="lg:col-span-3 md:col-span-2 col-span-1">
                            <LocationPicker
                                pais=Signal::derive(move || patient.get().pais)
                                estado=Signal::derive(move || patient.get().estado)
                                ciudad=Signal::derive(move || patient.get().ciudad)
                                on_change_pais=move |v| {
                                    patient.update(|p| {
                                        p.pais = v.clone();
                                        p.estado = String::new();
                                        p.ciudad = String::new();
                                        p.nacionalidad = if v == "Venezuela" { Nacionalidad::Venezolano } else { Nacionalidad::Extranjero };
                                    });
                                }
                                on_change_estado=move |v| patient.update(|p| p.estado = v)
                                on_change_ciudad=move |v| patient.update(|p| p.ciudad = v)
                            />
                        </div>

                        <FormField label={trs("reg-field-relative")} icon=view! { <i class="fa-solid fa-user-shield"></i> }.into_any()>
                            <input class="form-input" type="text" placeholder={trs("reg-placeholder-relative")}
                                prop:value=move || patient.get().familiar_encargado
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.familiar_encargado = v); } />
                        </FormField>
                        <div class="lg:col-span-2">
                            <FormField label={trs("reg-field-address")} icon=view! { <i class="fa-solid fa-house-medical"></i> }.into_any()>
                                <input class="form-input" type="text" placeholder={trs("reg-placeholder-address")}
                                    prop:value=move || patient.get().direccion
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.direccion = v); } />
                            </FormField>
                        </div>
                    </div>
                </FormSection>

                <FormSection title={trs("reg-section-hospital-admission")} icon=view! { <i class="fa-solid fa-hospital-user"></i> }.into_any()>
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 md:gap-6 mb-6 md:mb-8 lg:mb-10">
                        <FormField label={trs("reg-field-hospital-admission")} icon=view! { <i class="fa-solid fa-calendar-plus"></i> }.into_any()>
                            <input class="form-input" type="datetime-local" required
                                prop:value=move || { let p = patient.get(); p.fecha_ingreso_hospital.trim_end_matches('Z').chars().take(16).collect::<String>() }
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.fecha_ingreso_hospital = format!("{}:00Z", v));
                                } />
                        </FormField>
                        <FormField label={trs("reg-field-icu-admission")} icon=view! { <i class="fa-solid fa-truck-medical"></i> }.into_any()>
                            <input class="form-input" style="border-color:var(--uci-accent);" type="datetime-local" required
                                prop:value=move || { let p = patient.get(); p.fecha_ingreso_uci.trim_end_matches('Z').chars().take(16).collect::<String>() }
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.fecha_ingreso_uci = format!("{}:00Z", v));
                                } />
                        </FormField>
                        <FormField label={trs("reg-field-stay-time")} icon=view! { <i class="fa-solid fa-clock-rotate-left"></i> }.into_any()>
                            <div class="form-input flex items-center h-10 md:h-11 lg:h-12 text-sm md:text-base font-bold" style="background:rgba(59,130,246,0.05); border-color:var(--uci-accent); color:var(--uci-accent);">
                                {move || tiempo_estadia.get()}
                            </div>
                        </FormField>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4 md:gap-6 mb-6 md:mb-8 lg:mb-10">
                        <FormField label={trs("reg-field-admission-type")} icon=view! { <i class="fa-solid fa-shield-virus"></i> }.into_any()>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.tipo_admision = if v == "Urgente" { TipoAdmision::Urgente } else { TipoAdmision::Electiva });
                                }>
                                <option value="Urgente">{trs("reg-option-urgent")}</option>
                                <option value="Electiva">{trs("reg-option-elective")}</option>
                            </select>
                        </FormField>
                        <FormField label={trs("reg-field-referral")} icon=view! { <i class="fa-solid fa-right-left"></i> }.into_any()>
                            <div class="flex items-center gap-3 md:gap-4 h-10 md:h-11 lg:h-12 px-3 md:px-4 rounded-2xl border" style="background:var(--uci-surface); border-color:var(--uci-border);">
                                <Toggle
                                    value=Signal::derive(move || patient.get().migracion_otro_centro)
                                    on_change=move |v| patient.update(|p| p.migracion_otro_centro = v)
                                />
                                <span class="text-xs font-bold uppercase tracking-widest" style="color:var(--uci-text);">
                                    {move || if patient.get().migracion_otro_centro { trs("reg-label-from-other-center") } else { trs("reg-label-direct-admission") }}
                                </span>
                            </div>
                        </FormField>
                    </div>

                    {move || if patient.get().migracion_otro_centro {
                        Either::Left(view! {
                            <div class="mb-6 md:mb-8 lg:mb-10">
                                <FormField label={trs("reg-field-origin-center")} icon=view! { <i class="fa-solid fa-building-circle-arrow-right"></i> }.into_any()>
                                    <input class="form-input" type="text" placeholder={trs("reg-placeholder-origin-center")}
                                        on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.centro_origen = Some(v)); } />
                                </FormField>
                            </div>
                        })
                    } else { Either::Right(view! { <span></span> }) }}

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4 md:gap-6 mb-6 md:mb-8 lg:mb-10">
                        <div class="p-4 md:p-6 rounded-2xl border flex items-center justify-between" style="background:rgba(59,130,246,0.05); border-color:var(--uci-accent);">
                            <div class="flex items-center gap-3 md:gap-4">
                                <div class="w-10 h-10 md:w-12 md:h-12 rounded-xl flex items-center justify-center text-xl" style="background:rgba(59,130,246,0.2); color:var(--uci-accent);">
                                    <i class="fa-solid fa-mask-ventilator"></i>
                                </div>
                                <div>
                                    <div class="font-bold text-sm md:text-base" style="color:var(--uci-text);">{trs("reg-field-ventilation")}</div>
                                    <div class="text-[10px] uppercase font-bold" style="color:var(--uci-muted); letter-spacing:0.5px;">{trs("reg-label-invasive-support")}</div>
                                </div>
                            </div>
                            <Toggle
                                value=Signal::derive(move || patient.get().ventilacion_mecanica)
                                on_change=move |v| patient.update(|p| p.ventilacion_mecanica = v)
                            />
                        </div>
                    </div>

                    <FormField label={trs("reg-field-invasive-processes")} icon=view! { <i class="fa-solid fa-stretcher"></i> }.into_any()>
                        <textarea class="form-input" placeholder={trs("reg-placeholder-invasive-processes")} rows="3"
                            on:input=move |ev| {
                                let v = event_target_value(&ev);
                                patient.update(|p| p.procesos_invasivos = v.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
                            }></textarea>
                    </FormField>
                </FormSection>

                <FormSection title={trs("reg-section-diagnosis")} icon=view! { <i class="fa-solid fa-file-medical"></i> }.into_any()>
                    <div class="space-y-4 md:space-y-6">
                        <FormField label={trs("reg-field-clinical-description")} icon=view! { <i class="fa-solid fa-comment-medical"></i> }.into_any()>
                            <textarea class="form-input" placeholder={trs("reg-placeholder-clinical-description")} rows="3"
                                prop:value=move || patient.get().descripcion_ingreso
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.descripcion_ingreso = v); }></textarea>
                        </FormField>

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 md:gap-6">
                            <FormField label={trs("reg-field-hospital-diagnosis")} icon=view! { <i class="fa-solid fa-notes-medical"></i> }.into_any()>
                                <textarea class="form-input" placeholder={trs("reg-placeholder-hospital-diagnosis")} rows="4"
                                    prop:value=move || patient.get().diagnostico_hospital
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.diagnostico_hospital = v); }></textarea>
                            </FormField>
                            <FormField label={trs("reg-field-icu-diagnosis")} icon=view! { <i class="fa-solid fa-stethoscope" style="color:var(--uci-accent);"></i> }.into_any()>
                                <textarea class="form-input font-bold" style="border-color:var(--uci-accent);" placeholder={trs("reg-placeholder-icu-diagnosis")} rows="4"
                                    prop:value=move || patient.get().diagnostico_uci
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.diagnostico_uci = v); }></textarea>
                            </FormField>
                        </div>
                    </div>
                </FormSection>

                <Show when=move || !equipos_disponibles.get().is_empty()>
                    <FormSection title={trs("reg-section-equipment")} icon=view! { <i class="fa-solid fa-kit-medical"></i> }.into_any()>
                        <p class="text-sm mb-4" style="color:var(--uci-muted);">{trs("reg-equipment-description")}</p>
                        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
                            {move || {
                                let equipos = equipos_disponibles.get();
                                equipos.iter().map(|e| {
                                    let eq_id = e.equipo_id.clone();
                                    let eq_id2 = eq_id.clone();
                                    let eq_id3 = eq_id.clone();
                                    let nombre = e.nombre.clone();
                                    let tipo_label = e.tipo.label().to_string();
                                    let seleccionado = move || equipos_seleccionados.get().contains(&eq_id);
                                    let class_fn = move || format!(
                                        "flex items-center gap-3 p-3 rounded-xl border-2 cursor-pointer transition-all {}",
                                        if seleccionado() {
                                            "border-uci-accent bg-uci-accent/10"
                                        } else {
                                            "border-transparent bg-gray-50 dark:bg-gray-800 hover:border-gray-300"
                                        }
                                    );
                                    let checked_fn = move || equipos_seleccionados.get().contains(&eq_id2);
                                    view! {
                                        <label class=class_fn>
                                            <input type="checkbox"
                                                prop:checked=checked_fn
                                                on:change=move |_| {
                                                    equipos_seleccionados.update(|ids| {
                                                        if ids.contains(&eq_id3) {
                                                            ids.retain(|x| x != &eq_id3);
                                                        } else {
                                                            ids.push(eq_id3.clone());
                                                        }
                                                    });
                                                }
                                                class="form-checkbox h-4 w-4 rounded border-gray-300 text-uci-accent focus:ring-uci-accent"
                                            />
                                            <div>
                                                <div class="text-sm font-medium" style="color:var(--uci-text);">{nombre}</div>
                                                <div class="text-xs" style="color:var(--uci-muted);">{tipo_label}</div>
                                            </div>
                                        </label>
                                    }
                                }).collect_view()
                            }}
                        </div>
                    </FormSection>
                </Show>

                <div class="flex flex-col md:flex-row justify-end gap-3 md:gap-4 lg:gap-6 mt-10 md:mt-12 lg:mt-16 pb-16 md:pb-20 lg:pb-24">
                    <a href="/patients" class="btn-outline flex items-center justify-center gap-2 px-6 md:px-8 lg:px-10 h-11 md:h-12 lg:h-14 text-sm md:text-base group">
                        <i class="fa-solid fa-xmark group-hover:rotate-90 transition-transform"></i>
                        {trs("reg-btn-cancel")}
                    </a>
                    <button type="submit" class="btn-primary flex items-center justify-center gap-2 px-8 md:px-10 lg:px-12 h-11 md:h-12 lg:h-14 text-base md:text-lg" disabled=move || saving.get() || !camas_listas.get()>
                        {move || {
                            if saving.get() {
                                Either::Left(view! {
                                    <span class="flex items-center gap-2">
                                        <i class="fa-solid fa-circle-notch animate-spin"></i>
                                        {trs("reg-btn-processing")}
                                    </span>
                                })
                            } else {
                                Either::Right(view! {
                                    <span class="flex items-center gap-2">
                                        <i class="fa-solid fa-floppy-disk"></i>
                                        {trs("reg-btn-register")}
                                    </span>
                                })
                            }
                        }}
                    </button>
</div>
            </form>
        </div>
    }
}
#[component]
fn FormSection(title: String, icon: AnyView, children: Children) -> impl IntoView {
    view! {
        <div class="glass-card p-4 md:p-6 lg:p-8 md:p-10 mb-6 md:mb-8 lg:mb-10 animate-fade-in" style="border-color:rgba(100,116,139,0.6);">
            <div class="flex items-center gap-3 md:gap-4 mb-6 md:mb-8 lg:mb-10 pb-4 md:pb-6" style="border-bottom:1px solid var(--uci-border);">
                <div class="w-10 h-10 md:w-12 lg:w-14 rounded-xl md:rounded-2xl flex items-center justify-center text-lg md:text-xl lg:text-2xl shrink-0" style="background:linear-gradient(135deg,rgba(59,130,246,0.1),rgba(99,102,241,0.2)); color:var(--uci-accent); border:1px solid rgba(59,130,246,0.1);">
                    {icon}
                </div>
                <div>
                    <h2 class="text-base md:text-lg lg:text-xl font-black tracking-tight uppercase" style="color:var(--uci-text);">{title}</h2>
                    <div class="w-8 md:w-10 lg:w-12 h-1 rounded-full mt-1" style="background:var(--uci-accent);"></div>
                </div>
            </div>
            {children()}
        </div>
    }
}

#[component]
fn FormField(label: String, icon: AnyView, children: Children) -> impl IntoView {
    view! {
        <div class="space-y-2 md:space-y-3 w-full">
            <label class="form-label flex items-center gap-2 text-xs font-black uppercase tracking-widest" style="color:var(--uci-muted);">
                {icon}
                {label}
            </label>
            <div class="relative">
                {children()}
            </div>
        </div>
    }
}
