use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::api::{ClinicalDevice, RegisterDeviceRequest};
use crate::components::ui_kit::{ErrorState, LoadingState};
use crate::stores::user_has;

fn fmt_ts(ms: i64) -> String {
    if ms <= 0 {
        return "—".to_string();
    }
    let secs = ms / 1000;
    let dt = chrono::DateTime::from_timestamp(secs, 0);
    match dt {
        Some(d) => d.format("%Y-%m-%d %H:%M").to_string(),
        None => "—".to_string(),
    }
}

fn fmt_last_seen(ms: Option<i64>) -> String {
    match ms {
        Some(v) if v > 0 => fmt_ts(v),
        _ => "—".to_string(),
    }
}

fn estado_meta(estado: &str) -> (&'static str, &'static str) {
    match estado {
        "online" => ("#10B981", "En línea"),
        "offline" => ("#EF4444", "Fuera de línea"),
        "mantenimiento" => ("#F59E0B", "Mantenimiento"),
        _ => ("#6B7280", "Desconocido"),
    }
}

#[component]
pub fn DevicesPage() -> impl IntoView {
    let refresh = RwSignal::new(0u32);
    let filter = RwSignal::new(String::new());
    let show_form = RwSignal::new(false);
    let feedback = RwSignal::new(Option::<(bool, String)>::None);
    let saving = RwSignal::new(false);
    let can_write = user_has("devices:write");

    let device_type = RwSignal::new(String::new());
    let fabricante = RwSignal::new(String::new());
    let modelo = RwSignal::new(String::new());
    let firmware = RwSignal::new(String::new());
    let serial = RwSignal::new(String::new());
    let ubicacion = RwSignal::new(String::new());
    let estado = RwSignal::new("online".to_string());

    let status = LocalResource::new(move || {
        let _ = refresh.get();
        async move { api::get_device_status().await }
    });
    let devices = LocalResource::new(move || {
        let _ = refresh.get();
        let f = filter.get();
        async move { api::list_devices(Some(f)).await }
    });

    let submit = move |_| {
        if device_type.get().trim().is_empty()
            || fabricante.get().trim().is_empty()
            || modelo.get().trim().is_empty()
            || serial.get().trim().is_empty()
        {
            feedback.set(Some((false, "Complete tipo, fabricante, modelo y serial.".into())));
            return;
        }
        saving.set(true);
        feedback.set(None);
        let req = RegisterDeviceRequest {
            device_type: device_type.get(),
            fabricante: fabricante.get(),
            modelo: modelo.get(),
            firmware: firmware.get(),
            serial: serial.get(),
            cama_id: None,
            ubicacion: Some(ubicacion.get()).filter(|s| !s.is_empty()),
            estado: estado.get(),
        };
        spawn_local(async move {
            match api::register_device(req).await {
                Ok(d) => {
                    feedback.set(Some((true, format!("Dispositivo {} registrado.", d.id))));
                    show_form.set(false);
                    device_type.set(String::new());
                    fabricante.set(String::new());
                    modelo.set(String::new());
                    firmware.set(String::new());
                    serial.set(String::new());
                    ubicacion.set(String::new());
                    refresh.update(|n| *n += 1);
                }
                Err(e) => feedback.set(Some((false, e))),
            }
            saving.set(false);
        });
    };

    view! {
        <div class="page-enter">
            <div class="flex flex-wrap items-center justify-between gap-4 mb-6">
                <div>
                    <h1 class="text-2xl font-bold" style="color:var(--uci-text);">
                        <i class="fa-solid fa-microchip mr-2"></i>"Dispositivos y Monitores"
                    </h1>
                    <p class="text-sm mt-1" style="color:var(--uci-muted);">
                        "Inventario, estado y heartbeat de los dispositivos clínicos"
                    </p>
                </div>
                <div class="flex gap-2">
                    <Show when=move || can_write>
                        <button
                            class="btn-primary px-4 h-10 text-sm"
                            on:click=move |_| show_form.update(|v| *v = !*v)
                        >
                            <i class="fa-solid fa-plus mr-2"></i>"Registrar"
                        </button>
                    </Show>
                    <button
                        class="px-4 h-10 text-sm rounded-lg"
                        style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                        on:click=move |_| refresh.update(|n| *n += 1)
                    >
                        <i class="fa-solid fa-rotate mr-2"></i>"Actualizar"
                    </button>
                </div>
            </div>

            {move || feedback.get().map(|(ok, msg)| {
                let color = if ok { "#10B981" } else { "#EF4444" };
                let icon = if ok { "fa-circle-check" } else { "fa-circle-exclamation" };
                view! {
                    <div
                        class="flex items-start gap-3 p-4 rounded-xl mb-4"
                        style=format!("background:{color}1A; border:1px solid {color}66; color:var(--uci-text);")
                        role="status"
                    >
                        <i class=format!("fa-solid {icon} mt-0.5") style=format!("color:{color};")></i>
                        <span class="text-sm">{msg}</span>
                    </div>
                }
            })}

            <Suspense fallback=move || view! { <LoadingState label="Consultando dispositivos..." /> }>
                {move || match status.get() {
                    Some(Ok(s)) => view! { <SummaryCards summary=s /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo cargar el resumen: {}", e)
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>

            <Show when=move || show_form.get()>
                <div class="glass-card p-5 my-5">
                    <h3 class="text-sm font-bold uppercase mb-4" style="color:var(--uci-text);">
                        <i class="fa-solid fa-plus mr-2"></i>"Nuevo dispositivo"
                    </h3>
                    <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
                        <Field label="Tipo" sig=device_type placeholder="Monitor multiparamétrico" />
                        <Field label="Fabricante" sig=fabricante placeholder="Philips" />
                        <Field label="Modelo" sig=modelo placeholder="IntelliVue MX450" />
                        <Field label="Firmware (opcional)" sig=firmware placeholder="1.2.3" />
                        <Field label="Serial" sig=serial placeholder="SN-0001" />
                        <Field label="Ubicación (opcional)" sig=ubicacion placeholder="UCI-Cama 3" />
                        <div>
                            <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">"Estado"</label>
                            <select
                                class="w-full px-3 h-9 text-sm rounded-lg"
                                style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                prop:value=move || estado.get()
                                on:change=move |ev| estado.set(event_target_value(&ev))
                            >
                                <option value="online">"En línea"</option>
                                <option value="offline">"Fuera de línea"</option>
                                <option value="mantenimiento">"Mantenimiento"</option>
                            </select>
                        </div>
                    </div>
                    <div class="flex justify-end gap-3 mt-5">
                        <button
                            class="px-4 h-10 text-sm rounded-lg"
                            style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                            on:click=move |_| show_form.set(false)
                            disabled=move || saving.get()
                        >
                            "Cancelar"
                        </button>
                        <button class="btn-primary px-4 h-10 text-sm" on:click=submit disabled=move || saving.get()>
                            {move || if saving.get() { "Guardando..." } else { "Registrar" }}
                        </button>
                    </div>
                </div>
            </Show>

            <div class="flex items-center gap-3 my-5">
                <label class="text-sm" style="color:var(--uci-muted);">"Filtrar por estado:"</label>
                <select
                    class="px-3 h-9 text-sm rounded-lg"
                    style="background:var(--uci-surface); color:var(--uci-text); border:1px solid var(--uci-border);"
                    prop:value=move || filter.get()
                    on:change=move |ev| filter.set(event_target_value(&ev))
                >
                    <option value="">"Todos"</option>
                    <option value="online">"En línea"</option>
                    <option value="offline">"Fuera de línea"</option>
                    <option value="mantenimiento">"Mantenimiento"</option>
                </select>
            </div>

            <Suspense fallback=move || view! { <LoadingState label="Cargando dispositivos..." /> }>
                {move || match devices.get() {
                    Some(Ok(list)) => view! { <DevicesTable devices=list can_write=can_write refresh=refresh /> }.into_any(),
                    Some(Err(e)) => view! {
                        <ErrorState
                            message=format!("No se pudo cargar la lista: {}", e)
                            on_retry=Some(Callback::new(move |()| refresh.update(|n| *n += 1)))
                        />
                    }.into_any(),
                    None => view! {}.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn Field(
    label: &'static str,
    #[prop(into)] sig: RwSignal<String>,
    placeholder: &'static str,
) -> impl IntoView {
    view! {
        <div>
            <label class="block text-xs font-semibold mb-1" style="color:var(--uci-muted);">{label}</label>
            <input
                type="text"
                class="w-full px-3 h-9 text-sm rounded-lg"
                style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                prop:value=move || sig.get()
                on:input=move |ev| sig.set(event_target_value(&ev))
                placeholder=placeholder
            />
        </div>
    }
}

#[component]
fn SummaryCards(summary: api::DeviceStatusSummary) -> impl IntoView {
    let estado_cards = summary
        .por_estado
        .iter()
        .map(|c| {
            let (color, label) = estado_meta(&c.estado);
            view! {
                <div class="glass-card p-4">
                    <div class="text-xs uppercase font-semibold" style=format!("color:{color};")>{label}</div>
                    <div class="text-2xl font-bold mt-1" style="color:var(--uci-text);">{c.total}</div>
                </div>
            }
        })
        .collect_view();

    let tipos = summary
        .por_tipo
        .iter()
        .map(|t| {
            view! {
                <span
                    class="px-3 py-1 rounded-full text-xs font-semibold mr-2"
                    style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                >
                    {format!("{}: {}", t.device_type, t.total)}
                </span>
            }
        })
        .collect_view();

    view! {
        <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div class="glass-card p-4">
                <div class="text-xs uppercase font-semibold" style="color:var(--uci-muted);">"Total"</div>
                <div class="text-2xl font-bold mt-1" style="color:var(--uci-text);">{summary.total}</div>
            </div>
            {estado_cards}
        </div>
        <div class="mt-3">{tipos}</div>
    }
}

#[component]
fn DevicesTable(
    devices: Vec<ClinicalDevice>,
    can_write: bool,
    refresh: RwSignal<u32>,
) -> impl IntoView {
    if devices.is_empty() {
        return view! {
            <div class="p-10 text-center rounded-xl" style="background:var(--uci-surface); color:var(--uci-muted);">
                <i class="fa-solid fa-microchip text-2xl mb-2"></i>
                <p>"Sin dispositivos registrados"</p>
            </div>
        }.into_any();
    }

    let rows = devices
        .into_iter()
        .map(|d| {
            let (color, label) = estado_meta(&d.estado);
            let id = StoredValue::new(d.id.clone());
            view! {
                <tr class="hover:bg-black/5">
                    <td class="px-4 py-3 text-sm font-bold" style="color:var(--uci-text);">
                        {d.device_type.clone()}
                        <div class="text-xs font-normal" style="color:var(--uci-muted);">{format!("{} {}", d.fabricante, d.modelo)}</div>
                    </td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{d.serial}</td>
                    <td class="px-4 py-3">
                        <span class="px-2 py-1 rounded-full text-xs font-semibold" style=format!("background:{color}1A; color:{color};")>
                            {label}
                        </span>
                    </td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{d.ubicacion.clone().unwrap_or_else(|| "—".into())}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{fmt_last_seen(d.last_seen_at)}</td>
                    <td class="px-4 py-3 text-sm" style="color:var(--uci-muted);">{fmt_ts(d.registered_at)}</td>
                    <Show when=move || can_write>
                        <td class="px-4 py-3 text-right">
                            <button
                                class="text-xs px-3 py-1 rounded-lg"
                                style="background:var(--uci-bg); color:var(--uci-text); border:1px solid var(--uci-border);"
                                on:click=move |_| {
                                    let id = id.get_value();
                                    spawn_local(async move {
                                        let _ = api::heartbeat_device(&id).await;
                                        refresh.update(|n| *n += 1);
                                    });
                                }
                            >
                                <i class="fa-solid fa-heart-pulse mr-1"></i>"Heartbeat"
                            </button>
                        </td>
                    </Show>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="rounded-xl overflow-x-auto" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
            <table class="w-full">
                <thead style="background:var(--uci-bg);">
                    <tr>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Dispositivo"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Serial"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Estado"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Ubicación"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Últ. señal"</th>
                        <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">"Registrado"</th>
                        <Show when=move || can_write>
                            <th class="px-4 py-3 text-left text-sm font-medium" style="color:var(--uci-muted);">""</th>
                        </Show>
                    </tr>
                </thead>
                <tbody class="divide-y" style="border-color:var(--uci-border);">
                    {rows}
                </tbody>
            </table>
        </div>
    }.into_any()
}
