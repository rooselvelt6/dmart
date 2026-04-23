use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use crate::api;
use crate::components::{skin_picker::SkinPicker, toggle::Toggle};

#[component]
pub fn RegisterPage() -> impl IntoView {
    let patient = RwSignal::new(Patient::new());
    let saving   = RwSignal::new(false);
    let error    = RwSignal::new(None::<String>);
    let navigate = use_navigate();

    let edad_calculada = Memo::new(move |_| {
        let p = patient.get();
        if p.fecha_nacimiento.is_empty() { return 0u8; }
        if let Ok(dob) = chrono::NaiveDate::parse_from_str(&p.fecha_nacimiento, "%Y-%m-%d") {
            let today = chrono::Utc::now().date_naive();
            today.years_since(dob).unwrap_or(0).min(150) as u8
        } else { 0 }
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
        } else { "—".into() }
    });

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        saving.set(true);
        error.set(None);
        let p = patient.get();
        let nav = navigate.clone();
        spawn_local(async move {
            match api::create_patient(&p).await {
                Ok(created) => {
                    saving.set(false);
                    nav(&format!("/patients/{}", created.patient_id), Default::default());
                }
                Err(e) => {
                    saving.set(false);
                    error.set(Some(e));
                }
            }
        });
    };

    let validate_cedula = move |v: &str| {
        let re = regex::Regex::new(r"^[VEJvej]-[0-9]{5,9}$").unwrap();
        re.is_match(v)
    };
    
    let validate_hc = move |v: &str| {
        let re = regex::Regex::new(r"^HC-[0-9]{3,8}$").unwrap();
        re.is_match(v)
    };

    let cedula_valid = Memo::new(move |_| validate_cedula(&patient.get().cedula));
    let hc_valid = Memo::new(move |_| validate_hc(&patient.get().historia_clinica));

    view! {
        <div class="page-enter w-full max-w-5xl mx-auto px-4 py-6 md:py-8 lg:py-10">
            <div class="mb-8 md:mb-10 text-center">
                <a href="/patients" class="text-xs md:text-sm flex items-center justify-center gap-2 mb-4 md:mb-6 no-underline font-medium" style="color:var(--uci-muted);" onmouseenter="this.style.color='var(--uci-accent)'" onmouseleave="this.style.color='var(--uci-muted)'">
                    <i class="fa-solid fa-chevron-left"></i>
                    "Volver a Listado"
                </a>
                <h1 class="text-2xl md:text-3xl lg:text-4xl font-black tracking-tight" style="color:var(--uci-text);">"Expediente Clínico"</h1>
                <p class="text-sm md:text-base mt-2" style="color:var(--uci-muted);">"Registro formal de ingreso a la Unidad de Cuidados Intensivos"</p>
            </div>

            {move || error.get().map(|e| view! {
                <div class="p-4 md:p-5 rounded-xl mb-6 md:mb-8 text-sm font-semibold flex items-center gap-3" style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                    <i class="fa-solid fa-triangle-exclamation text-lg"></i>
                    {e}
                </div>
            })}

            <form on:submit=on_submit class="space-y-6 md:space-y-8">
                <FormSection title="Identificación del Paciente" icon=move || view! { <i class="fa-solid fa-id-card"></i> }>
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 md:gap-6 lg:gap-8">
                        <FormField label="Nombre(s) *" icon=move || view! { <i class="fa-solid fa-user"></i> }>
                            <input class="form-input" type="text" placeholder="Ej: Juan Alberto" required
                                prop:value=move || patient.get().nombre
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.nombre = v); } />
                        </FormField>
                        <FormField label="Apellido(s) *" icon=move || view! { <i class="fa-solid fa-user-group"></i> }>
                            <input class="form-input" type="text" placeholder="Ej: Pérez García" required
                                prop:value=move || patient.get().apellido
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.apellido = v); } />
                        </FormField>
                        
                        <FormField label="Cédula de Identidad *" icon=move || view! { <i class="fa-solid fa-address-card"></i> }>
                            <input class=move || format!("form-input transition-all {}", if cedula_valid.get() { "border-emerald-500/50 bg-emerald-500/5" } else if !patient.get().cedula.is_empty() { "border-rose-500/50 bg-rose-500/5" } else { "" })
                                type="text" placeholder="V-00000000" required
                                prop:value=move || patient.get().cedula
                                on:input=move |ev| { 
                                    let mut v = event_target_value(&ev).to_uppercase();
                                    if !v.is_empty() && !v.starts_with('V') && !v.starts_with('E') {
                                        v = format!("V-{}", v);
                                    }
                                    patient.update(|p| p.cedula = v); 
                                } />
                            <p class="text-[10px] mt-1 font-bold tracking-wide" class:text-emerald-600=move || cedula_valid.get() class:text-rose-500=move || !cedula_valid.get() && !patient.get().cedula.is_empty()>
                                {move || if cedula_valid.get() { "✓ Formato válido" } else if !patient.get().cedula.is_empty() { "✗ Use formato V-00000000" } else { "Requerido" }}
                            </p>
                        </FormField>

                        <FormField label="Historia Clínica *" icon=move || view! { <i class="fa-solid fa-folder-open"></i> }>
                            <input class=move || format!("form-input transition-all {}", if hc_valid.get() { "border-emerald-500/50 bg-emerald-500/5" } else if !patient.get().historia_clinica.is_empty() { "border-rose-500/50 bg-rose-500/5" } else { "" })
                                type="text" placeholder="HC-00000" required
                                prop:value=move || patient.get().historia_clinica
                                on:input=move |ev| { 
                                    let mut v = event_target_value(&ev).to_uppercase();
                                    if !v.is_empty() && !v.starts_with("HC") {
                                        v = format!("HC-{}", v);
                                    }
                                    patient.update(|p| p.historia_clinica = v); 
                                } />
                            <p class="text-[10px] mt-1 font-bold tracking-wide" class:text-emerald-600=move || hc_valid.get() class:text-rose-500=move || !hc_valid.get() && !patient.get().historia_clinica.is_empty()>
                                {move || if hc_valid.get() { "✓ Formato válido" } else if !patient.get().historia_clinica.is_empty() { "✗ Use formato HC-00000" } else { "Requerido" }}
                            </p>
                        </FormField>

                        <FormField label="Sexo" icon=move || view! { <i class="fa-solid fa-venus-mars"></i> }>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.sexo = if v == "Masculino" { Sexo::Masculino } else { Sexo::Femenino });
                                }>
                                <option value="Masculino">"Masculino"</option>
                                <option value="Femenino">"Femenino"</option>
                            </select>
                        </FormField>

                        <FormField label="Fecha de Nacimiento *" icon=move || view! { <i class="fa-solid fa-calendar-day"></i> }>
                            <input class="form-input" type="date" required
                                prop:value=move || patient.get().fecha_nacimiento
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.fecha_nacimiento = v); } />
                            <div class="mt-2 flex items-center gap-2 text-xs font-bold rounded-lg" style="background:rgba(59,130,246,0.1); color:var(--uci-accent); padding:8px 12px;">
                                <i class="fa-solid fa-circle-info"></i>
                                "Edad: " {move || { let e = edad_calculada.get(); if e > 0 { format!("{} años", e) } else { "—".into() } }}
                            </div>
                        </FormField>
                    </div>

                    <div class="mt-6 md:mt-8 p-4 md:p-6 rounded-2xl border" style="background:rgba(30,41,59,0.3); border-color:var(--uci-border);">
                        <label class="form-label mb-3 md:mb-5 flex items-center gap-2 text-sm">
                            <i class="fa-solid fa-palette" style="color:var(--uci-accent);"></i>
                            "Color de Piel (Escala Fitzpatrick)"
                        </label>
                        <SkinPicker
                            value=Signal::derive(move || patient.get().color_piel)
                            on_change=move |v| patient.update(|p| p.color_piel = v)
                        />
                    </div>
                </FormSection>

                <FormSection title="Procedencia y Contacto" icon=move || view! { <i class="fa-solid fa-location-dot"></i> }>
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 md:gap-6 lg:gap-8">
                        <FormField label="Nacionalidad" icon=move || view! { <i class="fa-solid fa-flag"></i> }>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| {
                                        if v == "Venezolano" {
                                            p.nacionalidad = Nacionalidad::Venezolano;
                                            p.pais = "Venezuela".into();
                                        } else {
                                            p.nacionalidad = Nacionalidad::Extranjero;
                                        }
                                    });
                                }>
                                <option value="Venezolano">"🇻🇪 Venezolano"</option>
                                <option value="Extranjero">"🌍 Extranjero"</option>
                            </select>
                        </FormField>
                        <FormField label="País" icon=move || view! { <i class="fa-solid fa-earth-americas"></i> }>
                            <input class="form-input" type="text" placeholder="País de residencia"
                                prop:value=move || patient.get().pais
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.pais = v); } />
                        </FormField>
                        <FormField label="Estado" icon=move || view! { <i class="fa-solid fa-map-location-dot"></i> }>
                            <input class="form-input" type="text" placeholder="Estado / Provincia"
                                prop:value=move || patient.get().estado
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.estado = v); } />
                        </FormField>
                        <FormField label="Ciudad" icon=move || view! { <i class="fa-solid fa-city"></i> }>
                            <input class="form-input" type="text" placeholder="Ciudad"
                                prop:value=move || patient.get().ciudad
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.ciudad = v); } />
                        </FormField>
                        <FormField label="Familiar Encargado" icon=move || view! { <i class="fa-solid fa-user-shield"></i> }>
                            <input class="form-input" type="text" placeholder="Nombre del responsable"
                                prop:value=move || patient.get().familiar_encargado
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.familiar_encargado = v); } />
                        </FormField>
                        <div class="lg:col-span-2">
                            <FormField label="Dirección de Residencia" icon=move || view! { <i class="fa-solid fa-house-medical"></i> }>
                                <input class="form-input" type="text" placeholder="Dirección completa"
                                    prop:value=move || patient.get().direccion
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.direccion = v); } />
                            </FormField>
                        </div>
                    </div>
                </FormSection>

                <FormSection title="Ingreso Hospitalario" icon=move || view! { <i class="fa-solid fa-hospital-user"></i> }>
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 md:gap-6 mb-6 md:mb-8 lg:mb-10">
                        <FormField label="Ingreso Hospitalario *" icon=move || view! { <i class="fa-solid fa-calendar-plus"></i> }>
                            <input class="form-input" type="datetime-local" required
                                prop:value=move || { let p = patient.get(); p.fecha_ingreso_hospital.trim_end_matches('Z').chars().take(16).collect::<String>() }
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.fecha_ingreso_hospital = format!("{}:00Z", v));
                                } />
                        </FormField>
                        <FormField label="Ingreso UCI *" icon=move || view! { <i class="fa-solid fa-truck-medical"></i> }>
                            <input class="form-input" style="border-color:var(--uci-accent);" type="datetime-local" required
                                prop:value=move || { let p = patient.get(); p.fecha_ingreso_uci.trim_end_matches('Z').chars().take(16).collect::<String>() }
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.fecha_ingreso_uci = format!("{}:00Z", v));
                                } />
                        </FormField>
                        <FormField label="Tiempo de Estadía" icon=move || view! { <i class="fa-solid fa-clock-rotate-left"></i> }>
                            <div class="form-input flex items-center h-10 md:h-11 lg:h-12 text-sm md:text-base font-bold" style="background:rgba(59,130,246,0.05); border-color:var(--uci-accent); color:var(--uci-accent);">
                                {move || tiempo_estadia.get()}
                            </div>
                        </FormField>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4 md:gap-6 mb-6 md:mb-8 lg:mb-10">
                        <FormField label="Tipo de Admisión" icon=move || view! { <i class="fa-solid fa-shield-virus"></i> }>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.tipo_admision = if v == "Urgente" { TipoAdmision::Urgente } else { TipoAdmision::Electiva });
                                }>
                                <option value="Urgente">"🚨 Urgente (No programada)"</option>
                                <option value="Electiva">"📅 Electiva (Programada)"</option>
                            </select>
                        </FormField>
                        <FormField label="Referido / Traslado" icon=move || view! { <i class="fa-solid fa-right-left"></i> }>
                            <div class="flex items-center gap-3 md:gap-4 h-10 md:h-11 lg:h-12 px-3 md:px-4 rounded-2xl border" style="background:rgba(30,41,59,0.2); border-color:var(--uci-border);">
                                <Toggle
                                    value=Signal::derive(move || patient.get().migracion_otro_centro)
                                    on_change=move |v| patient.update(|p| p.migracion_otro_centro = v)
                                />
                                <span class="text-xs font-bold uppercase tracking-widest" style="color:var(--uci-text);">
                                    {move || if patient.get().migracion_otro_centro { "Desde otro centro" } else { "Ingreso directo" }}
                                </span>
                            </div>
                        </FormField>
                    </div>

                    {move || if patient.get().migracion_otro_centro {
                        Either::Left(view! {
                            <div class="mb-6 md:mb-8 lg:mb-10">
                                <FormField label="Centro de Salud de Origen" icon=move || view! { <i class="fa-solid fa-building-circle-arrow-right"></i> }>
                                    <input class="form-input" type="text" placeholder="Nombre del hospital o clínica de origen"
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
                                    <div class="font-bold text-sm md:text-base" style="color:var(--uci-text);">"Ventilación Mecánica"</div>
                                    <div class="text-[10px] uppercase font-bold" style="color:var(--uci-muted); letter-spacing:0.5px;">"Soporte Invasivo"</div>
                                </div>
                            </div>
                            <Toggle
                                value=Signal::derive(move || patient.get().ventilacion_mecanica)
                                on_change=move |v| patient.update(|p| p.ventilacion_mecanica = v)
                            />
                        </div>
                    </div>

                    <FormField label="Procesos Invasivos Actuales" icon=move || view! { <i class="fa-solid fa-stretcher"></i> }>
                        <textarea class="form-input" placeholder="Ej: Catéter venoso central, Sonda vesical, Línea arterial..." rows="3"
                            on:input=move |ev| {
                                let v = event_target_value(&ev);
                                patient.update(|p| p.procesos_invasivos = v.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
                            }></textarea>
                    </FormField>
                </FormSection>

                <FormSection title="Diagnóstico y Clínica" icon=move || view! { <i class="fa-solid fa-file-medical"></i> }>
                    <div class="space-y-4 md:space-y-6">
                        <FormField label="Descripción del Cuadro Clínico" icon=move || view! { <i class="fa-solid fa-comment-medical"></i> }>
                            <textarea class="form-input" placeholder="Resumen del motivo de ingreso y evolución reciente..." rows="3"
                                prop:value=move || patient.get().descripcion_ingreso
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.descripcion_ingreso = v); }></textarea>
                        </FormField>
                        
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 md:gap-6">
                            <FormField label="Diagnóstico de Ingreso" icon=move || view! { <i class="fa-solid fa-notes-medical"></i> }>
                                <textarea class="form-input" placeholder="Diagnóstico presuntivo de hospitalización..." rows="4"
                                    prop:value=move || patient.get().diagnostico_hospital
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.diagnostico_hospital = v); }></textarea>
                            </FormField>
                            <FormField label="Diagnóstico UCI Confirmado" icon=move || view! { <i class="fa-solid fa-stethoscope" style="color:var(--uci-accent);"></i> }>
                                <textarea class="form-input font-bold" style="border-color:var(--uci-accent);" placeholder="Diagnóstico definitivo de ingreso a cuidados intensivos..." rows="4"
                                    prop:value=move || patient.get().diagnostico_uci
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.diagnostico_uci = v); }></textarea>
                            </FormField>
                        </div>
                    </div>
                </FormSection>

                <div class="flex flex-col md:flex-row justify-end gap-3 md:gap-4 lg:gap-6 mt-10 md:mt-12 lg:mt-16 pb-16 md:pb-20 lg:pb-24">
                    <a href="/patients" class="btn-outline flex items-center justify-center gap-2 px-6 md:px-8 lg:px-10 h-11 md:h-12 lg:h-14 text-sm md:text-base group">
                        <i class="fa-solid fa-xmark group-hover:rotate-90 transition-transform"></i>
                        "Cancelar"
                    </a>
                    <button type="submit" class="btn-primary flex items-center justify-center gap-2 px-8 md:px-10 lg:px-12 h-11 md:h-12 lg:h-14 text-base md:text-lg" disabled=move || saving.get()>
                        {move || {
                            if saving.get() {
                                Either::Left(view! { 
                                    <span class="flex items-center gap-2">
                                        <i class="fa-solid fa-circle-notch animate-spin"></i>
                                        "Procesando..."
                                    </span>
                                })
                            } else {
                                Either::Right(view! { 
                                    <span class="flex items-center gap-2">
                                        <i class="fa-solid fa-floppy-disk"></i>
                                        "Registrar Paciente"
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
fn FormSection<F, IV>(title: &'static str, icon: F, children: Children) -> impl IntoView 
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    view! {
        <div class="glass-card p-4 md:p-6 lg:p-8 md:p-10 mb-6 md:mb-8 lg:mb-10 animate-fade-in" style="border-color:rgba(100,116,139,0.6);">
            <div class="flex items-center gap-3 md:gap-4 mb-6 md:mb-8 lg:mb-10 pb-4 md:pb-6" style="border-bottom:1px solid var(--uci-border);">
                <div class="w-10 h-10 md:w-12 lg:w-14 rounded-xl md:rounded-2xl flex items-center justify-center text-lg md:text-xl lg:text-2xl shrink-0" style="background:linear-gradient(135deg,rgba(59,130,246,0.1),rgba(99,102,241,0.2)); color:var(--uci-accent); border:1px solid rgba(59,130,246,0.1);">
                    {icon()}
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
fn FormField<F, IV>(label: &'static str, icon: F, children: Children) -> impl IntoView 
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    view! {
        <div class="space-y-2 md:space-y-3 w-full">
            <label class="form-label flex items-center gap-2 text-xs font-black uppercase tracking-widest" style="color:var(--uci-muted);">
                {icon()}
                {label}
            </label>
            <div class="relative">
                {children()}
            </div>
        </div>
    }
}