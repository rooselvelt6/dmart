use crate::api;
use crate::components::severity_badge::SeverityBadge;
use crate::components::ui_kit::{ErrorState, LoadingState};
use dmart_shared::models::Sexo;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::JsCast;

/// Altura fija usada por el renderizado virtualizado (px por fila).
const ROW_HEIGHT: f64 = 72.0;

#[component]
pub fn PatientsPage() -> impl IntoView {
    let search_debounced = RwSignal::new(String::new());
    let retry = RwSignal::new(0u32);
    let estado_filter = RwSignal::new("activos".to_string());
    let patients_resource = LocalResource::new(move || {
        let q = search_debounced.get();
        let estado = estado_filter.get();
        let _r = retry.get();
        async move {
            api::list_patients(Some(&q), Some(&estado))
                .await
                .map_err(|e| e.to_string())
        }
    });

    // Virtual scrolling: solo se renderizan las filas visibles del contenedor.
    let scroll_top = RwSignal::new(0f64);
    let viewport_h = RwSignal::new(0f64);
    let on_scroll = move |ev: web_sys::Event| {
        if let Some(target) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
        {
            scroll_top.set(target.scroll_top() as f64);
            viewport_h.set(target.client_height() as f64);
        }
    };

    // Debounce de 300ms: cada nueva tecla cancela el temporizador anterior
    // (trailing edge: la búsqueda se dispara solo tras 300ms sin escribir).
    let debounce_timer = Rc::new(RefCell::new(None::<TimeoutHandle>));
    let on_search = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        let timer = debounce_timer.clone();
        if let Some(h) = timer.borrow_mut().take() {
            h.clear();
        }
        let inner_timer = timer.clone();
        *timer.borrow_mut() = set_timeout_with_handle(
            move || {
                search_debounced.set(value);
                *inner_timer.borrow_mut() = None;
            },
            Duration::from_millis(300),
        )
        .ok();
    };

    let delete_patient = move |id: String| {
        let patient_id = id.clone();
        spawn_local(async move {
            if let Ok(()) = api::delete_patient(&patient_id).await {
                window().location().reload().unwrap_or_default();
            }
        });
    };

    let render_list = move || match patients_resource.get() {
        Some(Ok(list)) if list.is_empty() => {
            let msg = match estado_filter.get().as_str() {
                "egresados" => "No hay pacientes egresados",
                "todos" => "No se encontraron pacientes",
                _ => "No hay pacientes activos en UCI",
            };
            view! {
                <div class="glass-card p-10 text-center" style="color:var(--uci-muted);">{msg}</div>
            }
            .into_any()
        }
        Some(Ok(list)) => {
            let list: Vec<_> = list;
            if list.is_empty() {
                view! {
                    <div class="glass-card p-10 text-center" style="color:var(--uci-muted);">"No se encontraron pacientes"</div>
                }.into_any()
            } else {
                view! {
            <div class="glass-card overflow-hidden">
                <div style="overflow-y:auto; max-height:65vh;" on:scroll=on_scroll>
                    {move || {
                        let total = list.len() as f64 * ROW_HEIGHT;
                        let start = (scroll_top.get() / ROW_HEIGHT).floor() as usize;
                        let vcount = ((scroll_top.get() + viewport_h.get()).ceil() / ROW_HEIGHT).ceil() as usize + 4;
                        let end = (start + vcount).min(list.len());
                        let rows = &list[start..end];
                        view! {
                            <div class="min-w-[760px]" style=move || format!("position:relative; height:{}px;", total)>
                                {rows.iter().enumerate().map(|(i, p)| {
                                    let top_px = (start + i) as f64 * ROW_HEIGHT;
                                    let pid = p.id.clone();
                                    let delete_id = p.id.clone();
                                    view! {
                                        <div
                                            style=move || format!("position:absolute; top:{}px; left:0; right:0; height:{}px; display:grid; grid-template-columns:minmax(180px,2fr) 130px 80px 48px 60px 96px auto; align-items:center; gap:8px; padding:0 16px; border-bottom:1px solid var(--uci-border);", top_px, ROW_HEIGHT)
                                        >
                                            <div class="flex items-center gap-2 md:gap-3 min-w-0">
                                                <div class="w-8 h-8 md:w-9 md:h-9 rounded-full flex items-center justify-center shrink-0" style="background:rgba(59,130,246,0.1);">
                                                    <svg class="w-4 h-4 md:w-5 md:h-5" style="color:var(--uci-accent);" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" /></svg>
                                                </div>
                                                <span class="font-semibold text-sm truncate" style="color:var(--uci-text);">{p.nombre_completo.clone()}</span>
                                                {(!p.fecha_egreso_uci.is_empty()).then(|| view! {
                                                    <span class="px-1.5 py-0.5 rounded text-[10px] font-bold shrink-0" style="background:rgba(148,163,184,0.2); color:#94A3B8;">"Egresado"</span>
                                                })}
                                            </div>
                                            <div style="color:var(--uci-muted);">
                                                <div class="text-sm truncate">{p.cedula.clone()}</div>
                                                <div class="text-xs truncate">HC: {p.historia_clinica.clone()}</div>
                                            </div>
                                            <div class="font-semibold text-sm" style="color:var(--uci-text);">{p.edad}</div>
                                            <div class="font-bold text-sm">
                                                {match p.sexo.clone() {
                                                    Sexo::Masculino => view!{ <span style="color:#60A5FA;">M</span> },
                                                    Sexo::Femenino => view!{ <span style="color:#F472B6;">F</span> }
                                                }}
                                            </div>
                                            <div class="text-center font-extrabold" style="color:var(--uci-text);">
                                                {p.ultimo_apache_score.map(|s|s.to_string()).unwrap_or_else(|| "-".into())}
                                            </div>
                                            <div class="text-center"><SeverityBadge level=p.estado_gravedad.clone() /></div>
                                            <div class="flex flex-wrap justify-end gap-1 sm:gap-2">
                                                <a href=format!("/patients/{}", pid) class="py-1 px-2 md:py-2 md:px-3 rounded text-xs font-semibold no-underline" style="background:rgba(59,130,246,0.1); color:var(--uci-accent);">"Ver"</a>
                                                <a href=format!("/patients/{}/edit", pid) class="py-1 px-2 md:py-2 md:px-3 rounded text-xs font-semibold no-underline" style="background:rgba(16,185,129,0.1); color:var(--uci-low);">"Editar"</a>
                                                <button
                                                    on:click=move |_| {
                                                        if let Some(w) = web_sys::window()
                                                            && let Ok(true) = w.confirm_with_message("¿Está seguro de eliminar este paciente? Esta acción no se puede deshacer.")
                                                        {
                                                            delete_patient(delete_id.clone());
                                                        }
                                                    }
                                                    class="py-1 px-2 md:py-2 md:px-3 rounded text-xs font-semibold no-underline"
                                                    style="background:rgba(239,68,68,0.1); color:#EF4444;"
                                                >"Eliminar"</button>
                                            </div>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }
                    }}
                </div>
            </div>
            }.into_any()
            }
        }
        Some(Err(e)) => view! {
            <ErrorState
                message=format!("No se pudieron cargar los pacientes: {}", e)
                on_retry=Some(Callback::new(move |()| {
                    retry.update(|v| *v += 1);
                }))
            />
        }
        .into_any(),
        None => ().into_any(),
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

                <div class="flex flex-wrap gap-2 mb-4 md:mb-5">
                    {[("activos", "Activos"), ("egresados", "Egresados"), ("todos", "Todos")]
                        .into_iter()
                        .map(|(val, label)| {
                            let is_active = move || estado_filter.get() == val;
                            view! {
                                <button
                                    type="button"
                                    class=move || if is_active() {
                                        "btn-primary px-4 py-2".to_string()
                                    } else {
                                        "btn-outline px-4 py-2".to_string()
                                    }
                                    on:click=move |_| estado_filter.set(val.to_string())
                                >{label}</button>
                            }
                        })
                        .collect_view()}
                </div>

    <Suspense fallback=move || view! { <LoadingState label="Cargando pacientes..." /> }>
                    {move || render_list()}
                </Suspense>
            </div>
        }
}
