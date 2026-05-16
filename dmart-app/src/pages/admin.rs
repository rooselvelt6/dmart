use crate::api;
use dmart_shared::models::*;
use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn AdminPage() -> impl IntoView {
    let (active_tab, set_active_tab) = signal("camas".to_string());

    let admin_stats = LocalResource::new(|| async move {
        api::get_admin_stats().await.ok()
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

    view! {
        <div class="min-h-screen p-6" style="background:var(--uci-bg);">
            <div class="max-w-7xl mx-auto">
                <div class="mb-8">
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-gear mr-2" style="color:var(--uci-accent);"></i>
                        "Panel de Administración"
                    </h1>
                    <p class="mt-1" style="color:var(--uci-muted);">"Gestión de UCI: Camas, Equipos y Personal"</p>
                </div>

                <Suspense fallback=move || view! {
                    <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-7 gap-3 mb-6">
                        {[1,2,3,4,5,6,7].iter().map(|_| view! {
                            <div class="p-4 rounded-xl animate-pulse" style="background:var(--uci-surface);">
                                <div class="h-4 w-16 rounded mb-2" style="background:var(--uci-border);"></div>
                                <div class="h-8 w-12 rounded" style="background:var(--uci-border);"></div>
                            </div>
                        }).collect_view()}
                    </div>
                }>
                    {move || admin_stats.get().map(|a| match a {
                        Some(stats) => Either::Left(view! {
                            <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-7 gap-3 mb-6">
                                {admin_stat_card("Camas", &stats.total_camas.to_string(), "#6366F1", "fa-bed")}
                                {admin_stat_card("Libres", &stats.camas_libres.to_string(), "#10B981", "fa-check")}
                                {admin_stat_card("Ocupadas", &stats.camas_ocupadas.to_string(), "#EF4444", "fa-xmark")}
                                {admin_stat_card("Equipos", &stats.total_equipos.to_string(), "#3B82F6", "fa-monitor-heart")}
                                {admin_stat_card("Disponibles", &stats.equipos_disponibles.to_string(), "#8B5CF6", "fa-box")}
                                {admin_stat_card("Medicos", &stats.medicos_activos.to_string(), "#F59E0B", "fa-user-doctor")}
                                {admin_stat_card("Enfermeros", &stats.enfermeros_activos.to_string(), "#EC4899", "fa-user-nurse")}
                            </div>
                        }),
                        None => Either::Right(view! {
                            <div class="p-4 mb-6 rounded-xl text-sm" style="background:rgba(239,68,68,0.1); color:#DC2626;">
                                <i class="fa-solid fa-triangle-exclamation mr-2"></i>"No se pudieron cargar las estadísticas del servidor"
                            </div>
                        }),
                    })}
                </Suspense>

                <div class="tabs flex gap-2 mb-6 pb-4" style="border-bottom:1px solid var(--uci-border);">
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

                <Show when=move || active_tab.get() == "camas">
                    <CamasPanel/>
                </Show>
                <Show when=move || active_tab.get() == "equipos">
                    <EquiposPanel/>
                </Show>
                <Show when=move || active_tab.get() == "staff">
                    <StaffPanel/>
                </Show>
            </div>
        </div>
    }
}

fn admin_stat_card(title: &str, value: &str, color: &str, icon: &str) -> impl IntoView {
    let title = title.to_string();
    let value = value.to_string();
    let c = color;
    view! {
        <div class="p-4 rounded-xl" style=format!("background:var(--uci-surface); border-top:2px solid {};", c)>
            <div class="flex items-center gap-2 mb-1">
                <i class=format!("fa-solid {} text-sm", icon) style=format!("color:{};", c)></i>
                <span class="text-[10px] uppercase font-bold" style="color:var(--uci-muted);">{title}</span>
            </div>
            <div class="text-2xl font-extrabold" style=format!("color:{}; font-family:'JetBrains Mono',monospace;", c)>{value}</div>
        </div>
    }
}

// ─── Shared ──────────────────────────────────────────────────────

fn parse_tipo_cama(s: &str) -> TipoCama {
    match s {
        "Aislamiento" => TipoCama::Aislamiento,
        "Pediátrica" => TipoCama::Pediatrica,
        "Coronaria" => TipoCama::Coronaria,
        "Quemados" => TipoCama::Quemados,
        "Otro" => TipoCama::Otro,
        _ => TipoCama::General,
    }
}

fn parse_estado_cama(s: &str) -> EstadoCama {
    match s {
        "Ocupada" => EstadoCama::Ocupada,
        "Mantenimiento" => EstadoCama::Mantenimiento,
        "Limpieza" => EstadoCama::Limpieza,
        _ => EstadoCama::Libre,
    }
}

fn parse_tipo_equipo(s: &str) -> TipoEquipo {
    match s {
        "Monitor" => TipoEquipo::Monitor,
        "Computador" => TipoEquipo::Computador,
        "BombaInfusion" | "Bomba de Infusión" => TipoEquipo::BombaInfusion,
        "VentiladorMecanico" | "Ventilador Mecánico" => TipoEquipo::VentiladorMecanico,
        _ => TipoEquipo::Otro,
    }
}

fn parse_estado_equipo(s: &str) -> EstadoEquipo {
    match s {
        "Mantenimiento" | "En Mantenimiento" => EstadoEquipo::Mantenimiento,
        "Inactivo" => EstadoEquipo::Inactivo,
        "Reparacion" | "En Reparación" => EstadoEquipo::Reparacion,
        _ => EstadoEquipo::Activo,
    }
}

fn parse_rol(s: &str) -> UserRole {
    match s {
        "Admin" => UserRole::Admin,
        "Medico" => UserRole::Medico,
        "Enfermero" => UserRole::Enfermero,
        _ => UserRole::Viewer,
    }
}

// ─── Camas Panel ─────────────────────────────────────────────────

#[component]
fn CamasPanel() -> impl IntoView {
    let (camas, set_camas) = signal::<Vec<Cama>>(vec![]);
    let (show_form, set_show_form) = signal(false);
    let (edit_cama, set_edit_cama) = signal::<Option<Cama>>(None);
    let (form_numero, set_form_numero) = signal(1u8);
    let (form_tipo, set_form_tipo) = signal("General".to_string());
    let (form_estado, set_form_estado) = signal("Libre".to_string());
    let saving = RwSignal::new(false);
    let error_msg = RwSignal::new(None::<String>);

    let fetch = move || {
        spawn_local(async move {
            if let Ok(c) = api::get::<Vec<Cama>>("/admin/camas").await {
                set_camas.set(c);
            }
        });
    };
    fetch();

    let reset_form = move || {
        set_edit_cama.set(None);
        set_form_numero.set(1u8);
        set_form_tipo.set("General".to_string());
        set_form_estado.set("Libre".to_string());
        set_show_form.set(false);
        error_msg.set(None);
    };

    let open_edit = move |c: Cama| {
        set_edit_cama.set(Some(c.clone()));
        set_form_numero.set(c.numero);
        set_form_tipo.set(c.tipo.label().to_string());
        set_form_estado.set(c.estado.label().to_string());
        set_show_form.set(true);
    };

    let save = move || {
        saving.set(true);
        error_msg.set(None);
        let numero = form_numero.get();
        let tipo = form_tipo.get();
        let estado = form_estado.get();
        let _es_editar = edit_cama.get().is_some();
        let cama_id = edit_cama.get().map(|c| c.cama_id.clone());

        spawn_local(async move {
            let result = if let Some(ref id) = cama_id {
                api::update_cama(id, numero, &tipo, &estado).await
            } else {
                api::create_cama(numero, &tipo, &estado).await
            };
            match result {
                Ok(_) => {
                    saving.set(false);
                    reset_form();
                    fetch();
                }
                Err(e) => {
                    saving.set(false);
                    error_msg.set(Some(e));
                }
            }
        });
    };

    let delete_cama = move |id: String| {
        spawn_local(async move {
            let _ = api::delete_cama(&id).await;
            fetch();
        });
    };

    view! {
        <div>
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-lg font-bold" style="color:var(--uci-text);">
                    <i class="fa-solid fa-bed mr-2" style="color:var(--uci-accent);"></i>"Gestión de Camas"
                </h2>
                <button on:click=move |_| { reset_form(); set_show_form.update(|v| *v = !*v); }
                    class="px-4 py-2 rounded-lg text-sm font-medium text-white"
                    style="background:var(--uci-accent);">
                    <i class="fa-solid fa-plus mr-1"></i>{move || if show_form.get() { "Cancelar" } else { "Nueva Cama" }}
                </button>
            </div>

            {move || error_msg.get().map(|e| view! {
                <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                    style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                    <i class="fa-solid fa-triangle-exclamation"></i>{e}
                </div>
            })}

            <Show when=move || show_form.get()>
                <div class="p-4 rounded-xl mb-6 flex flex-wrap items-end gap-4"
                    style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Número"</label>
                        <input type="number" min="1" class="form-input w-24"
                            prop:value=move || form_numero.get().to_string()
                            on:input=move |ev| { let v = event_target_value(&ev).parse().unwrap_or(1); set_form_numero.set(v); } />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Tipo"</label>
                        <select class="form-select"
                            prop:value=move || form_tipo.get()
                            on:change=move |ev| { set_form_tipo.set(event_target_value(&ev)); }>
                            <option value="General">"General"</option>
                            <option value="Aislamiento">"Aislamiento"</option>
                            <option value="Pediátrica">"Pediátrica"</option>
                            <option value="Coronaria">"Coronaria"</option>
                            <option value="Quemados">"Quemados"</option>
                            <option value="Otro">"Otro"</option>
                        </select>
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Estado"</label>
                        <select class="form-select"
                            prop:value=move || form_estado.get()
                            on:change=move |ev| { set_form_estado.set(event_target_value(&ev)); }>
                            <option value="Libre">"Libre"</option>
                            <option value="Ocupada">"Ocupada"</option>
                            <option value="Mantenimiento">"Mantenimiento"</option>
                            <option value="Limpieza">"Limpieza"</option>
                        </select>
                    </div>
                    <button on:click=move |_| save() class="btn-primary px-6 h-10 text-sm" disabled=saving>
                        {move || if saving.get() { "Guardando..." } else if edit_cama.get().is_some() { "Actualizar" } else { "Crear" }}
                    </button>
                </div>
            </Show>

            <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
                {move || camas.get().iter().map(|c| {
                    let bg = match c.estado {
                        EstadoCama::Libre => "bg-emerald-50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-800",
                        EstadoCama::Ocupada => "bg-rose-50 dark:bg-rose-900/20 border-rose-200 dark:border-rose-800",
                        EstadoCama::Mantenimiento => "bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800",
                        EstadoCama::Limpieza => "bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800",
                    };
                    let cama_clone = c.clone();
                    let cama_id = c.cama_id.clone();
                    let estado_label = c.estado.label().to_string();
                    let tipo_label = c.tipo.label().to_string();
                    let tipo_icon = c.tipo.icon().to_string();
                    let paciente = c.paciente_nombre.clone();

                    view! {
                        <div class=format!("rounded-xl p-4 border-2 {}", bg) style="position:relative;">
                            <div class="flex items-center justify-between mb-1">
                                <span class="text-xs font-bold uppercase tracking-wider" style="color:var(--uci-muted);">
                                    <i class=format!("fa-solid {} mr-1", tipo_icon)></i>{tipo_label}
                                </span>
                                <div class="flex gap-1">
                                    <button on:click=move |_| { let c = cama_clone.clone(); open_edit(c); }
                                        class="text-xs p-1 rounded hover:bg-black/10">
                                        <i class="fa-solid fa-pen"></i>
                                    </button>
                                    <button on:click=move |_| delete_cama(cama_id.clone())
                                        class="text-xs p-1 rounded hover:bg-black/10" style="color:#DC2626;">
                                        <i class="fa-solid fa-trash"></i>
                                    </button>
                                </div>
                            </div>
                            <div class="text-lg font-bold text-center">{format!("Cama {}", c.numero)}</div>
                            <div class="text-sm text-center mt-1 font-medium" style="color:var(--uci-accent);">{estado_label}</div>
                            {paciente.map(|p| view! {
                                <div class="text-xs text-center mt-2 truncate" style="color:var(--uci-muted);">{p}</div>
                            })}
                        </div>
                    }
                }).collect_view()}
            </div>

            <div class="mt-6">
                <h3 class="text-sm font-bold uppercase mb-3" style="color:var(--uci-muted);">
                    <i class="fa-solid fa-table mr-2"></i>"Registro de Camas"
                </h3>
                <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                    <table class="w-full">
                        <thead style="background:var(--uci-bg);">
                            <tr>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"#"</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Numero"</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Tipo"</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Estado"</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Paciente"</th>
                                <th class="px-4 py-3 text-right text-sm font-medium" style="color:var(--uci-muted);">"Acciones"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y" style="border-color:var(--uci-border);">
                            {move || camas.get().iter().enumerate().map(|(i, c)| {
                                let idx = i + 1;
                                let cama_clone = c.clone();
                                let cama_id = c.cama_id.clone();
                                let tipo_label = c.tipo.label().to_string();
                                let estado_label = c.estado.label().to_string();
                                let paciente = c.paciente_nombre.clone();
                                let estado_class = match c.estado {
                                    EstadoCama::Libre => "bg-emerald-100 text-emerald-700",
                                    EstadoCama::Ocupada => "bg-rose-100 text-rose-700",
                                    EstadoCama::Mantenimiento => "bg-amber-100 text-amber-700",
                                    EstadoCama::Limpieza => "bg-blue-100 text-blue-700",
                                };

                                view! {
                                    <tr class="hover:bg-black/5">
                                        <td class="px-4 py-3 text-sm font-mono" style="color:var(--uci-muted);">{idx.to_string()}</td>
                                        <td class="px-4 py-3 text-sm font-bold" style="color:var(--uci-text);">{format!("Cama {}", c.numero)}</td>
                                        <td class="px-4 py-3 text-sm" style="color:var(--uci-text);">{tipo_label}</td>
                                        <td class="px-4 py-3">
                                            <span class=format!("px-2 py-1 rounded-full text-xs font-medium {}", estado_class)>{estado_label}</span>
                                        </td>
                                        <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">
                                            {paciente.unwrap_or_else(|| "—".into())}
                                        </td>
                                        <td class="px-4 py-3 text-right">
                                            <button on:click=move |_| { let c = cama_clone.clone(); open_edit(c); }
                                                class="text-xs px-2 py-1 rounded hover:bg-black/10 mr-1">
                                                <i class="fa-solid fa-pen"></i>
                                            </button>
                                            <button on:click=move |_| delete_cama(cama_id.clone())
                                                class="text-xs px-2 py-1 rounded hover:bg-black/10" style="color:#DC2626;">
                                                <i class="fa-solid fa-trash"></i>
                                            </button>
                                        </td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

// ─── Equipos Panel ───────────────────────────────────────────────

#[component]
fn EquiposPanel() -> impl IntoView {
    let (equipos, set_equipos) = signal::<Vec<Equipo>>(vec![]);
    let (_camas, _set_camas) = signal::<Vec<Cama>>(vec![]);
    let (show_form, set_show_form) = signal(false);
    let (edit_equipo, set_edit_equipo) = signal::<Option<Equipo>>(None);
    let (form_nombre, set_form_nombre) = signal(String::new());
    let (form_tipo, set_form_tipo) = signal("VentiladorMecanico".to_string());
    let (form_marca, set_form_marca) = signal(String::new());
    let (form_modelo, set_form_modelo) = signal(String::new());
    let (form_serial, set_form_serial) = signal(String::new());
    let (form_estado, set_form_estado) = signal("Activo".to_string());
    let saving = RwSignal::new(false);
    let error_msg = RwSignal::new(None::<String>);

    let fetch = move || {
        spawn_local(async move {
            if let Ok(e) = api::get::<Vec<Equipo>>("/admin/equipos").await {
                set_equipos.set(e);
            }
        });
    };
    fetch();

    let reset_form = move || {
        set_edit_equipo.set(None);
        set_form_nombre.set(String::new());
        set_form_tipo.set("VentiladorMecanico".to_string());
        set_form_marca.set(String::new());
        set_form_modelo.set(String::new());
        set_form_serial.set(String::new());
        set_form_estado.set("Activo".to_string());
        set_show_form.set(false);
        error_msg.set(None);
    };

    let open_edit = move |e: Equipo| {
        set_edit_equipo.set(Some(e.clone()));
        set_form_nombre.set(e.nombre.clone());
        set_form_tipo.set(format!("{:?}", e.tipo));
        set_form_marca.set(e.marca.clone());
        set_form_modelo.set(e.modelo.clone());
        set_form_serial.set(e.serial.clone());
        set_form_estado.set(e.estado.label().to_string());
        set_show_form.set(true);
    };

    let save = move || {
        saving.set(true);
        error_msg.set(None);
        let nombre = form_nombre.get();
        let tipo_str = form_tipo.get();
        let _marca = form_marca.get();
        let modelo = form_modelo.get();
        let serial = form_serial.get();
        let estado_str = form_estado.get();

        let tipo_eq = parse_tipo_equipo(&tipo_str);
        let estado_eq = parse_estado_equipo(&estado_str);

        let _es_editar = edit_equipo.get().is_some();
        let equipo_id = edit_equipo.get().map(|e| e.equipo_id.clone());

        spawn_local(async move {
            let result = if let Some(ref id) = equipo_id {
                let body = api::UpdateEquipoRequest {
                    nombre: Some(nombre),
                    tipo: Some(tipo_str),
                    modelo: Some(modelo),
                    serie: Some(serial),
                    estado: Some(estado_str),
                };
                api::update_equipo(id, body).await.map(|_| ())
            } else {
                let body = api::CreateEquipoRequest {
                    nombre,
                    tipo: format!("{:?}", tipo_eq),
                    modelo,
                    serie: serial,
                    estado: estado_eq.label().to_string(),
                };
                api::create_equipo(body).await.map(|_| ())
            };
            match result {
                Ok(_) => { saving.set(false); reset_form(); fetch(); }
                Err(e) => { saving.set(false); error_msg.set(Some(e)); }
            }
        });
    };

    let delete_eq = move |id: String| {
        spawn_local(async move {
            let _ = api::delete_equipo(&id).await;
            fetch();
        });
    };

    view! {
        <div>
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-lg font-bold" style="color:var(--uci-text);">
                    <i class="fa-solid fa-monitor-heart mr-2" style="color:var(--uci-accent);"></i>"Gestión de Equipos"
                </h2>
                <button on:click=move |_| { reset_form(); set_show_form.update(|v| *v = !*v); }
                    class="px-4 py-2 rounded-lg text-sm font-medium text-white"
                    style="background:var(--uci-accent);">
                    <i class="fa-solid fa-plus mr-1"></i>{move || if show_form.get() { "Cancelar" } else { "Nuevo Equipo" }}
                </button>
            </div>

            {move || error_msg.get().map(|e| view! {
                <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                    style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                    <i class="fa-solid fa-triangle-exclamation"></i>{e}
                </div>
            })}

            <Show when=move || show_form.get()>
                <div class="p-4 rounded-xl mb-6" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                    <div class="grid grid-cols-2 md:grid-cols-5 gap-4">
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Nombre"</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_nombre.get()
                                on:input=move |ev| set_form_nombre.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Tipo"</label>
                            <select class="form-select"
                                prop:value=move || form_tipo.get()
                                on:change=move |ev| set_form_tipo.set(event_target_value(&ev))>
                                <option value="VentiladorMecanico">"Ventilador Mecánico"</option>
                                <option value="Monitor">"Monitor"</option>
                                <option value="Computador">"Computador"</option>
                                <option value="BombaInfusion">"Bomba de Infusión"</option>
                                <option value="Otro">"Otro"</option>
                            </select>
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Marca"</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_marca.get()
                                on:input=move |ev| set_form_marca.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Modelo"</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_modelo.get()
                                on:input=move |ev| set_form_modelo.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Serial"</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_serial.get()
                                on:input=move |ev| set_form_serial.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="flex items-end gap-4 mt-4">
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Estado"</label>
                            <select class="form-select"
                                prop:value=move || form_estado.get()
                                on:change=move |ev| set_form_estado.set(event_target_value(&ev))>
                                <option value="Activo">"Activo"</option>
                                <option value="Mantenimiento">"Mantenimiento"</option>
                                <option value="Inactivo">"Inactivo"</option>
                                <option value="Reparacion">"Reparación"</option>
                            </select>
                        </div>
                        <button on:click=move |_| save() class="btn-primary px-6 h-10 text-sm" disabled=saving>
                            {move || if saving.get() { "Guardando..." } else if edit_equipo.get().is_some() { "Actualizar" } else { "Crear" }}
                        </button>
                    </div>
                </div>
            </Show>

            <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <table class="w-full">
                    <thead style="background:var(--uci-bg);">
                        <tr>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Nombre"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Tipo"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Marca/Modelo"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Serial"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Estado"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Cama"</th>
                            <th class="px-4 py-3 text-right text-sm font-medium" style="color:var(--uci-muted);">"Acciones"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y" style="border-color:var(--uci-border);">
                        {move || equipos.get().iter().map(|e| {
                            let eq = e.clone();
                            let eq_id = e.equipo_id.clone();
                            let nombre = e.nombre.clone();
                            let tipo_label = e.tipo.label().to_string();
                            let marca = e.marca.clone();
                            let modelo = e.modelo.clone();
                            let serial = e.serial.clone();
                            let estado_label = e.estado.label().to_string();
                            let estado_class = match e.estado {
                                EstadoEquipo::Activo => "bg-emerald-100 text-emerald-700",
                                EstadoEquipo::Mantenimiento => "bg-amber-100 text-amber-700",
                                EstadoEquipo::Inactivo => "bg-gray-100 text-gray-700",
                                EstadoEquipo::Reparacion => "bg-red-100 text-red-700",
                            };
                            let cama_asignada = e.cama_id.clone().unwrap_or_default();

                            view! {
                                <tr class="hover:bg-black/5">
                                    <td class="px-4 py-3 text-sm font-medium" style="color:var(--uci-text);">{nombre}</td>
                                    <td class="px-4 py-3 text-sm" style="color:var(--uci-text);">{tipo_label}</td>
                                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{format!("{} {}", marca, modelo)}</td>
                                    <td class="px-4 py-3 text-sm font-mono" style="color:var(--uci-muted);">{serial}</td>
                                    <td class="px-4 py-3">
                                        <span class=format!("px-2 py-1 rounded-full text-xs font-medium {}", estado_class)>{estado_label}</span>
                                    </td>
                                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">
                                        {if cama_asignada.is_empty() { "—".into() } else { format!("#{}", &cama_asignada[..8]) }}
                                    </td>
                                    <td class="px-4 py-3 text-right">
                                        <button on:click=move |_| { let e = eq.clone(); open_edit(e); }
                                            class="text-xs px-2 py-1 rounded hover:bg-black/10 mr-1">
                                            <i class="fa-solid fa-pen"></i>
                                        </button>
                                        <button on:click=move |_| delete_eq(eq_id.clone())
                                            class="text-xs px-2 py-1 rounded hover:bg-black/10" style="color:#DC2626;">
                                            <i class="fa-solid fa-trash"></i>
                                        </button>
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

// ─── Staff Panel ─────────────────────────────────────────────────

#[component]
fn StaffPanel() -> impl IntoView {
    let (staff, set_staff) = signal::<Vec<User>>(vec![]);
    let (show_form, set_show_form) = signal(false);
    let (edit_user, set_edit_user) = signal::<Option<User>>(None);
    let (form_nombre, set_form_nombre) = signal(String::new());
    let (form_username, set_form_username) = signal(String::new());
    let (form_password, set_form_password) = signal(String::new());
    let (form_rol, set_form_rol) = signal("Medico".to_string());
    let saving = RwSignal::new(false);
    let error_msg = RwSignal::new(None::<String>);

    let fetch = move || {
        spawn_local(async move {
            if let Ok(s) = api::list_staff().await {
                set_staff.set(s);
            }
        });
    };
    fetch();

    let reset_form = move || {
        set_edit_user.set(None);
        set_form_nombre.set(String::new());
        set_form_username.set(String::new());
        set_form_password.set(String::new());
        set_form_rol.set("Medico".to_string());
        set_show_form.set(false);
        error_msg.set(None);
    };

    let open_edit = move |u: User| {
        set_edit_user.set(Some(u.clone()));
        set_form_nombre.set(u.nombre.clone());
        set_form_username.set(u.username.clone());
        set_form_password.set(String::new());
        set_form_rol.set(u.rol.to_string());
        set_show_form.set(true);
    };

    let save = move || {
        saving.set(true);
        error_msg.set(None);
        let nombre = form_nombre.get();
        let username = form_username.get();
        let password = form_password.get();
        let rol_str = form_rol.get();
        let _es_editar = edit_user.get().is_some();
        let user_id = edit_user.get().map(|u| u.user_id.clone());

        spawn_local(async move {
            let user = User {
                nombre,
                username,
                password_hash: password,
                rol: parse_rol(&rol_str),
                activo: true,
                ..User::default()
            };
            let result = if let Some(ref id) = user_id {
                let mut u = edit_user.get().unwrap();
                u.nombre = user.nombre.clone();
                u.username = user.username.clone();
                u.rol = user.rol.clone();
                if !user.password_hash.is_empty() {
                    u.password_hash = user.password_hash.clone();
                }
                api::update_staff(id, &u).await.map(|_| ())
            } else {
                api::create_staff(&user).await.map(|_| ())
            };
            match result {
                Ok(_) => { saving.set(false); reset_form(); fetch(); }
                Err(e) => { saving.set(false); error_msg.set(Some(e)); }
            }
        });
    };

    let delete_st = move |id: String| {
        spawn_local(async move {
            let _ = api::delete_staff(&id).await;
            fetch();
        });
    };

    let toggle_st = move |id: String| {
        spawn_local(async move {
            let _ = api::toggle_staff(&id).await;
            fetch();
        });
    };

    view! {
        <div>
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-lg font-bold" style="color:var(--uci-text);">
                    <i class="fa-solid fa-users mr-2" style="color:var(--uci-accent);"></i>"Gestión de Personal"
                </h2>
                <button on:click=move |_| { reset_form(); set_show_form.update(|v| *v = !*v); }
                    class="px-4 py-2 rounded-lg text-sm font-medium text-white"
                    style="background:var(--uci-accent);">
                    <i class="fa-solid fa-plus mr-1"></i>{move || if show_form.get() { "Cancelar" } else { "Nuevo Personal" }}
                </button>
            </div>

            {move || error_msg.get().map(|e| view! {
                <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                    style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                    <i class="fa-solid fa-triangle-exclamation"></i>{e}
                </div>
            })}

            <Show when=move || show_form.get()>
                <div class="p-4 rounded-xl mb-6" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                    <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Nombre"</label>
                            <input class="form-input" type="text" placeholder="Nombre completo"
                                prop:value=move || form_nombre.get()
                                on:input=move |ev| set_form_nombre.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Usuario"</label>
                            <input class="form-input" type="text" placeholder="username"
                                prop:value=move || form_username.get()
                                on:input=move |ev| set_form_username.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Contraseña"</label>
                            <input class="form-input" type="password" placeholder={move || if edit_user.get().is_some() { "Dejar vacío para no cambiar" } else { "Contraseña" }}
                                prop:value=move || form_password.get()
                                on:input=move |ev| set_form_password.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Rol"</label>
                            <select class="form-select"
                                prop:value=move || form_rol.get()
                                on:change=move |ev| set_form_rol.set(event_target_value(&ev))>
                                <option value="Admin">"Admin"</option>
                                <option value="Medico">"Médico"</option>
                                <option value="Enfermero">"Enfermero"</option>
                                <option value="Viewer">"Viewer"</option>
                            </select>
                        </div>
                    </div>
                    <div class="flex justify-end mt-4">
                        <button on:click=move |_| save() class="btn-primary px-6 h-10 text-sm" disabled=saving>
                            {move || if saving.get() { "Guardando..." } else if edit_user.get().is_some() { "Actualizar" } else { "Crear" }}
                        </button>
                    </div>
                </div>
            </Show>

            <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <table class="w-full">
                    <thead style="background:var(--uci-bg);">
                        <tr>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Nombre"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Usuario"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Rol"</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Estado"</th>
                            <th class="px-4 py-3 text-right text-sm font-medium" style="color:var(--uci-muted);">"Acciones"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y" style="border-color:var(--uci-border);">
                        {move || staff.get().iter().map(|u| {
                            let user = u.clone();
                            let uid1 = u.user_id.clone();
                            let uid2 = u.user_id.clone();
                            let nombre = u.nombre.clone();
                            let username = u.username.clone();
                            let rol_label = u.rol.label().to_string();
                            let rol_class = match u.rol {
                                UserRole::Admin => "bg-purple-100 text-purple-700",
                                UserRole::Medico => "bg-blue-100 text-blue-700",
                                UserRole::Enfermero => "bg-pink-100 text-pink-700",
                                UserRole::Viewer => "bg-gray-100 text-gray-700",
                            };
                            let activo = u.activo;

                            view! {
                                <tr class="hover:bg-black/5">
                                    <td class="px-4 py-3 text-sm font-medium" style="color:var(--uci-text);">{nombre}</td>
                                    <td class="px-4 py-3 text-sm font-mono" style="color:var(--uci-muted);">{username}</td>
                                    <td class="px-4 py-3">
                                        <span class=format!("px-2 py-1 rounded-full text-xs font-medium {}", rol_class)>{rol_label}</span>
                                    </td>
                                    <td class="px-4 py-3">
                                        <button on:click=move |_| toggle_st(uid1.clone())
                                            class=format!("px-2 py-1 rounded-full text-xs font-medium {}",
                                                if activo { "bg-emerald-100 text-emerald-700" } else { "bg-gray-100 text-gray-700" }
                                            )>
                                            {if activo { "Activo" } else { "Inactivo" }}
                                        </button>
                                    </td>
                                    <td class="px-4 py-3 text-right">
                                        <button on:click=move |_| { let u = user.clone(); open_edit(u); }
                                            class="text-xs px-2 py-1 rounded hover:bg-black/10 mr-1">
                                            <i class="fa-solid fa-pen"></i>
                                        </button>
                                        <button on:click=move |_| delete_st(uid2.clone())
                                            class="text-xs px-2 py-1 rounded hover:bg-black/10" style="color:#DC2626;">
                                            <i class="fa-solid fa-trash"></i>
                                        </button>
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
