use leptos::prelude::*;
use leptos_router::hooks::*;
use dmart_shared::models::*;
use crate::api;
use crate::components::severity_badge::SeverityBadge;

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
            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:24px;">
                <div>
                    <h1 style="font-size:26px; font-weight:800; color:#E2E8F0; margin:0 0 4px;">"Registro de Pacientes"</h1>
                    <p style="color:#475569; font-size:14px; margin:0;">"Busque, revise y gestione los pacientes de la UCI"</p>
                </div>
                <a href="/patients/new" class="btn-primary" style="text-decoration:none;">"+ Nuevo Paciente"</a>
            </div>

            <div style="margin-bottom:20px;">
                <input type="text" class="form-input" placeholder="Buscar por nombre, cedula o historia clinica..." on:input=on_search style="width:100%;" />
            </div>

            <Suspense fallback=move || view! { <div style="text-align:center; padding:60px;">"Cargando..."</div> }>
                {move || patients.get().map(|list| {
                    view! {
                        <div class="glass-card" style="overflow-x:auto;">
                            <table style="width:100%; text-align:left; border-collapse:collapse;">
                                <thead>
                                    <tr style="border-bottom:1px solid #2A3547;">
                                        <th style="padding:16px; color:#64748B; font-size:11px; font-weight:700; text-transform:uppercase;">"Paciente"</th>
                                        <th style="padding:16px; color:#64748B; font-size:11px; font-weight:700; text-transform:uppercase;">"ID / HC"</th>
                                        <th style="padding:16px; color:#64748B; font-size:11px; font-weight:700; text-transform:uppercase;">"Edad"</th>
                                        <th style="padding:16px; color:#64748B; font-size:11px; font-weight:700; text-transform:uppercase;">"Sexo"</th>
                                        <th style="padding:16px; color:#64748B; font-size:11px; font-weight:700; text-transform:uppercase; text-align:center;">"Score"</th>
                                        <th style="padding:16px; color:#64748B; font-size:11px; font-weight:700; text-transform:uppercase; text-align:center;">"Estado"</th>
                                        <th style="padding:16px; color:#64748B; font-size:11px; font-weight:700; text-transform:uppercase; text-align:right;">"Acciones"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list.iter().map(|p| {
                                        let pid = p.id.clone();
                                        view! {
                                            <tr style="border-bottom:1px solid #2A3547;">
                                                <td style="padding:16px;">
                                                    <div style="display:flex; align-items:center; gap:12px;">
                                                        <div style="width:40px; height:40px; border-radius:50%; background:#3B82F6/10; display:flex; align-items:center; justify-content:center; color:#3B82F6;">
                                                            <svg style="width:20px;height:20px;" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" /></svg>
                                                        </div>
                                                        <span style="font-weight:600; color:#E2E8F0;">{p.nombre_completo.clone()}</span>
                                                    </div>
                                                </td>
                                                <td style="padding:16px; color:#94A3B8;">
                                                    <div>{p.cedula.clone()}</div>
                                                    <div style="font-size:12px;">HC: {p.historia_clinica.clone()}</div>
                                                </td>
                                                <td style="padding:16px; color:#E2E8F0; font-weight:600;">{p.edad}" anos"</td>
                                                <td style="padding:16px;">
                                                    {match p.sexo.clone() { 
                                                        Sexo::Masculino => view!{ <span style="color:#60A5FA;">M</span> }, 
                                                        Sexo::Femenino => view!{ <span style="color:#F472B6;">F</span> } 
                                                    }}
                                                </td>
                                                <td style="padding:16px; text-align:center; font-size:20px; font-weight:800; color:#E2E8F0;">
                                                    {p.ultimo_apache_score.map(|s|s.to_string()).unwrap_or_else(|| "-".into())}
                                                </td>
                                                <td style="padding:16px; text-align:center;"><SeverityBadge level=p.estado_gravedad.clone() /></td>
                                                <td style="padding:16px; text-align:right;">
                                                    <div style="display:flex; justify-content:flex-end; gap:8px;">
                                                        <a href=format!("/patients/{}", pid) style="padding:8px 12px; border-radius:6px; background:#3B82F6/10; color:#3B82F6; font-size:12px; font-weight:600; text-decoration:none;">"Ver"</a>
                                                        <a href=format!("/patients/{}/edit", pid) style="padding:8px 12px; border-radius:6px; background:#10B981/10; color:#10B981; font-size:12px; font-weight:600; text-decoration:none;">"Editar"</a>
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
