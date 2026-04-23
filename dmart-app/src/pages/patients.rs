use leptos::prelude::*;
use crate::api;
use crate::components::severity_badge::SeverityBadge;
use dmart_shared::models::Sexo;

#[component]
pub fn PatientsPage() -> impl IntoView {
    let search_debounced = RwSignal::new(String::new());

    let patients = LocalResource::new(move || {
        let q = search_debounced.get();
        async move { api::list_patients(Some(&q)).await.unwrap_or_default() }
    });

    let on_search = move |ev: web_sys::Event| {
        search_debounced.set(event_target_value(&ev));
    };

    view! {
        <div class="page-enter">
            <div class="flex flex-col md:flex-row justify-between items-start gap-4 mb-5 md:mb-6">
                <div>
                    <h1 class="text-xl md:text-2xl lg:text-3xl font-extrabold" style="color:var(--uci-text); margin:0 0 4px;">"Registro de Pacientes"</h1>
                    <p style="color:var(--uci-muted); font-size:13px; md:text-14px; margin:0;">"Busque, revise y gestione los pacientes de la UCI"</p>
                </div>
                <a href="/patients/new" class="btn-primary text-center no-underline whitespace-nowrap">"+ Nuevo Paciente"</a>
            </div>

            <div class="mb-4 md:mb-5">
                <input type="text" class="form-input w-full" placeholder="Buscar por nombre, cedula o historia clinica..." on:input=on_search />
            </div>

            <Suspense fallback=move || view! { <div class="text-center p-10" style="color:var(--uci-muted);">"Cargando..."</div> }>
                {move || patients.get().map(|list| {
                    view! {
                        <div class="glass-card overflow-x-auto">
                            <table class="w-full text-left border-collapse min-w-[600px]">
                                <thead>
                                    <tr style="border-bottom:1px solid var(--uci-border);">
                                        <th class="p-3 md:p-4 text-xs font-bold uppercase" style="color:var(--uci-muted);">"Paciente"</th>
                                        <th class="p-3 md:p-4 text-xs font-bold uppercase hidden md:table-cell" style="color:var(--uci-muted);">"ID / HC"</th>
                                        <th class="p-3 md:p-4 text-xs font-bold uppercase hidden sm:table-cell" style="color:var(--uci-muted);">"Edad"</th>
                                        <th class="p-3 md:p-4 text-xs font-bold uppercase" style="color:var(--uci-muted);">"Sexo"</th>
                                        <th class="p-3 md:p-4 text-xs font-bold uppercase text-center" style="color:var(--uci-muted);">"Score"</th>
                                        <th class="p-3 md:p-4 text-xs font-bold uppercase text-center" style="color:var(--uci-muted);">"Estado"</th>
                                        <th class="p-3 md:p-4 text-xs font-bold uppercase text-right" style="color:var(--uci-muted);">"Acciones"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list.iter().map(|p| {
                                        let pid = p.id.clone();
                                        view! {
                                            <tr style="border-bottom:1px solid var(--uci-border);">
                                                <td class="p-3 md:p-4">
                                                    <div class="flex items-center gap-2 md:gap-3">
                                                        <div class="w-8 h-8 md:w-10 md:h-10 rounded-full flex items-center justify-center shrink-0" style="background:rgba(59,130,246,0.1);">
                                                            <svg class="w-4 h-4 md:w-5 md:h-5" style="color:var(--uci-accent);" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" /></svg>
                                                        </div>
                                                        <span class="font-semibold text-sm" style="color:var(--uci-text);">{p.nombre_completo.clone()}</span>
                                                    </div>
                                                </td>
                                                <td class="p-3 md:p-4 hidden md:table-cell" style="color:var(--uci-muted);">
                                                    <div class="text-sm">{p.cedula.clone()}</div>
                                                    <div class="text-xs">HC: {p.historia_clinica.clone()}</div>
                                                </td>
                                                <td class="p-3 md:p-4 hidden sm:table-cell font-semibold" style="color:var(--uci-text);">{p.edad}" años"</td>
                                                <td class="p-3 md:p-4">
                                                    {match p.sexo.clone() { 
                                                        Sexo::Masculino => view!{ <span style="color:#60A5FA;">M</span> }, 
                                                        Sexo::Femenino => view!{ <span style="color:#F472B6;">F</span> } 
                                                    }}
                                                </td>
                                                <td class="p-3 md:p-4 text-center font-extrabold text-lg" style="color:var(--uci-text);">
                                                    {p.ultimo_apache_score.map(|s|s.to_string()).unwrap_or_else(|| "-".into())}
                                                </td>
                                                <td class="p-3 md:p-4 text-center"><SeverityBadge level=p.estado_gravedad.clone() /></td>
                                                <td class="p-3 md:p-4 text-right">
                                                    <div class="flex flex-col sm:flex-row justify-end gap-2">
                                                        <a href=format!("/patients/{}", pid) class="py-1 px-2 md:py-2 md:px-3 rounded text-xs font-semibold no-underline" style="background:rgba(59,130,246,0.1); color:var(--uci-accent);">"Ver"</a>
                                                        <a href=format!("/patients/{}/edit", pid) class="py-1 px-2 md:py-2 md:px-3 rounded text-xs font-semibold no-underline" style="background:rgba(16,185,129,0.1); color:var(--uci-low);">"Editar"</a>
                                                    </div>
                                                </td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                    }
                })}
            </Suspense>
        </div>
    }
}
