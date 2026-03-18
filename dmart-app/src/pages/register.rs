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

    // Edad calculada reactivamente
    let edad_calculada = Memo::new(move |_| {
        let p = patient.get();
        if p.fecha_nacimiento.is_empty() { return 0u8; }
        if let Ok(dob) = chrono::NaiveDate::parse_from_str(&p.fecha_nacimiento, "%Y-%m-%d") {
            let today = chrono::Utc::now().date_naive();
            today.years_since(dob).unwrap_or(0).min(150) as u8
        } else { 0 }
    });

    // Tiempo estadía hospitalaria calculado
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

    view! {
        <div class="page-enter w-full max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-6 md:py-10">
            <div class="mb-6 md:mb-8">
                <a href="/patients" class="text-sm text-slate-500 hover:text-slate-300 transition-colors inline-flex items-center gap-2 mb-3 md:mb-4 no-underline">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
                    </svg>
                    "Volver a Pacientes"
                </a>
                <h1 class="text-2xl md:text-3xl lg:text-4xl font-extrabold text-slate-200 mb-2">"Registro de Nuevo Paciente"</h1>
                <p class="text-slate-500 text-sm md:text-base">"Complete todos los campos del expediente clínico"</p>
            </div>

            {move || error.get().map(|e| view! {
                <div class="bg-red-500/10 border border-red-500/30 rounded-xl p-4 mb-5 text-red-400 text-sm animate-pulse">
                    <div class="flex items-center gap-2">
                        <svg class="w-5 h-5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                        </svg>
                        {e}
                    </div>
                </div>
            })}

            <form on:submit=on_submit class="space-y-6">
                // ─── Sección 1: Identificación ──────────────────────────────
                <FormSection title="Identificación del Paciente" icon=move || view! {
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                    </svg>
                }>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <FormField label="Nombre *" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="Nombre(s)" required
                                prop:value=move || patient.get().nombre
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.nombre = v); } />
                        </FormField>
                        <FormField label="Apellido *" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="Apellido(s)" required
                                prop:value=move || patient.get().apellido
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.apellido = v); } />
                        </FormField>
                        <FormField label="Cédula de Identidad *" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V8a2 2 0 00-2-2h-5m-4 0V5a2 2 0 114 0v1m-4 0a2 2 0 104 0m-5 8a2 2 0 100-4 2 2 0 000 4zm0 0c1.333 0 4 1 4 3" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="V-00000000" required
                                prop:value=move || patient.get().cedula
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.cedula = v); } />
                        </FormField>
                        <FormField label="Nº Historia Clínica *" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="HC-00000" required
                                prop:value=move || patient.get().historia_clinica
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.historia_clinica = v); } />
                        </FormField>
                        <FormField label="Sexo" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
                            </svg>
                        }>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.sexo = if v == "Masculino" { Sexo::Masculino } else { Sexo::Femenino });
                                }>
                                <option value="Masculino">"Masculino"</option>
                                <option value="Femenino">"Femenino"</option>
                            </select>
                        </FormField>
                        <FormField label="Fecha de Nacimiento *" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                            </svg>
                        }>
                            <input class="form-input" type="date" required
                                prop:value=move || patient.get().fecha_nacimiento
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.fecha_nacimiento = v); } />
                            <div class="mt-2 flex items-center gap-2 text-xs font-bold text-uci-accent bg-uci-accent/5 p-2 rounded-lg border border-uci-accent/10">
                                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                                </svg>
                                "Edad estimada: " {move || { let e = edad_calculada.get(); if e > 0 { format!("{} años", e) } else { "—".into() } }}
                            </div>
                        </FormField>
                    </div>

                    // Color de piel
                    <div class="mt-6 p-4 rounded-xl border border-uci-border/40 bg-uci-bg/30">
                        <label class="form-label mb-4 flex items-center gap-2">
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01" />
                            </svg>
                            "Color de Piel (Escala Fitzpatrick)"
                        </label>
                        <SkinPicker
                            value=Signal::derive(move || patient.get().color_piel)
                            on_change=move |v| patient.update(|p| p.color_piel = v)
                        />
                    </div>
                </FormSection>

                // ─── Sección 2: Procedencia ──────────────────────────────────
                <FormSection title="Procedencia y Contacto" icon=move || view! {
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z" />
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 11a3 3 0 11-6 0 3 3 0 016 0z" />
                    </svg>
                }>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <FormField label="Nacionalidad" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 002 2h4.605M8.02 4.027A10 10 0 0120 12c0 1.091-.174 2.141-.497 3.127m-1.782 3.127A9.957 9.957 0 0112 22a9.96 9.96 0 01-6.191-2.144m1.782-3.127a9.956 9.956 0 01-3.127-1.782m0 0a9.96 9.96 0 01-2.144-6.191c0-1.091.174-2.141.497-3.127" />
                            </svg>
                        }>
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
                        <FormField label="País" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="País de residencia"
                                prop:value=move || patient.get().pais
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.pais = v); } />
                        </FormField>
                        <FormField label="Estado" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="Estado / Provincia"
                                prop:value=move || patient.get().estado
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.estado = v); } />
                        </FormField>
                        <FormField label="Ciudad" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z" />
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 11a3 3 0 11-6 0 3 3 0 016 0z" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="Ciudad"
                                prop:value=move || patient.get().ciudad
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.ciudad = v); } />
                        </FormField>
                        <FormField label="Lugar de Nacimiento" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9h18" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="Ciudad / País de nacimiento"
                                prop:value=move || patient.get().lugar_nacimiento
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.lugar_nacimiento = v); } />
                        </FormField>
                        <FormField label="Familiar Encargado" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                            </svg>
                        }>
                            <input class="form-input" type="text" placeholder="Nombre del responsable"
                                prop:value=move || patient.get().familiar_encargado
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.familiar_encargado = v); } />
                        </FormField>
                        <div class="md:col-span-2">
                            <FormField label="Dirección de Residencia" icon=move || view! {
                                <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
                                </svg>
                            }>
                                <input class="form-input" type="text" placeholder="Dirección completa"
                                    prop:value=move || patient.get().direccion
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.direccion = v); } />
                            </FormField>
                        </div>
                    </div>
                </FormSection>

                // ─── Sección 3: Ingreso Hospitalario ────────────────────────
                <FormSection title="Ingreso Hospitalario" icon=move || view! {
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
                    </svg>
                }>
                    <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
                        <FormField label="Fecha Ingreso Hospital *" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                            </svg>
                        }>
                            <input class="form-input" type="datetime-local" required
                                prop:value=move || { let p = patient.get(); p.fecha_ingreso_hospital.trim_end_matches('Z').chars().take(16).collect::<String>() }
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.fecha_ingreso_hospital = format!("{}:00Z", v));
                                } />
                        </FormField>
                        <FormField label="Fecha Ingreso UCI *" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                            </svg>
                        }>
                            <input class="form-input" type="datetime-local" required
                                prop:value=move || { let p = patient.get(); p.fecha_ingreso_uci.trim_end_matches('Z').chars().take(16).collect::<String>() }
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.fecha_ingreso_uci = format!("{}:00Z", v));
                                } />
                        </FormField>
                        <FormField label="Estadía Pre-UCI" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                            </svg>
                        }>
                            <div class="form-input flex items-center bg-uci-accent/5 border-uci-accent/20 text-uci-accent font-bold h-[42px]">
                                {move || tiempo_estadia.get()}
                            </div>
                        </FormField>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
                        <FormField label="Tipo de Admisión" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                            </svg>
                        }>
                            <select class="form-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patient.update(|p| p.tipo_admision = if v == "Urgente" { TipoAdmision::Urgente } else { TipoAdmision::Electiva });
                                }>
                                <option value="Urgente">"🚨 Urgente"</option>
                                <option value="Electiva">"📅 Electiva"</option>
                            </select>
                        </FormField>
                        <FormField label="Referido / Traslado" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
                            </svg>
                        }>
                            <div class="flex items-center gap-4 h-[42px] px-4 rounded-xl border border-uci-border/40 bg-uci-bg/20">
                                <Toggle
                                    value=Signal::derive(move || patient.get().migracion_otro_centro)
                                    on_change=move |v| patient.update(|p| p.migracion_otro_centro = v)
                                />
                                <span class="text-xs font-semibold text-uci-muted uppercase tracking-wider">
                                    {move || if patient.get().migracion_otro_centro { "Desde otro centro" } else { "Ingreso directo" }}
                                </span>
                            </div>
                        </FormField>
                    </div>

                    {move || if patient.get().migracion_otro_centro {
                        Either::Left(view! {
                            <div class="mb-8 animate-slide-in">
                                <FormField label="Centro de Salud de Origen" icon=move || view! {
                                    <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
                                    </svg>
                                }>
                                    <input class="form-input" type="text" placeholder="Nombre completo del hospital/clínica"
                                        on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.centro_origen = Some(v)); } />
                                </FormField>
                            </div>
                        })
                    } else { Either::Right(view! { <span></span> }) }}

                    // Soporte vital — toggles
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
                        <div class="glass-card p-4 flex items-center justify-between border-uci-accent/20 bg-uci-accent/5">
                            <div class="flex items-center gap-3">
                                <div class="w-10 h-10 rounded-full bg-uci-accent/10 flex items-center justify-center text-uci-accent">
                                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                                    </svg>
                                </div>
                                <div>
                                    <div class="font-bold text-uci-text text-sm mb-0.5">"Ventilación Mecánica"</div>
                                    <div class="text-[10px] uppercase font-bold text-uci-muted tracking-tight">"Soporte invasivo"</div>
                                </div>
                            </div>
                            <Toggle
                                value=Signal::derive(move || patient.get().ventilacion_mecanica)
                                on_change=move |v| patient.update(|p| p.ventilacion_mecanica = v)
                            />
                        </div>
                    </div>

                    // Procesos invasivos
                    <FormField label="Procesos Invasivos (uno por línea)" icon=move || view! {
                        <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01" />
                        </svg>
                    }>
                        <textarea class="form-input" placeholder="Ej: Catéter venoso central&#10;Sonda vesical&#10;Línea arterial" rows="3"
                            on:input=move |ev| {
                                let v = event_target_value(&ev);
                                patient.update(|p| p.procesos_invasivos = v.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
                            }></textarea>
                    </FormField>
                </FormSection>

                // ─── Sección 4: Diagnóstico Clínico ──────────────────────────
                <FormSection title="Diagnóstico y Clínica" icon=move || view! {
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                    </svg>
                }>
                    <div class="space-y-6">
                        <FormField label="Descripción del Ingreso" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                            </svg>
                        }>
                            <textarea class="form-input" placeholder="Descripción general del ingreso..." rows="2"
                                prop:value=move || patient.get().descripcion_ingreso
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.descripcion_ingreso = v); }></textarea>
                        </FormField>
                        <FormField label="Antecedentes Médicos" icon=move || view! {
                            <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                            </svg>
                        }>
                            <textarea class="form-input" placeholder="HTA, DM, enfermedades previas..." rows="2"
                                prop:value=move || patient.get().antecedentes
                                on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.antecedentes = v); }></textarea>
                        </FormField>
                        
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                            <FormField label="Diagnóstico — Hospital" icon=move || view! {
                                <svg class="w-4 h-4 text-uci-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
                                </svg>
                            }>
                                <textarea class="form-input" placeholder="Diagnóstico de entrada..." rows="3"
                                    prop:value=move || patient.get().diagnostico_hospital
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.diagnostico_hospital = v); }></textarea>
                            </FormField>
                            <FormField label="Diagnóstico — UCI" icon=move || view! {
                                <svg class="w-4 h-4 text-uci-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                                </svg>
                            }>
                                <textarea class="form-input border-uci-accent/30" placeholder="Confirmación diagnóstica UCI..." rows="3"
                                    prop:value=move || patient.get().diagnostico_uci
                                    on:input=move |ev| { let v = event_target_value(&ev); patient.update(|p| p.diagnostico_uci = v); }></textarea>
                            </FormField>
                        </div>
                    </div>
                </FormSection>

                // ─── Submit ──────────────────────────────────────────────────
                <div class="flex flex-col md:flex-row justify-end gap-4 mt-12 pb-20">
                    <a href="/patients" class="btn-outline flex items-center justify-center gap-2 group">
                        <svg class="w-4 h-4 group-hover:-translate-x-1 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
                        </svg>
                        "Cancelar"
                    </a>
                    <button type="submit" class="btn-primary flex items-center justify-center gap-2 min-w-[200px]" disabled=move || saving.get()>
                        {move || {
                            if saving.get() {
                                Either::Left(view! { 
                                    <span class="flex items-center gap-2">
                                        <span class="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></span>
                                        "Guardando..."
                                    </span>
                                })
                            } else {
                                Either::Right(view! { 
                                    <span class="flex items-center gap-2">
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
                                        </svg>
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

// ─── Helpers ────────────────────────────────────────────────────────────────

#[component]
fn FormSection<F, IV>(title: &'static str, icon: F, children: Children) -> impl IntoView 
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    view! {
        <div class="glass-card p-4 sm:p-6 lg:p-8 mb-6 md:mb-8 animate-fade-in">
            <div class="flex items-center gap-3 mb-6 pb-4 border-b border-slate-700/50">
                <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-orange-500/20 to-red-500/20 flex items-center justify-center text-orange-500 shadow-inner flex-shrink-0">
                    {icon()}
                </div>
                <h2 class="text-base md:text-lg font-extrabold text-slate-200 tracking-tight uppercase">{title}</h2>
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
        <div class="space-y-2 w-full">
            <label class="form-label flex items-center gap-2 text-sm font-medium text-slate-300">
                {icon()}
                {label}
            </label>
            {children()}
        </div>
    }
}
