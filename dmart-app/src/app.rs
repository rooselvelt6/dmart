use crate::pages::{
    dashboard::DashboardPage, login::LoginPage, measurement::MeasurementPage,
    patient_detail::PatientDetailPage, patient_edit::PatientEditPage, patients::PatientsPage,
    register::RegisterPage,
};
use gloo_storage::{LocalStorage, Storage};
use leptos::either::Either;
use leptos::prelude::*;
use leptos_router::components::{Redirect, Route, Router, Routes, A};
use leptos_router::hooks::use_location;
use leptos_router::path;

#[component]
pub fn App() -> impl IntoView {
    let is_auth = move || LocalStorage::get::<String>("dmart_auth").is_ok();

    view! {
        <Router>
            <div class="flex min-h-screen" style="background:#0A0E1A">
                <Show when=is_auth fallback=|| ()>
                    <NavSidebar />
                </Show>

                <main class="main-content" style=move || if is_auth() { "" } else { "margin-left: 0; width: 100%" }>
                    <Routes fallback=|| view! { "Pagina no encontrada" }>
                        <Route path=path!("/login") view=LoginPage />

                        <Route path=path!("/") view=move || {
                            if !is_auth() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <DashboardPage /> })
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
fn NavSidebar() -> impl IntoView {
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

    view! {
        <nav class="nav-sidebar">
            <div style="padding:24px 16px 16px; border-bottom: 1px solid #2A3547;">
                <div style="display:flex; align-items:center; gap:10px; margin-bottom:4px;">
                    <div style="
                        width:36px; height:36px; border-radius:10px;
                        background:linear-gradient(135deg,#3B82F6,#6366F1);
                        display:flex; align-items:center; justify-content:center;
                        box-shadow: 0 4px 12px rgba(59,130,246,0.4);
                        font-size:18px; font-weight:900; color:white; flex-shrink:0;
                    ">"+"</div>
                    <div>
                        <div style="font-weight:700; font-size:16px; color:#E2E8F0;">"UCI - DMART"</div>
                        <div style="font-size:10px; color:#475569; text-transform:uppercase; letter-spacing:1px;">"Cuidados Intensivos"</div>
                    </div>
                </div>
            </div>

            <div style="padding:12px 8px; flex:1; overflow-y:auto;">
                <div style="font-size:10px; color:#475569; text-transform:uppercase; letter-spacing:1px; padding:8px 8px 4px; font-weight:600;">"PRINCIPAL"</div>

                <A href="/" attr:class=move || format!("nav-link {}", if is_active("/") && path() == "/" { "active" } else { "" })>
                    {svg_icon("M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6")}
                    "Dashboard"
                </A>

                <A href="/patients" attr:class=move || format!("nav-link {}", if is_active("/patients") { "active" } else { "" })>
                    {svg_icon("M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z")}
                    "Pacientes"
                </A>

                <div style="font-size:10px; color:#475569; text-transform:uppercase; letter-spacing:1px; padding:12px 8px 4px; font-weight:600;">"ACCIONES"</div>

                <A href="/patients/new" attr:class=move || format!("nav-link {}", if path() == "/patients/new" { "active" } else { "" })>
                    {svg_icon("M18 9v3m0 0v3m0-3h3m-3 0h-3m-2-5a4 4 0 11-8 0 4 4 0 018 0zM3 20a6 6 0 0112 0v1H3v-1z")}
                    "Nuevo Paciente"
                </A>
            </div>

            <div style="padding:16px; border-top:1px solid #2A3547;">
                <button
                    on:click=move |_| {
                        let _ = LocalStorage::delete("dmart_auth");
                        window().location().reload().unwrap_or_default();
                    }
                    style="
                        width:100%; padding:10px 16px; 
                        background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3);
                        border-radius:10px; color:#EF4444; font-size:13px; font-weight:600;
                        cursor:pointer; display:flex; align-items:center; justify-content:center; gap:8px;
                        transition:all 0.2s;
                    "
                >
                    <svg style="width:16px;height:16px;" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                    </svg>
                    "Cerrar Sesión"
                </button>
                <div style="font-size:11px; color:#334155; text-align:center; margin-top:12px;">
                    "UCI-DMART v1.0"
                    <br />
                    <span style="color:#1E2537;">"Rust + Leptos + WASM"</span>
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
