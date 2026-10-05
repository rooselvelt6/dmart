use crate::api;
use crate::components::ui_kit::{ErrorState, LoadingState};
use crate::i18n::trs;
use dmart_shared::models::*;
use leptos::either::Either;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn AdminPage() -> impl IntoView {
    let (active_tab, set_active_tab) = signal("camas".to_string());

    let admin_stats = LocalResource::new(|| async move { api::get_admin_stats().await.ok() });

    let tab_class = |tab: &str| {
        format!(
            "px-4 py-2 rounded-lg font-medium transition-all {}",
            if active_tab.get() == tab {
                "bg-uci-accent text-white".to_string()
            } else {
                "text-uci-muted hover:text-uci-text hover:bg-gray-100 dark:hover:bg-gray-800"
                    .to_string()
            }
        )
    };

    view! {
        <div class="min-h-screen p-6" style="background:var(--uci-bg);">
            <div class="max-w-7xl mx-auto">
                <div class="mb-8">
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-gear mr-2" style="color:var(--uci-accent);"></i>
                        {trs("adm-panel-title")}
                    </h1>
                    <p class="mt-1" style="color:var(--uci-muted);">{trs("adm-panel-subtitle")}</p>
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
                                {admin_stat_card(trs("adm-stat-camas"), stats.total_camas.to_string(), "#6366F1", "fa-bed")}
                                {admin_stat_card(trs("adm-stat-libres"), stats.camas_libres.to_string(), "#10B981", "fa-check")}
                                {admin_stat_card(trs("adm-stat-ocupadas"), stats.camas_ocupadas.to_string(), "#EF4444", "fa-xmark")}
                                {admin_stat_card(trs("adm-stat-equipos"), stats.total_equipos.to_string(), "#3B82F6", "fa-monitor-heart")}
                                {admin_stat_card(trs("adm-stat-disponibles"), stats.equipos_disponibles.to_string(), "#8B5CF6", "fa-box")}
                                {admin_stat_card(trs("adm-stat-medicos"), stats.medicos_activos.to_string(), "#F59E0B", "fa-user-doctor")}
                                {admin_stat_card(trs("adm-stat-enfermeros"), stats.enfermeros_activos.to_string(), "#EC4899", "fa-user-nurse")}
                            </div>
                        }),
                        None => Either::Right(view! {
                            <div class="p-4 mb-6 rounded-xl text-sm" style="background:rgba(239,68,68,0.1); color:#DC2626;">
                                <i class="fa-solid fa-triangle-exclamation mr-2"></i>{trs("adm-stats-load-error")}
                            </div>
                        }),
                    })}
                </Suspense>

                <div class="tabs flex gap-2 mb-6 pb-4" style="border-bottom:1px solid var(--uci-border);">
                    <button class=tab_class("camas") on:click=move |_| set_active_tab.set("camas".to_string())>
                        <i class="fa-solid fa-bed mr-2"></i>{trs("adm-tab-camas")}
                    </button>
                    <button class=tab_class("equipos") on:click=move |_| set_active_tab.set("equipos".to_string())>
                        <i class="fa-solid fa-monitor-heart mr-2"></i>{trs("adm-tab-equipos")}
                    </button>
                    <button class=tab_class("staff") on:click=move |_| set_active_tab.set("staff".to_string())>
                        <i class="fa-solid fa-users mr-2"></i>{trs("adm-tab-staff")}
                    </button>
                    <button class=tab_class("institucion") on:click=move |_| set_active_tab.set("institucion".to_string())>
                        <i class="fa-solid fa-hospital mr-2"></i>{trs("adm-tab-institucion")}
                    </button>
                    <button class=tab_class("auditoria") on:click=move |_| set_active_tab.set("auditoria".to_string())>
                        <i class="fa-solid fa-shield-halved mr-2"></i>{trs("adm-tab-auditoria")}
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
                <Show when=move || active_tab.get() == "institucion">
                    <InstitucionPanel/>
                </Show>
                <Show when=move || active_tab.get() == "auditoria">
                    <AuditPanel/>
                </Show>
            </div>
        </div>
    }
}

fn admin_stat_card(title: String, value: String, color: &str, icon: &str) -> impl IntoView + use<> {
    let c = color;
    view! {
        <div class="p-4 rounded-xl" style=format!("background:var(--uci-surface); border-top:2px solid {};", c)>
            <div class="flex items-center gap-2 mb-1">
                <i class=format!("fa-solid {} text-sm", icon) style=format!("color:{};", c)></i>
                <span class="text-[10px] uppercase font-bold" style="color:var(--uci-muted);">{title}</span>
            </div>
            <div class="text-safe text-2xl font-extrabold min-w-0 overflow-hidden" style=format!("color:{}; font-family:'JetBrains Mono',monospace;", c)>{value}</div>
        </div>
    }
}

// ─── Shared ──────────────────────────────────────────────────────

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

// ─── Institución Panel ────────────────────────────────────────────

#[component]
fn InstitucionPanel() -> impl IntoView {
    let (config, set_config) = signal(InstitucionConfig::default());
    let (guardado, set_guardado) = signal(false);
    let loading = RwSignal::new(false);
    let error_msg = RwSignal::new(None::<String>);

    spawn_local(async move {
        if let Ok(c) = api::get::<InstitucionConfig>("/admin/institucion").await {
            set_config.set(c);
        }
    });

    let save = move || {
        loading.set(true);
        error_msg.set(None);
        set_guardado.set(false);
        let data = config.get();
        spawn_local(async move {
            match api::put::<_, InstitucionConfig>("/admin/institucion", &data).await {
                Ok(_) => {
                    loading.set(false);
                    set_guardado.set(true);
                }
                Err(e) => {
                    loading.set(false);
                    error_msg.set(Some(e));
                }
            }
        });
    };

    view! {
        <div>
            <h2 class="text-lg font-bold mb-6" style="color:var(--uci-text);">
                <i class="fa-solid fa-hospital mr-2" style="color:var(--uci-accent);"></i>{trs("adm-institucion-title")}
            </h2>

            {move || error_msg.get().map(|e| view! {
                <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                    style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                    <i class="fa-solid fa-triangle-exclamation"></i>{e}
                </div>
            })}

            {move || (guardado.get()).then(|| view! {
                <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                    style="background:rgba(16,185,129,0.1); border:1px solid rgba(16,185,129,0.3); color:#10B981;">
                    <i class="fa-solid fa-check-circle"></i>{trs("adm-institucion-saved")}
                </div>
            })}

            <div class="p-6 rounded-xl" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-institucion-nombre-label")}</label>
                        <input class="form-input w-full" type="text"
                            prop:value=move || config.get().nombre
                            on:input=move |ev| set_config.update(|c| c.nombre = event_target_value(&ev)) />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-institucion-rif-label")}</label>
                        <input class="form-input w-full" type="text"
                            prop:value=move || config.get().rif
                            on:input=move |ev| set_config.update(|c| c.rif = event_target_value(&ev)) />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-institucion-direccion-label")}</label>
                        <input class="form-input w-full" type="text"
                            prop:value=move || config.get().direccion
                            on:input=move |ev| set_config.update(|c| c.direccion = event_target_value(&ev)) />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-institucion-telefono-label")}</label>
                        <input class="form-input w-full" type="text"
                            prop:value=move || config.get().telefono
                            on:input=move |ev| set_config.update(|c| c.telefono = event_target_value(&ev)) />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-institucion-email-label")}</label>
                        <input class="form-input w-full" type="email"
                            prop:value=move || config.get().email
                            on:input=move |ev| set_config.update(|c| c.email = event_target_value(&ev)) />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-institucion-logo-label")}</label>
                        <input class="form-input w-full" type="text"
                            prop:value=move || config.get().logo_url.unwrap_or_default()
                            on:input=move |ev| set_config.update(|c| c.logo_url = Some(event_target_value(&ev))) />
                    </div>
                </div>
                <div class="flex justify-end mt-6">
                    <button on:click=move |_| save() class="btn-primary px-6 h-10 text-sm" disabled=loading>
                        {move || if loading.get() { trs("adm-btn-saving") } else { trs("adm-btn-save") }}
                    </button>
                </div>
            </div>
        </div>
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
            if let Ok(p) = api::get::<PaginatedResponse<Cama>>("/admin/camas?limit=200").await {
                set_camas.set(p.items);
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
        set_form_tipo.set(format!("{:?}", c.tipo));
        set_form_estado.set(format!("{:?}", c.estado));
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
                    <i class="fa-solid fa-bed mr-2" style="color:var(--uci-accent);"></i>{trs("adm-camas-title")}
                </h2>
                <button on:click=move |_| { reset_form(); set_show_form.update(|v| *v = !*v); }
                    class="px-4 py-2 rounded-lg text-sm font-medium text-white"
                    style="background:var(--uci-accent);">
                    <i class="fa-solid fa-plus mr-1"></i>{move || if show_form.get() { trs("adm-btn-cancel") } else { trs("adm-btn-new-bed") }}
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
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-cama-numero-label")}</label>
                        <input type="number" min="1" class="form-input w-24"
                            prop:value=move || form_numero.get().to_string()
                            on:input=move |ev| { let v = event_target_value(&ev).parse().unwrap_or(1); set_form_numero.set(v); } />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-cama-tipo-label")}</label>
                        <select class="form-select"
                            prop:value=move || form_tipo.get()
                            on:change=move |ev| { set_form_tipo.set(event_target_value(&ev)); }>
                            <option value="General">{trs("adm-cama-tipo-general")}</option>
                            <option value="Aislamiento">{trs("adm-cama-tipo-aislamiento")}</option>
                            <option value="Pediatrica">{trs("adm-cama-tipo-pediatrica")}</option>
                            <option value="Coronaria">{trs("adm-cama-tipo-coronaria")}</option>
                            <option value="Quemados">{trs("adm-cama-tipo-quemados")}</option>
                            <option value="Otro">{trs("adm-cama-tipo-otro")}</option>
                        </select>
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-cama-estado-label")}</label>
                        <select class="form-select"
                            prop:value=move || form_estado.get()
                            on:change=move |ev| { set_form_estado.set(event_target_value(&ev)); }>
                            <option value="Libre">{trs("adm-cama-estado-libre")}</option>
                            <option value="Ocupada">{trs("adm-cama-estado-ocupada")}</option>
                            <option value="Mantenimiento">{trs("adm-cama-estado-mantenimiento")}</option>
                            <option value="Limpieza">{trs("adm-cama-estado-limpieza")}</option>
                        </select>
                    </div>
                    <button on:click=move |_| save() class="btn-primary px-6 h-10 text-sm" disabled=saving>
                        {move || if saving.get() { trs("adm-btn-saving") } else if edit_cama.get().is_some() { trs("adm-btn-update") } else { trs("adm-btn-create") }}
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
                    <i class="fa-solid fa-table mr-2"></i>{trs("adm-camas-registry-title")}
                </h3>
                <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                    <table class="w-full">
                        <thead style="background:var(--uci-bg);">
                            <tr>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"#"</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-numero")}</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-tipo")}</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-estado")}</th>
                                <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-paciente")}</th>
                                <th class="px-4 py-3 text-right text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-acciones")}</th>
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
            if let Ok(p) = api::get::<PaginatedResponse<Equipo>>("/admin/equipos?limit=200").await {
                set_equipos.set(p.items);
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
        set_form_estado.set(format!("{:?}", e.estado));
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
                    <i class="fa-solid fa-monitor-heart mr-2" style="color:var(--uci-accent);"></i>{trs("adm-equipos-title")}
                </h2>
                <button on:click=move |_| { reset_form(); set_show_form.update(|v| *v = !*v); }
                    class="px-4 py-2 rounded-lg text-sm font-medium text-white"
                    style="background:var(--uci-accent);">
                    <i class="fa-solid fa-plus mr-1"></i>{move || if show_form.get() { trs("adm-btn-cancel") } else { trs("adm-btn-new-equipo") }}
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
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-equipo-nombre-label")}</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_nombre.get()
                                on:input=move |ev| set_form_nombre.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-equipo-tipo-label")}</label>
                            <select class="form-select"
                                prop:value=move || form_tipo.get()
                                on:change=move |ev| set_form_tipo.set(event_target_value(&ev))>
                                <option value="VentiladorMecanico">{trs("adm-equipo-tipo-ventilador")}</option>
                                <option value="Monitor">{trs("adm-equipo-tipo-monitor")}</option>
                                <option value="Computador">{trs("adm-equipo-tipo-computador")}</option>
                                <option value="BombaInfusion">{trs("adm-equipo-tipo-bomba")}</option>
                                <option value="Otro">{trs("adm-equipo-tipo-otro")}</option>
                            </select>
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-equipo-marca-label")}</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_marca.get()
                                on:input=move |ev| set_form_marca.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-equipo-modelo-label")}</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_modelo.get()
                                on:input=move |ev| set_form_modelo.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-equipo-serial-label")}</label>
                            <input class="form-input" type="text"
                                prop:value=move || form_serial.get()
                                on:input=move |ev| set_form_serial.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="flex items-end gap-4 mt-4">
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-equipo-estado-label")}</label>
                            <select class="form-select"
                                prop:value=move || form_estado.get()
                                on:change=move |ev| set_form_estado.set(event_target_value(&ev))>
                                <option value="Activo">{trs("adm-equipo-estado-activo")}</option>
                                <option value="Mantenimiento">{trs("adm-equipo-estado-mantenimiento")}</option>
                                <option value="Inactivo">{trs("adm-equipo-estado-inactivo")}</option>
                                <option value="Reparacion">{trs("adm-equipo-estado-reparacion")}</option>
                            </select>
                        </div>
                        <button on:click=move |_| save() class="btn-primary px-6 h-10 text-sm" disabled=saving>
                            {move || if saving.get() { trs("adm-btn-saving") } else if edit_equipo.get().is_some() { trs("adm-btn-update") } else { trs("adm-btn-create") }}
                        </button>
                    </div>
                </div>
            </Show>

            <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <table class="w-full">
                    <thead style="background:var(--uci-bg);">
                        <tr>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-nombre")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-tipo")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-marca-modelo")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-serial")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-equipo-estado-label")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-cama")}</th>
                            <th class="px-4 py-3 text-right text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-acciones")}</th>
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
    let (staff, set_staff) = signal::<Vec<StaffInfo>>(vec![]);
    let (rol_filter, set_rol_filter) = signal("todos".to_string());
    let (show_form, set_show_form) = signal(false);
    let (edit_user, set_edit_user) = signal::<Option<StaffInfo>>(None);
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

    let open_edit = move |u: StaffInfo| {
        set_edit_user.set(Some(u.clone()));
        set_form_nombre.set(u.nombre.clone());
        set_form_username.set(u.username.clone());
        set_form_password.set(String::new());
        set_form_rol.set(u.rol.to_string());
        set_show_form.set(true);
    };

    let save = move || {
        let nombre = form_nombre.get();
        let username = form_username.get();
        let password = form_password.get();
        let rol_str = form_rol.get();
        let user_id = edit_user.get().map(|u| u.user_id.clone());

        if nombre.trim().is_empty() {
            error_msg.set(Some(trs("adm-staff-error-name-required")));
            return;
        }
        if user_id.is_none() && username.trim().is_empty() {
            error_msg.set(Some(trs("adm-staff-error-user-required")));
            return;
        }
        if user_id.is_none() && password.len() < 8 {
            error_msg.set(Some(trs("adm-staff-error-pass-min")));
            return;
        }
        if user_id.is_some() && !password.is_empty() && password.len() < 8 {
            error_msg.set(Some(trs("adm-staff-error-pass-min")));
            return;
        }

        saving.set(true);
        error_msg.set(None);

        spawn_local(async move {
            let result = if let Some(ref id) = user_id {
                api::update_staff(id, &nombre, &rol_str, &password)
                    .await
                    .map(|_| ())
            } else {
                api::create_staff(&username, &nombre, &rol_str, &password)
                    .await
                    .map(|_| ())
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
            <div class="flex items-center justify-between mb-6 flex-wrap gap-3">
                <h2 class="text-lg font-bold" style="color:var(--uci-text);">
                    <i class="fa-solid fa-users mr-2" style="color:var(--uci-accent);"></i>{trs("adm-staff-title")}
                </h2>
                <div class="flex items-center gap-3">
                    <select class="form-select" aria-label="Filtrar por rol"
                        prop:value=move || rol_filter.get()
                        on:change=move |ev| set_rol_filter.set(event_target_value(&ev))>
                        <option value="todos">{trs("adm-staff-filter-all")}</option>
                        <option value="Admin">{trs("adm-staff-role-admin")}</option>
                        <option value="Medico">{trs("adm-staff-role-medico")}</option>
                        <option value="Enfermero">{trs("adm-staff-role-enfermero")}</option>
                        <option value="Viewer">{trs("adm-staff-role-viewer")}</option>
                        <option value="Soporte">{trs("adm-staff-role-soporte")}</option>
                    </select>
                    <button on:click=move |_| { reset_form(); set_show_form.update(|v| *v = !*v); }
                        class="px-4 py-2 rounded-lg text-sm font-medium text-white"
                        style="background:var(--uci-accent);">
                        <i class="fa-solid fa-plus mr-1"></i>{move || if show_form.get() { trs("adm-btn-cancel") } else { trs("adm-btn-new-staff") }}
                    </button>
                </div>
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
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-staff-nombre-label")}</label>
                            <input class="form-input" type="text" placeholder="Nombre completo"
                                prop:value=move || form_nombre.get()
                                on:input=move |ev| set_form_nombre.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-staff-usuario-label")}</label>
                            <input class="form-input" type="text" placeholder="username"
                                prop:value=move || form_username.get()
                                on:input=move |ev| set_form_username.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-staff-password-label")}</label>
                            <input class="form-input" type="password" placeholder={move || if edit_user.get().is_some() { trs("adm-staff-password-placeholder") } else { trs("adm-staff-password-label") }}
                                prop:value=move || form_password.get()
                                on:input=move |ev| set_form_password.set(event_target_value(&ev)) />
                        </div>
                        <div>
                            <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">{trs("adm-staff-rol-label")}</label>
                            <select class="form-select"
                                prop:value=move || form_rol.get()
                                on:change=move |ev| set_form_rol.set(event_target_value(&ev))>
                                <option value="Admin">{trs("adm-staff-role-admin")}</option>
                                <option value="Medico">{trs("adm-staff-role-medico")}</option>
                                <option value="Enfermero">{trs("adm-staff-role-enfermero")}</option>
                                <option value="Viewer">{trs("adm-staff-role-viewer")}</option>
                                <option value="Soporte">{trs("adm-staff-role-soporte")}</option>
                            </select>
                        </div>
                    </div>
                    <div class="flex justify-end mt-4">
                        <button on:click=move |_| save() class="btn-primary px-6 h-10 text-sm" disabled=saving>
                            {move || if saving.get() { trs("adm-btn-saving") } else if edit_user.get().is_some() { trs("adm-btn-update") } else { trs("adm-btn-create") }}
                        </button>
                    </div>
                </div>
            </Show>

            <div class="rounded-xl overflow-hidden" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <table class="w-full">
                    <thead style="background:var(--uci-bg);">
                        <tr>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-staff-nombre-label")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-usuario")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-rol")}</th>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-equipo-estado-label")}</th>
                            <th class="px-4 py-3 text-right text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-table-acciones")}</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y" style="border-color:var(--uci-border);">
                        {move || {
                            let filtro = rol_filter.get();
                            staff.get().iter()
                                .filter(|u| filtro == "todos" || u.rol.to_string() == filtro)
                                .map(|u| {
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
                                UserRole::Soporte => "bg-amber-100 text-amber-700",
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
                                            {if activo { trs("adm-status-activo") } else { trs("adm-status-inactivo") }}
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
                        }).collect_view()}}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

const AUDIT_ACTIONS: &[(&str, &str)] = &[
    ("", "adm-audit-filter-todas"),
    ("LOGIN", "adm-audit-action-login"),
    ("LOGIN_FAILED", "adm-audit-action-login-failed"),
    ("LOGOUT", "adm-audit-action-logout"),
    ("CREATE", "adm-audit-action-create"),
    ("READ", "adm-audit-action-read"),
    ("UPDATE", "adm-audit-action-update"),
    ("DELETE", "adm-audit-action-delete"),
    ("EXPORT", "adm-audit-action-export"),
    ("CONFIG_CHANGE", "adm-audit-action-config-change"),
    ("AUTH_CHANGE", "adm-audit-action-auth-change"),
    ("ACCESS_DENIED", "adm-audit-action-access-denied"),
    ("DATA_ACCESS", "adm-audit-action-data-access"),
    ("DATA_MODIFICATION", "adm-audit-action-data-modification"),
];

fn audit_action_class(action: &str) -> &'static str {
    match action {
        "LOGIN_FAILED" | "ACCESS_DENIED" | "DELETE" => "bg-red-100 text-red-700",
        "CONFIG_CHANGE" | "AUTH_CHANGE" => "bg-purple-100 text-purple-700",
        "EXPORT" => "bg-amber-100 text-amber-700",
        "CREATE" | "DATA_MODIFICATION" => "bg-emerald-100 text-emerald-700",
        _ => "bg-gray-100 text-gray-700",
    }
}

#[component]
fn AuditPanel() -> impl IntoView {
    let (limit, set_limit) = signal(50usize);
    let (action_filter, set_action_filter) = signal(String::new());
    let (refresh, set_refresh) = signal(0u32);
    let (report, set_report) = signal(Option::<api::IntegrityReport>::None);
    let (busy, set_busy) = signal(false);
    let (msg, set_msg) = signal(Option::<(bool, String)>::None);

    let logs = LocalResource::new(move || {
        let action = action_filter.get();
        let lim = limit.get();
        let _ = refresh.get();
        async move {
            let action = if action.is_empty() {
                None
            } else {
                Some(action.as_str())
            };
            api::get_audit_logs(lim, action).await
        }
    });
let uci_stats = LocalResource::new(|| async move { api::get_stats().await.ok() });

    let do_verify = move |_| {
        set_busy.set(true);
        set_msg.set(None);
        spawn_local(async move {
            match api::verify_audit_chain().await {
                Ok(r) => set_report.set(Some(r)),
                Err(e) => set_msg.set(Some((false, e))),
            }
            set_busy.set(false);
        });
    };

    let do_seal = move |_| {
        set_busy.set(true);
        set_msg.set(None);
        spawn_local(async move {
            match api::seal_audit_batch().await {
                Ok(Some(b)) => set_msg.set(Some((
                    true,
                    format!("{} #{}: {} {}.", trs("adm-audit-batch-sealed"), b.sequence, b.count, trs("adm-audit-events")),
                ))),
                Ok(None) => set_msg.set(Some((
                    true,
                    trs("adm-audit-no-events-to-seal"),
                ))),
                Err(e) => set_msg.set(Some((false, e))),
            }
            set_busy.set(false);
            set_refresh.update(|n| *n += 1);
        });
    };

    view! {
        <div class="space-y-4">
            <Suspense fallback=move || view! { <div class="h-16 animate-pulse rounded-xl" style="background:var(--uci-surface);"></div> }>
                {move || uci_stats.get().map(|s| match s {
                    Some(stats) => view! {
                        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
                            {admin_stat_card(trs("adm-stat-pacientes"), stats.total_pacientes.to_string(), "#6366F1", "fa-users")}
                            {admin_stat_card(trs("adm-stat-activos"), stats.pacientes_activos.to_string(), "#10B981", "fa-heart-pulse")}
                            {admin_stat_card(trs("adm-stat-criticos"), stats.por_gravedad.criticos.to_string(), "#EF4444", "fa-triangle-exclamation")}
                            {admin_stat_card(trs("adm-stat-severos"), stats.por_gravedad.severos.to_string(), "#F59E0B", "fa-circle-exclamation")}
                            {admin_stat_card(trs("adm-stat-mortalidad"), format!("{:.1}%", stats.ejecutivo.mortalidad_real_pct), "#8B5CF6", "fa-chart-line")}
                            {admin_stat_card(trs("adm-stat-los"), format!("{:.1}d", stats.ejecutivo.los_dias_promedio), "#3B82F6", "fa-clock")}
                        </div>
                    }.into_any(),
                    None => ().into_any(),
                })}
            </Suspense>

            <div class="rounded-xl p-4" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <div class="flex flex-wrap items-end gap-4">
                    <div>
                        <label class="block text-xs font-medium mb-1" style="color:var(--uci-muted);">{trs("adm-audit-filter-label")}</label>
                        <select
                            class="px-3 py-2 rounded-lg text-sm"
                            style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                            prop:value=move || action_filter.get()
                            on:change=move |ev| {
                                set_action_filter.set(event_target_value(&ev));
                                set_refresh.update(|n| *n += 1);
                            }
                        >
                            {AUDIT_ACTIONS.iter().map(|(v, l)| view! {
                                <option value=*v>{trs(l)}</option>
                            }).collect_view()}
                        </select>
                    </div>
                    <div>
                        <label class="block text-xs font-medium mb-1" style="color:var(--uci-muted);">{trs("adm-audit-limit-label")}</label>
                        <select
                            class="px-3 py-2 rounded-lg text-sm"
                            style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                            on:change=move |ev| {
                                if let Ok(n) = event_target_value(&ev).parse::<usize>() {
                                    set_limit.set(n);
                                }
                                set_refresh.update(|n| *n += 1);
                            }
                        >
                            <option value="25">"25"</option>
                            <option value="50" selected>"50"</option>
                            <option value="100">"100"</option>
                            <option value="200">"200"</option>
                        </select>
                    </div>
                    <div class="flex-1"></div>
                    <button
                        class="px-4 h-10 rounded-lg text-sm font-medium"
                        style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                        disabled=move || busy.get()
                        on:click=do_verify
                    >
                        <i class="fa-solid fa-link mr-2"></i>{trs("adm-btn-verify-chain")}
                    </button>
                    <button
                        class="btn-primary px-4 h-10 text-sm"
                        disabled=move || busy.get()
                        on:click=do_seal
                    >
                        <i class="fa-solid fa-lock mr-2"></i>{trs("adm-btn-seal-batch")}
                    </button>
                </div>

                {move || msg.get().map(|(ok, text)| view! {
                    <div class="mt-3 p-3 rounded-lg text-sm"
                        style=if ok { "background:rgba(16,185,129,0.12); color:#059669;" }
                        else { "background:rgba(239,68,68,0.12); color:#DC2626;" }>
                        <i class=format!("fa-solid {} mr-2", if ok { "fa-circle-check" } else { "fa-triangle-exclamation" })></i>
                        {text}
                    </div>
                })}

                {move || report.get().map(|r| {
                    let color = if r.ok { "#10B981" } else { "#EF4444" };
                    view! {
                        <div class="mt-3 p-3 rounded-lg text-sm" style=format!("background:{}1A; color:{};", color, color)>
                            <div class="font-semibold mb-1">
                                {if r.ok { trs("adm-audit-chain-intact") } else { trs("adm-audit-chain-compromised") }}
                            </div>
                            <div>
                                {format!(
                                    "{} {} · {} {} · {} {} · {} {} · {} {} · {} {} · {} {}",
                                    r.logs_total, trs("adm-audit-total"),
                                    r.logs_hashed, trs("adm-audit-hashed"),
                                    r.logs_valid, trs("adm-audit-valid"),
                                    r.logs_unhashed, trs("adm-audit-unhashed"),
                                    r.batches_total, trs("adm-audit-batches"),
                                    r.signatures_valid, trs("adm-audit-signatures"),
                                    r.sealable_logs, trs("adm-audit-sealable")
                                )}
                            </div>
                        </div>
                    }
                })}

            </div>

            <Suspense fallback=move || view! { <LoadingState label={trs("adm-audit-loading")}/> }>
                {move || logs.get().map(|res| match res {
                    Ok(list) if list.is_empty() => view! {
                        <div class="p-8 text-center rounded-xl text-sm" style="background:var(--uci-surface); color:var(--uci-muted);">
                            <i class="fa-solid fa-inbox text-2xl mb-2"></i>
                            <p>{trs("adm-audit-empty")}</p>
                        </div>
                    }.into_any(),
                    Ok(list) => view! {
                        <AuditLogsTable logs=list />
                    }.into_any(),
                    Err(e) => view! {
                        <ErrorState message=e on_retry=Some(Callback::new(move |_| set_refresh.update(|n| *n += 1))) />
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn AuditLogsTable(logs: Vec<api::AuditLog>) -> impl IntoView {
    let rows = logs
        .into_iter()
        .map(|l| {
            let class = audit_action_class(&l.action).to_string();
            let success_color = if l.success { "#10B981" } else { "#EF4444" };
            let success_label = if l.success { trs("adm-audit-result-ok") } else { trs("adm-audit-result-fail") };
            let usuario = l
                .username
                .clone()
                .or_else(|| l.user_id.clone())
                .unwrap_or_else(|| "—".to_string());
            let ip = l.ip_address.clone().unwrap_or_else(|| "—".to_string());
            let detalle = l
                .details
                .clone()
                .or_else(|| l.error_message.clone())
                .unwrap_or_default();
            let resource = match &l.resource_id {
                Some(id) => format!("{}#{}", l.resource, id),
                None => l.resource.clone(),
            };
            view! {
                <tr class="hover:bg-black/5">
                    <td class="px-4 py-3 text-xs whitespace-nowrap" style="color:var(--uci-muted);">{l.timestamp.clone()}</td>
                    <td class="px-4 py-3 text-sm font-medium" style="color:var(--uci-text);">{usuario}</td>
                    <td class="px-4 py-3">
                        <span class=format!("px-2 py-1 rounded-full text-xs font-semibold {}", class)>{l.action.clone()}</span>
                    </td>
                    <td class="px-4 py-3 text-sm font-mono text-xs" style="color:var(--uci-muted);">{resource}</td>
                    <td class="px-4 py-3 text-xs" style="color:var(--uci-muted);">{ip}</td>
                    <td class="px-4 py-3">
                        <span class="text-xs font-semibold" style=format!("color:{};", success_color)>{success_label}</span>
                    </td>
                    <td class="px-4 py-3 text-xs max-w-xs truncate" style="color:var(--uci-muted);">{detalle}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="rounded-xl overflow-x-auto" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <table class="w-full">
                <thead style="background:var(--uci-bg);">
                    <tr>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-audit-col-fecha")}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-audit-col-usuario")}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-audit-col-accion")}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-audit-col-recurso")}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-audit-col-ip")}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-audit-col-resultado")}</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">{trs("adm-audit-col-detalles")}</th>
                    </tr>
                </thead>
                <tbody class="divide-y" style="border-color:var(--uci-border);">
                    {rows}
                </tbody>
            </table>
        </div>
    }
}
