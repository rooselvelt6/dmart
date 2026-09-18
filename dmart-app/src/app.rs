use crate::components::theme_toggle::ThemeToggle;
use crate::pages::{
    admin::AdminPage, dashboard::DashboardPage, login::LoginPage, measurement::MeasurementPage,
    patient_detail::PatientDetailPage, patient_edit::PatientEditPage, patients::PatientsPage,
    perfil::PerfilPage, register::RegisterPage, support::SupportConsole,
};
use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{A, Redirect, Route, Router, Routes};
use leptos_router::hooks::*;
use leptos_router::path;

use crate::stores::start_session_refresh;
use crate::stores::{
    clear_session, current_user, fetch_patients_cached, has_token, is_admin, load_patients_cached,
    save_user, user_has,
};

#[component]
pub fn App() -> impl IntoView {
    let (is_auth, set_is_auth) = signal(has_token());
    provide_context(set_is_auth);
    let sidebar_open = RwSignal::new(false);
    let _ = crate::stores::create_theme_store();
    if has_token() {
        start_session_refresh();
        // Las sesiones creadas antes del gating solo guardaban el token: si no
        // hay identidad, se recupera con `/auth/me` (y valida que el token
        // siga vigente). La recarga hace que el gating por rol se reevalúe.
        spawn_local(async move {
            if current_user().is_none() {
                match crate::api::me().await {
                    Ok(u) => save_user(&u),
                    Err(_) => {
                        clear_session();
                        set_is_auth.set(false);
                    }
                }
                window().location().reload().unwrap_or_default();
            }
        });
    }

    // Suscripción en tiempo real a eventos del servidor (nuevas mediciones).
    let _realtime = crate::stores::use_realtime();

    let preloaded = RwSignal::new(load_patients_cached().unwrap_or_default());
    spawn_local(async move {
        let fresh = fetch_patients_cached().await;
        preloaded.set(fresh);
    });

    view! {
        <Router>
            <div class="flex flex-col md:flex-row min-h-screen" style="background:var(--uci-bg)">
                <Show when=move || is_auth.get() fallback=|| ()>
                    <NavSidebar sidebar_open />
                </Show>

                <main class="w-full md:ml-[260px] p-4 md:p-8" style=move || if is_auth.get() { "" } else { "margin-left: 0; width: 100%" }>
                    <Show when=move || is_auth.get()>
                        <button
                            on:click=move |_| sidebar_open.update(|o| *o = !*o)
                            aria-label="Abrir menú de navegación"
                            aria-expanded=move || sidebar_open.get()
                            class="md:hidden fixed top-4 left-4 z-30 p-2 rounded-lg shadow-lg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500"
                            style="background:var(--uci-surface); border:1px solid var(--uci-border);"
                        >
                            <svg style="width:24px;height:24px;" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
                            </svg>
                        </button>
                    </Show>
                    <Routes fallback=|| view! { "Pagina no encontrada" }>
                        <Route path=path!("/login") view=LoginPage />

                        <Route path=path!("/") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <DashboardPage /> })
                            }
                        } />

                        <Route path=path!("/patients") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <PatientsPage /> })
                            }
                        } />

                        <Route path=path!("/patients/new") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else if !user_has("patients:create") {
                                Either::Left(view! { <Redirect path="/patients"/> })
                            } else {
                                Either::Right(view! { <RegisterPage /> })
                            }
                        } />

                        <Route path=path!("/patients/:id") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <PatientDetailPage /> })
                            }
                        } />

                        <Route path=path!("/patients/:id/edit") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <PatientEditPage /> })
                            }
                        } />

                        <Route path=path!("/patients/:id/measure") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <MeasurementPage /> })
                            }
                        } />

                        <Route path=path!("/admin") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else if !is_admin() {
                                Either::Left(view! { <Redirect path="/"/> })
                            } else {
                                Either::Right(view! { <AdminPage /> })
                            }
                        } />

                        <Route path=path!("/perfil") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else {
                                Either::Right(view! { <PerfilPage /> })
                            }
                        } />

                        <Route path=path!("/admin/soporte") view=move || {
                            if !is_auth.get() {
                                Either::Left(view! { <Redirect path="/login"/> })
                            } else if !user_has("support:read") {
                                Either::Left(view! { <Redirect path="/"/> })
                            } else {
                                Either::Right(view! { <SupportConsole /> })
                            }
                        } />
                    </Routes>
                </main>
            </div>
            <crate::stores::ToastContainer />
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

    let is_active_exact = move |target: &str| path() == target;

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
        ) style="background:var(--uci-surface); backdrop-filter:blur(12px);">
            <div style="padding:20px 16px 20px; border-bottom:1px solid var(--uci-border); background:linear-gradient(180deg, var(--uci-surface) 0%, rgba(59,130,246,0.03) 100%);">
                <div style="display:flex; align-items:center; gap:12px; margin-bottom:8px;">
                    <div style="
                        width:44px; height:44px; border-radius:14px;
                        background:linear-gradient(135deg, #0EA5E9 0%, #2563EB 50%, #6366F1 100%);
                        display:flex; align-items:center; justify-content:center;
                        box-shadow: 0 4px 16px rgba(14, 165, 233, 0.35), inset 0 1px 1px rgba(255,255,255,0.2);
                        font-size:20px; font-weight:900; color:white; flex-shrink:0;
                        position:relative; overflow:hidden;
                    ">
                        <i class="fa-solid fa-heart-pulse" style="position:relative; z-index:1; color:white;"></i>
                        <div style="position:absolute; top:0; left:0; right:0; bottom:0; background:linear-gradient(45deg, transparent 30%, rgba(255,255,255,0.15) 50%, transparent 70%); transform:translateX(-100%); animation:shimmer 3s infinite;"></div>
                    </div>
                    <div class="hidden md:block">
                        <div style="font-weight:700; font-size:17px; color:var(--uci-text); letter-spacing:-0.3px;">
                            <span style="color:#0EA5E9;">UCI</span> <span style="font-weight:800; color:var(--uci-text);">DMART</span>
                        </div>
                        <div style="font-size:10px; color:var(--uci-muted); text-transform:uppercase; letter-spacing:1.5px; font-weight:600; margin-top:2px;">
                            <i class="fa-solid fa-plus-minus" style="font-size:8px; margin-right:4px;"></i>
                            Unidad de Cuidados Intensivos
                        </div>
                    </div>
                    <button
                        on:click=close_sidebar
                        aria-label="Cerrar menú de navegación"
                        class="md:hidden ml-auto p-2 hover:bg-uci-border/30 rounded-lg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500"
                    >
                        <svg style="width:20px;height:20px;" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>
                <style>
                    {".@keyframes shimmer { 0% { transform:translateX(-100%); } 100% { transform:translateX(100%); } }"}
                </style>
            </div>

            <div style="padding:16px 12px; flex:1; overflow-y:auto;">
                <div style="font-size:10px; color:#64748B; text-transform:uppercase; letter-spacing:1.2px; padding:8px 10px 8px; font-weight:700;">
                    <i class="fa-solid fa-layer-group" style="margin-right:6px; font-size:8px;"></i>PRINCIPAL
                </div>

                <A href="/" attr:class=move || format!("nav-link {}", if is_active_exact("/") { "active" } else { "" })>
                    <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #10B981 0%, #059669 100%);">
                        <i class="fa-solid fa-house w-6 text-center text-lg" style="color:white;"></i>
                    </div>
                    <span style="font-weight:500;">Dashboard</span>
                </A>

                <A href="/patients" attr:class=move || format!("nav-link {}", if is_active_exact("/patients") { "active" } else { "" })>
                    <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #3B82F6 0%, #2563EB 100%);">
                        <i class="fa-solid fa-users w-6 text-center text-lg" style="color:white;"></i>
                    </div>
                    <span style="font-weight:500;">Pacientes</span>
                </A>

                <div style="font-size:10px; color:#64748B; text-transform:uppercase; letter-spacing:1.2px; padding:20px 10px 8px; font-weight:700;">
                    <i class="fa-solid fa-wand-magic-sparkles" style="margin-right:6px; font-size:8px;"></i>ACCIONES
                </div>

                <Show when=move || user_has("patients:create")>
                    <A href="/patients/new" attr:class=move || format!("nav-link {}", if is_active_exact("/patients/new") { "active" } else { "" })>
                        <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #F59E0B 0%, #D97706 100%);">
                            <i class="fa-solid fa-user-plus w-6 text-center text-lg" style="color:white;"></i>
                        </div>
                        <span style="font-weight:500;">Nuevo Paciente</span>
                    </A>
                </Show>

                <Show when=move || is_admin()>
                    <A href="/admin" attr:class=move || format!("nav-link {}", if is_active("/admin") && !is_active("/admin/soporte") { "active" } else { "" })>
                        <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #6366F1 0%, #4F46E5 100%);">
                            <i class="fa-solid fa-gears w-6 text-center text-lg" style="color:white;"></i>
                        </div>
                        <span style="font-weight:500;">Administración</span>
                    </A>
                </Show>

                <Show when=move || user_has("support:read")>
                    <A href="/admin/soporte" attr:class=move || format!("nav-link {}", if is_active("/admin/soporte") { "active" } else { "" })>
                        <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #F59E0B 0%, #D97706 100%);">
                            <i class="fa-solid fa-screwdriver-wrench w-6 text-center text-lg" style="color:white;"></i>
                        </div>
                        <span style="font-weight:500;">Soporte Técnico</span>
                    </A>
                </Show>

                <A href="/perfil" attr:class=move || format!("nav-link {}", if is_active_exact("/perfil") { "active" } else { "" })>
                    <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #14B8A6 0%, #0D9488 100%);">
                        <i class="fa-solid fa-user-gear w-6 text-center text-lg" style="color:white;"></i>
                    </div>
                    <span style="font-weight:500;">Mi Perfil</span>
                </A>

                {move || active_patient_id().map(|pid| {
                    let base = format!("/patients/{}", pid);
                    view! {
                        <div class="mt-4 pt-4 border-t border-uci-border sidebar-quick-fade" style="background:linear-gradient(180deg, rgba(236,72,153,0.06) 0%, transparent 100%); margin:12px; border-radius:12px; padding:12px;">
                            <div style="font-size:10px; color:#EC4899; text-transform:uppercase; letter-spacing:1px; padding:4px 8px 8px; font-weight:900;">
                                <i class="fa-solid fa-id-card-clip" style="margin-right:5px;"></i>PACIENTE ACTIVO
                            </div>

                            <A href=base.clone() attr:class="nav-link">
                                <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #EC4899 0%, #DB2777 100%);">
                                    <i class="fa-solid fa-user-injured w-6 text-center text-lg" style="color:white;"></i>
                                </div>
                                <span style="font-weight:500;">Expediente</span>
                            </A>

                            <A href=format!("{}/measure", base) attr:class="nav-link">
                                <div class="nav-icon-wrapper" style="background: linear-gradient(135deg, #06B6D4 0%, #0891B2 100%);">
                                    <i class="fa-solid fa-calculator w-6 text-center text-lg" style="color:white;"></i>
                                </div>
                                <span style="font-weight:500;">Medir Escalas</span>
                            </A>
                        </div>
                    }
                })}
            </div>

            <div style="padding:16px; border-top:1px solid var(--uci-border);">
                <Show when=move || crate::stores::current_user().is_some() fallback=|| ()>
                    <div style="margin-bottom:12px; padding:10px 12px; border-radius:10px; background:rgba(14,165,233,0.06); border:1px solid var(--uci-border); display:flex; align-items:center; gap:10px;">
                        <div style="width:34px; height:34px; border-radius:50%; background:linear-gradient(135deg,#0EA5E9,#6366F1); display:flex; align-items:center; justify-content:center; color:white; font-weight:700; flex-shrink:0;">
                            {move || {
                                let u = crate::stores::current_user();
                                u.map(|x| x.nombre.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or("?".into())).unwrap_or_default()
                            }}
                        </div>
                        <div style="min-width:0; flex:1;">
                            <div style="font-size:13px; font-weight:700; color:var(--uci-text); white-space:nowrap; overflow:hidden; text-overflow:ellipsis;">
                                {move || crate::stores::current_user().map(|x| x.nombre).unwrap_or_default()}
                            </div>
                            <div style="font-size:10px; color:var(--uci-muted); text-transform:uppercase; letter-spacing:0.5px;">
                                {move || crate::stores::current_user().map(|x| x.rol.label()).unwrap_or_default()}
                            </div>
                        </div>
                    </div>
                </Show>
                <div style="margin-bottom:12px;">
                    <ThemeToggle />
                </div>
                <button
                    on:click=move |_| {
                        let set_auth = use_context::<WriteSignal<bool>>();
                        spawn_local(async move {
                            let _ = crate::api::logout().await;
                            clear_session();
                            if let Some(setter) = set_auth {
                                setter.set(false);
                            }
                            window().location().reload().unwrap_or_default();
                        });
                    }
                    aria-label="Cerrar sesión"
                    style="
                        width:100%; padding:10px 16px;
                        background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3);
                        border-radius:10px; color:var(--uci-critical); font-size:13px; font-weight:600;
                        cursor:pointer; display:flex; align-items:center; justify-content:center; gap:8px;
                        transition:all 0.2s;
                    "
                >
                    <svg style="width:16px;height:16px;" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
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
