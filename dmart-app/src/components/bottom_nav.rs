//! Barra de navegación inferior para móvil y su drawer "Más".
//!
//! Por debajo de `md` el sidebar de escritorio no cabe: `input.css` lo oculta
//! y esta barra lo sustituye. Los destinos que no caben en los cuatro botones
//! principales van al drawer "Más", junto con las acciones que en escritorio
//! vivían en el pie del sidebar (nuevo paciente, contexto del paciente activo,
//! tema, idioma y logout). Sin eso, en móvil no había forma de crear un
//! paciente, cambiar el idioma ni cerrar sesión.

use crate::i18n::tr;
use crate::stores::{current_user, is_admin, logout, user_has};
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

/// Visibilidad de un destino según el rol con el que se ha iniciado sesión.
#[derive(Clone, Copy)]
enum Gate {
    /// Visible para cualquiera con sesión.
    Always,
    /// Visible si el usuario tiene este permiso RBAC.
    Perm(&'static str),
    /// Visible solo para administradores (`users:read`).
    Admin,
}

/// Destino de navegación: ruta, clave de i18n, icono de Font Awesome y gating.
#[derive(Clone, Copy)]
struct Dest {
    href: &'static str,
    key: &'static str,
    icon: &'static str,
    gate: Gate,
}

impl Dest {
    /// ¿Le toca a este usuario ver el destino?
    ///
    /// Los permisos son los mismos que aplica el sidebar: la barra no puede
    /// ofrecerle a un rol una sección que allí no ve.
    fn visible(&self) -> bool {
        match self.gate {
            Gate::Always => true,
            Gate::Perm(p) => user_has(p),
            Gate::Admin => is_admin(),
        }
    }
}

/// Los cuatro destinos de la barra inferior.
const PRIMARY: &[Dest] = &[
    Dest {
        href: "/",
        key: "nav-dashboard",
        icon: "fa-house",
        gate: Gate::Always,
    },
    Dest {
        href: "/patients",
        key: "nav-patients",
        icon: "fa-users",
        gate: Gate::Always,
    },
    Dest {
        href: "/escalation",
        key: "nav-escalation",
        icon: "fa-bell",
        gate: Gate::Perm("escalation:read"),
    },
    Dest {
        href: "/perfil",
        key: "nav-profile",
        icon: "fa-user-gear",
        gate: Gate::Always,
    },
];

/// Lo que no cabe en la barra, más el alta de paciente que en escritorio está
/// en la sección de acciones del sidebar.
const SECONDARY: &[Dest] = &[
    Dest {
        href: "/patients/new",
        key: "nav-patients-new",
        icon: "fa-user-plus",
        gate: Gate::Perm("patients:create"),
    },
    Dest {
        href: "/cds",
        key: "nav-cds",
        icon: "fa-clipboard-list",
        gate: Gate::Perm("patients:read"),
    },
    Dest {
        href: "/devices",
        key: "nav-devices",
        icon: "fa-microchip",
        gate: Gate::Perm("devices:read"),
    },
    Dest {
        href: "/data-quality",
        key: "nav-quality",
        icon: "fa-shield-heart",
        gate: Gate::Perm("quality:read"),
    },
    Dest {
        href: "/admin/soporte",
        key: "nav-support",
        icon: "fa-screwdriver-wrench",
        gate: Gate::Perm("support:read"),
    },
    Dest {
        href: "/admin/tenants",
        key: "nav-tenants",
        icon: "fa-building",
        gate: Gate::Perm("tenants:read"),
    },
    Dest {
        href: "/admin/audit",
        key: "nav-audit",
        icon: "fa-clipboard-check",
        gate: Gate::Perm("audit:read"),
    },
    Dest {
        href: "/admin",
        key: "nav-admin",
        icon: "fa-gears",
        gate: Gate::Admin,
    },
];

/// Atajo contextual del paciente abierto: sufijo de la ruta, clave de i18n e
/// icono. Los mismos tres enlaces que el bloque "paciente activo" del sidebar.
const PATIENT_LINKS: &[(&str, &str, &str)] = &[
    ("", "nav-record", "fa-id-card-clip"),
    ("/measure", "nav-measure", "fa-calculator"),
    ("/timeline", "nav-timeline", "fa-clock-rotate-left"),
];

/// ¿La ruta actual cae dentro del destino? Todo cuelgue cuenta como activo
/// (una ficha de paciente mantiene "Pacientes" encendido); la raíz, solo exacta.
fn path_hits(path: &str, target: &str) -> bool {
    if target == "/" {
        path == "/"
    } else {
        path.starts_with(target)
    }
}

/// Identificador del paciente que se está viendo, si la ruta es su ficha.
fn active_patient_id(path: &str) -> Option<String> {
    if path.starts_with("/patients/") && !path.starts_with("/patients/new") {
        path.split('/').nth(2).map(str::to_string)
    } else {
        None
    }
}

/// Un destino de navegación, marcado activo según la ruta actual.
///
/// Todo lo capturado por las reactividades del markup es `'static` (o clonado
/// como señal), así que los destinos salen de las constantes de arriba.
fn nav_item(
    d: &'static Dest,
    path: Memo<String>,
    item_class: &'static str,
    icon_class: &'static str,
) -> impl IntoView {
    let href = d.href;
    view! {
        <A
            href=href
            attr:class=move || {
                if path_hits(&path.get(), href) {
                    format!("{item_class} active")
                } else {
                    item_class.to_string()
                }
            }
        >
            <span class=icon_class aria-hidden="true">
                <i class=format!("fa-solid {}", d.icon)></i>
            </span>
            <span>{move || tr(d.key, None)}</span>
        </A>
    }
}

/// Barra inferior (móvil) + drawer "Más" con el resto de destinos y las
/// acciones de sesión. Todo el markup depende de las clases de `input.css`.
#[component]
pub fn BottomNavBar() -> impl IntoView {
    let location = use_location();
    let path = location.pathname;
    let more_open = RwSignal::new(false);

    // El drawer se cierra solo al cambiar de ruta, no solo al pulsar sus
    // enlaces: así también lo cierra el botón "atrás" del navegador.
    Effect::new(move |_| {
        let _ = location.pathname.get();
        more_open.set(false);
    });

    // Bloquea el scroll del fondo mientras el drawer está abierto. La clase la
    // neutraliza `input.css` a partir de `md`, donde el drawer ya no existe, por
    // si se gira el móvil con el drawer abierto.
    Effect::new(move |_| {
        let open = more_open.get();
        if let Some(window) = web_sys::window()
            && let Some(doc) = window.document()
            && let Some(html) = doc.document_element()
        {
            let classes = html.class_list();
            let _ = if open {
                classes.add_1("nav-sheet-open")
            } else {
                classes.remove_1("nav-sheet-open")
            };
        }
    });

    view! {
        <>
            <nav class="bottom-nav" aria-label=move || tr("aria-main-nav", None)>
                {PRIMARY
                    .iter()
                    .filter(|d| d.visible())
                    .map(|d| nav_item(d, path, "bottom-nav-item", "bottom-nav-icon"))
                    .collect_view()}

                <button
                    type="button"
                    class="bottom-nav-item"
                    class:active=move || more_open.get()
                    aria-expanded=move || more_open.get()
                    aria-controls="more-sheet"
                    on:click=move |_| more_open.set(true)
                >
                    <span class="bottom-nav-icon" aria-hidden="true">
                        <i class="fa-solid fa-ellipsis"></i>
                    </span>
                    <span class="bottom-nav-label">{move || tr("nav-more", None)}</span>
                </button>
            </nav>

            <Show when=move || more_open.get()>
                <div
                    class="more-sheet-scrim"
                    on:click=move |_| more_open.set(false)
                    aria-hidden="true"
                ></div>
                <div
                    class="more-sheet"
                    id="more-sheet"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="more-sheet-title"
                >
                    <div class="more-sheet-grip" aria-hidden="true"></div>

                    <div class="more-sheet-head">
                        <span class="more-sheet-title" id="more-sheet-title">
                            {move || tr("nav-more", None)}
                        </span>
                        <button
                            type="button"
                            class="more-sheet-close"
                            aria-label=move || tr("aria-close", None)
                            on:click=move |_| more_open.set(false)
                        >
                            <i class="fa-solid fa-xmark" aria-hidden="true"></i>
                        </button>
                    </div>

                    // Quién ha iniciado sesión: en escritorio lo muestra el pie
                    // del sidebar, que en móvil no existe.
                    <Show when=move || current_user().is_some() fallback=|| ()>
                        <div class="more-sheet-user">
                            <span class="more-sheet-avatar" aria-hidden="true">
                                {move || {
                                    current_user()
                                        .and_then(|u| u.nombre.chars().next())
                                        .map(|c| c.to_uppercase().to_string())
                                        .unwrap_or_else(|| "?".to_string())
                                }}
                            </span>
                            <span class="more-sheet-user-meta">
                                <span class="more-sheet-user-name">
                                    {move || current_user().map(|u| u.nombre).unwrap_or_default()}
                                </span>
                                <span class="more-sheet-user-role">
                                    {move || current_user().map(|u| u.rol.label()).unwrap_or_default()}
                                </span>
                            </span>
                        </div>
                    </Show>

                    <nav class="more-sheet-list">
                        {SECONDARY
                            .iter()
                            .filter(|d| d.visible())
                            .map(|d| nav_item(d, path, "more-sheet-item", "more-sheet-icon"))
                            .collect_view()}

                        // Atajos de la ficha abierta: en escritorio cuelgan del
                        // sidebar; en móvil, de este drawer.
                        {move || active_patient_id(&path.get()).map(|pid| {
                            let base = format!("/patients/{pid}");
                            view! {
                                <div class="more-sheet-section">
                                    {move || tr("nav-patient-active", None)}
                                </div>
                                {PATIENT_LINKS.iter().map(|(suffix, key, icon)| {
                                    let href = format!("{base}{suffix}");
                                    let icon = (*icon).to_string();
                                    view! {
                                        <A href=href attr:class="more-sheet-item">
                                            <span class="more-sheet-icon" aria-hidden="true">
                                                <i class=format!("fa-solid {icon}")></i>
                                            </span>
                                            <span>{move || tr(key, None)}</span>
                                        </A>
                                    }
                                }).collect_view()}
                            }
                        })}
                    </nav>

                    <div class="more-sheet-foot">
                        <crate::theme::ThemeSelector />
                        <crate::theme::LangSelector />
                        <button
                            type="button"
                            class="more-sheet-logout"
                            on:click=move |_| logout()
                        >
                            <i class="fa-solid fa-arrow-right-from-bracket" aria-hidden="true"></i>
                            {move || tr("nav-logout", None)}
                        </button>
                    </div>
                </div>
            </Show>
        </>
    }
}
