use crate::api;
use dmart_shared::models::*;
use leptos::prelude::*;

#[component]
pub fn AdminPage() -> impl IntoView {
    let (active_tab, set_active_tab) = signal("camas".to_string());
    let (_camas, set_camas) = signal::<Vec<Cama>>(vec![]);
    let (_equipos, set_equipos) = signal::<Vec<Equipo>>(vec![]);
    let (_staff, set_staff) = signal::<Vec<User>>(vec![]);
    let (_stats, set_stats) = signal::<AdminStats>(AdminStats::default());

    let fetch_camas = LocalResource::new(move || async move {
        api::get::<Vec<Cama>>("/api/admin/camas").await.ok()
    });
    let fetch_equipos = LocalResource::new(move || async move {
        api::get::<Vec<Equipo>>("/api/admin/equipos").await.ok()
    });
    let fetch_staff = LocalResource::new(move || async move {
        api::get::<Vec<User>>("/api/admin/staff").await.ok()
    });
    let fetch_stats = LocalResource::new(move || async move {
        api::get::<AdminStats>("/api/admin/stats").await.ok()
    });

    Effect::new(move |_| {
        if let Some(v) = fetch_camas.get() {
            set_camas.set(v.unwrap_or_default());
        }
    });
    Effect::new(move |_| {
        if let Some(v) = fetch_equipos.get() {
            set_equipos.set(v.unwrap_or_default());
        }
    });
    Effect::new(move |_| {
        if let Some(v) = fetch_staff.get() {
            set_staff.set(v.unwrap_or_default());
        }
    });
    Effect::new(move |_| {
        if let Some(v) = fetch_stats.get() {
            set_stats.set(v.unwrap_or_default());
        }
    });

    let tab_class = |tab: &str| {
        format!(
            "px-4 py-2 rounded-lg font-medium transition-all {}",
            if active_tab.get() == tab {
                "bg-uci-accent text-white".to_string()
            } else {
                "text-uci-muted hover:text-uci-text hover:bg-gray-100 dark:hover:bg-gray-800".to_string()
            }
        )
    };

    let current_tab = move || active_tab.get();

    view! {
        <div class="min-h-screen bg-gray-50 dark:bg-gray-900 p-6">
            <div class="max-w-7xl mx-auto">
                <div class="mb-8">
                    <h1 class="text-2xl font-bold text-uci-text dark:text-white">
                        <i class="fa-solid fa-gear mr-2 text-uci-accent"></i>
                        "Panel de Administración"
                    </h1>
                    <p class="text-uci-muted mt-1">"Gestión de UCI: Camas, Equipos y Personal"</p>
                </div>

                <div class="tabs flex gap-2 mb-6 border-b border-gray-200 dark:border-gray-700 pb-4">
                    <button class=tab_class("camas") on:click=move |_| set_active_tab.set("camas".to_string())>
                        <i class="fa-solid fa-bed mr-2"></i>"Camas"
                    </button>
                    <button class=tab_class("equipos") on:click=move |_| set_active_tab.set("equipos".to_string())>
                        <i class="fa-solid fa-monitor-heart mr-2"></i>"Equipos"
                    </button>
                    <button class=tab_class("staff") on:click=move |_| set_active_tab.set("staff".to_string())>
                        <i class="fa-solid fa-users mr-2"></i>"Personal"
                    </button>
                </div>

                <Show when=move || current_tab() == "camas">
                    <CamasPanel/>
                </Show>
                <Show when=move || current_tab() == "equipos">
                    <EquiposPanel/>
                </Show>
                <Show when=move || current_tab() == "staff">
                    <StaffPanel/>
                </Show>
            </div>
        </div>
    }
}

#[component]
fn CamasPanel() -> impl IntoView {
    let (camas, set_camas) = signal::<Vec<Cama>>(vec![]);
    let fetch_camas = LocalResource::new(move || async move {
        api::get::<Vec<Cama>>("/api/admin/camas").await.ok()
    });
    Effect::new(move |_| {
        if let Some(v) = fetch_camas.get() {
            set_camas.set(v.unwrap_or_default());
        }
    });

    view! {
        <div>
            <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-emerald-600">{camas.get().iter().filter(|c| c.estado.label() == "Libre").count()}</div>
                    <div class="text-sm text-uci-muted">"Camas Libres"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-rose-600">{camas.get().iter().filter(|c| c.estado.label() == "Ocupada").count()}</div>
                    <div class="text-sm text-uci-muted">"Camas Ocupadas"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-amber-600">{camas.get().iter().filter(|c| c.estado.label() == "Mantenimiento").count()}</div>
                    <div class="text-sm text-uci-muted">"En Mantenimiento"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-gray-600">{camas.get().len()}</div>
                    <div class="text-sm text-uci-muted">"Total Camas"</div>
                </div>
            </div>

            <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
                {camas.get().iter().map(|c| {
                    let bg_class = match c.estado.label() {
                        "Libre" => "bg-emerald-50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-800",
                        "Ocupada" => "bg-rose-50 dark:bg-rose-900/20 border-rose-200 dark:border-rose-800",
                        "Mantenimiento" => "bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800",
                        "Limpieza" => "bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800",
                        _ => "bg-gray-50 dark:bg-gray-800",
                    };
                    let estado_label = c.estado.label().to_string();
                    let paciente = c.paciente_nombre.clone();
                    view! {
                        <div class=format!("rounded-xl p-4 border-2 {}", bg_class)>
                            <div class="text-lg font-bold text-center">{format!("Cama {}", c.numero)}</div>
                            <div class="text-sm text-center mt-1 text-uci-primary">{estado_label}</div>
                            {paciente.clone().map(|p| view! {
                                <div class="text-xs text-center mt-2 text-uci-muted truncate">{p}</div>
                            })}
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn EquiposPanel() -> impl IntoView {
    let (equipos, set_equipos) = signal::<Vec<Equipo>>(vec![]);
    let fetch_equipos = LocalResource::new(move || async move {
        api::get::<Vec<Equipo>>("/api/admin/equipos").await.ok()
    });
    Effect::new(move |_| {
        if let Some(v) = fetch_equipos.get() {
            set_equipos.set(v.unwrap_or_default());
        }
    });

    view! {
        <div>
            <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-emerald-600">{equipos.get().iter().filter(|e| e.estado.label() == "Activo").count()}</div>
                    <div class="text-sm text-uci-muted">"Equipos Activos"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-amber-600">{equipos.get().iter().filter(|e| e.estado.label() == "En Mantenimiento").count()}</div>
                    <div class="text-sm text-uci-muted">"En Mantenimiento"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-gray-600">{equipos.get().iter().filter(|e| e.tipo.label() == "Ventilador Mecánico").count()}</div>
                    <div class="text-sm text-uci-muted">"Ventiladores"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-gray-600">{equipos.get().len()}</div>
                    <div class="text-sm text-uci-muted">"Total Equipos"</div>
                </div>
            </div>

            <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm overflow-hidden">
                <table class="w-full">
                    <thead class="bg-gray-50 dark:bg-gray-700">
                        <tr>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Nombre"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Tipo"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Marca/Modelo"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Serial"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Estado"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-gray-100 dark:divide-gray-700">
                        {equipos.get().iter().map(|e| {
                            let nombre = e.nombre.clone();
                            let tipo_label = e.tipo.label().to_string();
                            let marca = e.marca.clone();
                            let modelo = e.modelo.clone();
                            let serial = e.serial.clone();
                            let estado_label = e.estado.label().to_string();
                            let estado_class = match e.estado.label() {
                                "Activo" => "bg-emerald-100 text-emerald-700",
                                "En Mantenimiento" => "bg-amber-100 text-amber-700",
                                "Inactivo" => "bg-gray-100 text-gray-700",
                                _ => "bg-red-100 text-red-700",
                            };
                            view! {
                                <tr class="hover:bg-gray-50 dark:hover:bg-gray-700">
                                    <td class="px-4 py-3 text-sm text-uci-text dark:text-white font-medium">{nombre}</td>
                                    <td class="px-4 py-3 text-sm text-uci-text dark:text-white">{tipo_label}</td>
                                    <td class="px-4 py-3 text-sm text-uci-muted">{format!("{} {}", marca, modelo)}</td>
                                    <td class="px-4 py-3 text-sm text-uci-muted font-mono">{serial}</td>
                                    <td class="px-4 py-3">
                                        <span class=format!("px-2 py-1 rounded-full text-xs font-medium {}", estado_class)>{estado_label}</span>
                                    </td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

#[component]
fn StaffPanel() -> impl IntoView {
    let (staff, set_staff) = signal::<Vec<User>>(vec![]);
    let fetch_staff = LocalResource::new(move || async move {
        api::get::<Vec<User>>("/api/admin/staff").await.ok()
    });
    Effect::new(move |_| {
        if let Some(v) = fetch_staff.get() {
            set_staff.set(v.unwrap_or_default());
        }
    });

    view! {
        <div>
            <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-blue-600">{staff.get().iter().filter(|u| u.rol.label() == "Medico" && u.activo).count()}</div>
                    <div class="text-sm text-uci-muted">"Médicos Activos"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-pink-600">{staff.get().iter().filter(|u| u.rol.label() == "Enfermero" && u.activo).count()}</div>
                    <div class="text-sm text-uci-muted">"Enfermeros Activos"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-gray-600">{staff.get().iter().filter(|u| !u.activo).count()}</div>
                    <div class="text-sm text-uci-muted">"Inactivos"</div>
                </div>
                <div class="bg-white dark:bg-gray-800 rounded-xl p-4 shadow-sm">
                    <div class="text-2xl font-bold text-gray-600">{staff.get().len()}</div>
                    <div class="text-sm text-uci-muted">"Total Personal"</div>
                </div>
            </div>

            <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm overflow-hidden">
                <table class="w-full">
                    <thead class="bg-gray-50 dark:bg-gray-700">
                        <tr>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Nombre"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Usuario"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Rol"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium text-uci-muted">"Estado"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-gray-100 dark:divide-gray-700">
                        {staff.get().iter().map(|u| {
                            let nombre = u.nombre.clone();
                            let username = u.username.clone();
                            let rol_label = u.rol.label().to_string();
                            let rol_class = match u.rol.label() {
                                "Admin" => "bg-purple-100 text-purple-700",
                                "Medico" => "bg-blue-100 text-blue-700",
                                "Enfermero" => "bg-pink-100 text-pink-700",
                                _ => "bg-gray-100 text-gray-700",
                            };
                            let activo = u.activo;
                            view! {
                                <tr class="hover:bg-gray-50 dark:hover:bg-gray-700">
                                    <td class="px-4 py-3 text-sm text-uci-text dark:text-white font-medium">{nombre}</td>
                                    <td class="px-4 py-3 text-sm text-uci-muted font-mono">{username}</td>
                                    <td class="px-4 py-3">
                                        <span class=format!("px-2 py-1 rounded-full text-xs font-medium {}", rol_class)>{rol_label}</span>
                                    </td>
                                    <td class="px-4 py-3">
                                        <span class=format!("px-2 py-1 rounded-full text-xs font-medium {}",
                                            if activo { "bg-emerald-100 text-emerald-700" } else { "bg-gray-100 text-gray-700" }
                                        )>{if activo { "Activo" } else { "Inactivo" }}</span>
                                    </td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}