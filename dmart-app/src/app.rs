use crate::components::theme_toggle::ThemeToggle;
use crate::pages::{
    dashboard::DashboardPage, login::LoginPage, measurement::MeasurementPage,
    patient_detail::PatientDetailPage, patient_edit::PatientEditPage, patients::PatientsPage,
    register::RegisterPage, uci_stats::UciStats,
};
use gloo_storage::{LocalStorage, Storage};
use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Redirect, Route, Router, Routes, A};
use leptos_router::hooks::*;
use leptos_router::path;

use crate::stores::{fetch_patients_cached, load_patients_cached};

#[component]
pub fn App() -> impl IntoView {
    let is_auth = move || LocalStorage::get::<String>("dmart_auth").is_ok();
    let sidebar_open = RwSignal::new(false);
    let _ = crate::stores::create_theme_store();

    let preloaded = RwSignal::new(load_patients_cached().unwrap_or_default());
    spawn_local(async move {
        let fresh = fetch_patients_cached().await;
        preloaded.set(fresh);
    });

    view! {
        <Router>
            <div class="flex flex-col md:flex-row min-h-screen" style="background:var(--uci-bg)">
                <Show when=is_auth fallback=|| ()>
                    <NavSidebar sidebar_open />
                </Show>

                <main class="w-full md:ml-[260px] p-4 md:p-8" style=move || if is_auth() { "" } else { "margin-left: 0; width: 100%" }>
                    <Show when=is_auth>
                        <button
                            on:click=move |_| sidebar_open.update(|o| *o = !*o)
                            class="md:hidden fixed top-4 left-4 z-30 p-2 rounded-lg shadow-lg"
                            style="background:var(--uci-surface); border:1px solid var(--uci-border);"
                        >
                            <svg style="width:24px;height:24px;" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
                            </svg>
                        </button>
                    </Show>
                    <Routes fallback=|| view! { "Pagina no encontrada" }>
                        <Route path=path!("/login") view=LoginPage />

                        <Route path=path!("/") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <DashboardPage /> })
                            }
                        } />

                        <Route path=path!("/stats") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <UciStats /> })
                            }
                        } />

                        <Route path=path!("/patients") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <PatientsPage /> })
                            }
                        } />

                        <Route path=path!("/patients/new") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <RegisterPage /> })
                            }
                        } />

                        <Route path=path!("/patients/:id") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <PatientDetailPage /> })
                            }
                        } />

                        <Route path=path!("/patients/:id/edit") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <PatientEditPage /> })
                            }
                        } />

                        <Route path=path!("/patients/:id/measure") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <MeasurementPage /> })
                            }
                        } />
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

#[component]
fn NavSidebar(sidebar_open: RwSignal<bool>) -> impl IntoView {
    let location = use_location();
    let path = move || location.pathname.get();

    let is_active = move |target: &str| {
        let p = path();
        if target == "/" {
            p == "/"
        } else {
            p.starts_with(target)
        }
    };

    let is_active_exact = move |target: &str| {
        path() == target
    };

    let _is_active_query = move |target: &str, query_str: &str| {
        path().starts_with(target) && location.search.get().contains(query_str)
    };

    let active_patient_id = move || {
        let p = path();
        if p.starts_with("/patients/") && !p.starts_with("/patients/new") {
            let parts: Vec<&str> = p.split('/').collect();
            if parts.len() >= 3 {
                return Some(parts[2].to_string());
            }
        }
        None
    };

    let close_sidebar = move |_: web_sys::MouseEvent| {
        sidebar_open.set(false);
    };

    view! {
        <nav class=move || format!(
            "nav-sidebar {} {}",
            if sidebar_open.get() { "open" } else { "" },
            if sidebar_open.get() { "fixed inset-0 z-50" } else { "hidden md:block fixed left-0 top-0 z-50 h-screen" }
        ) style="background:var(--uci-surface);">
            <div style="padding:24px 16px 16px; border-bottom:1px solid var(--uci-border);">
                <div style="display:flex; align-items:center; gap:10px; margin-bottom:4px;">
                    <div style="
                        width:36px; height:36px; border-radius:10px;
                        background:linear-gradient(135deg,#3B82F6,#6366F1);
                        display:flex; align-items:center; justify-content:center;
                        box-shadow: 0 4px 12px rgba(59,130,246,0.4);
                        font-size:18px; font-weight:900; color:white; flex-shrink:0;
                    ">"+"</div>
                    <div class="hidden md:block">
                        <div style="font-weight:700; font-size:16px; color:var(--uci-text);">"UCI - DMART"</div>
                        <div style="font-size:10px; color:var(--uci-muted); text-transform:uppercase; letter-spacing:1px;">"Cuidados Intensivos"</div>
                    </div>
                    <button
                        on:click=close_sidebar
                        class="md:hidden ml-auto p-2 hover:bg-uci-border/30 rounded-lg"
                    >
                        <svg style="width:20px;height:20px;" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>
            </div>

            <div style="padding:12px 8px; flex:1; overflow-y:auto;">
                <div style="font-size:10px; color:var(--uci-muted); text-transform:uppercase; letter-spacing:1px; padding:8px 8px 4px; font-weight:600;">"PRINCIPAL"</div>

                <A href="/" attr:class=move || format!("nav-link {}", if is_active_exact("/") { "active" } else { "" })>
                    <i class="fa-solid fa-house w-6 text-center text-lg" style="color:#10B981;"></i>
                    "Dashboard"
                </A>

                <A href="/patients" attr:class=move || format!("nav-link {}", if is_active_exact("/patients") { "active" } else { "" })>
                    <i class="fa-solid fa-users w-6 text-center text-lg" style="color:#3B82F6;"></i>
                    "Pacientes"
                </A>

                <A href="/stats" attr:class=move || format!("nav-link {}", if is_active("/stats") { "active" } else { "" })>
                    <i class="fa-solid fa-chart-bar w-6 text-center text-lg" style="color:#8B5CF6;"></i>
                    "Estadísticas"
                </A>

                <div style="font-size:10px; color:var(--uci-muted); text-transform:uppercase; letter-spacing:1px; padding:12px 8px 4px; font-weight:600;">"ACCIONES"</div>

                <A href="/patients/new" attr:class=move || format!("nav-link {}", if is_active_exact("/patients/new") { "active" } else { "" })>
                    <i class="fa-solid fa-user-plus w-6 text-center text-lg" style="color:#F59E0B;"></i>
                    "Nuevo Paciente"
                </A>

                {move || active_patient_id().map(|pid| {
                    let base = format!("/patients/{}", pid);
                    view! {
                        <div class="mt-4 pt-4 border-t border-uci-border sidebar-quick-fade">
                            <div style="font-size:10px; color:var(--uci-accent); text-transform:uppercase; letter-spacing:1px; padding:4px 8px 8px; font-weight:900;">"PACIENTE ACTIVO"</div>
                            
                            <A href=base.clone() attr:class="nav-link">
                                <i class="fa-solid fa-id-card w-6 text-center text-lg" style="color:#EC4899;"></i>
                                "Expediente / Perfil"
                            </A>

                            <A href=format!("{}/measure", base) attr:class="nav-link">
                                <i class="fa-solid fa-calculator w-6 text-center text-lg" style="color:#06B6D4;"></i>
                                "Medir Escalas"
                            </A>
                        </div>
                    }
                })}
            </div>

            <div style="padding:16px; border-top:1px solid var(--uci-border);">
                <div style="margin-bottom:12px;">
                    <ThemeToggle />
                </div>
                <button
                    on:click=move |_| {
                        let _ = LocalStorage::delete("dmart_auth");
                        window().location().reload().unwrap_or_default();
                    }
                    style="
                        width:100%; padding:10px 16px; 
                        background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3);
                        border-radius:10px; color:var(--uci-critical); font-size:13px; font-weight:600;
                        cursor:pointer; display:flex; align-items:center; justify-content:center; gap:8px;
                        transition:all 0.2s;
                    "
                >
                    <svg style="width:16px;height:16px;" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                    </svg>
                    "Cerrar Sesión"
                </button>
                <div style="font-size:11px; color:var(--uci-muted); text-align:center; margin-top:12px;">
                    "UCI-DMART v2.0"
                    <br />
                    <span style="color:var(--uci-muted);">"Rust + Leptos + WASM"</span>
                </div>
            </div>
        </nav>
    }
}

fn svg_icon(path: &str) -> impl IntoView {
    let path = path.to_string();
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" style="width:18px;height:18px;flex-shrink:0;" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d=path />
        </svg>
    }
}
