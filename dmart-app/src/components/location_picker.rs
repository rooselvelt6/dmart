use leptos::prelude::*;
use std::collections::BTreeMap;

type GeoData = BTreeMap<String, BTreeMap<String, Vec<String>>>;

fn load_geo_data() -> GeoData {
    let json = include_str!("../data/geodata.json");
    serde_json::from_str(json).unwrap_or_default()
}

fn es_pais_libre(pais: &str) -> bool {
    pais == "Otro" || pais.is_empty()
}

#[component]
pub fn LocationPicker<F1, F2, F3>(
    pais: Signal<String>,
    estado: Signal<String>,
    ciudad: Signal<String>,
    on_change_pais: F1,
    on_change_estado: F2,
    on_change_ciudad: F3,
) -> impl IntoView
where
    F1: Fn(String) + Send + Sync + 'static,
    F2: Fn(String) + Send + Sync + 'static,
    F3: Fn(String) + Send + Sync + 'static,
{
    let geo = StoredValue::new(load_geo_data());
    let paises: Vec<String> = geo.with_value(|g| g.keys().cloned().collect());
    let paises = StoredValue::new(paises);

    let es_libre = Memo::new(move |_| es_pais_libre(&pais.get()));

    let on_change_pais = StoredValue::new(on_change_pais);
    let on_change_estado = StoredValue::new(on_change_estado);
    let on_change_ciudad = StoredValue::new(on_change_ciudad);

    let estados_lista = Memo::new(move |_| {
        let p = pais.get();
        geo.with_value(|g| {
            g.get(&p)
                .map(|e| e.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        })
    });

    let ciudades_lista = Memo::new(move |_| {
        let p = pais.get();
        let e = estado.get();
        geo.with_value(|g| {
            g.get(&p)
                .and_then(|m| m.get(&e))
                .cloned()
                .unwrap_or_default()
        })
    });

    view! {
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
                <label class="form-label flex items-center gap-2 text-xs font-black uppercase tracking-widest" style="color:var(--uci-muted);">
                    <i class="fa-solid fa-globe"></i>"País"
                </label>
                <select class="form-select"
                    prop:value=move || pais.get()
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        on_change_pais.with_value(|f| f(v.clone()));
                        if es_pais_libre(&v) {
                            on_change_estado.with_value(|f| f(String::new()));
                            on_change_ciudad.with_value(|f| f(String::new()));
                        }
                    }
                >
                    <option value="" disabled selected=pais.get().is_empty()>"Seleccione un país"</option>
                    {move || paises.with_value(|p| {
                        let current = pais.get();
                        p.iter().map(|nombre| {
                            let selected = current == *nombre;
                            view! {
                                <option value=nombre.clone() selected=selected>{nombre.clone()}</option>
                            }
                        }).collect_view()
                    })}
                </select>
            </div>

            {move || if es_libre.get() {
                view! {
                    <>
                        <div>
                            <label class="form-label flex items-center gap-2 text-xs font-black uppercase tracking-widest" style="color:var(--uci-muted);">
                                <i class="fa-solid fa-map-location-dot"></i>"Estado / Provincia"
                            </label>
                            <input class="form-input" type="text" placeholder="Estado / Provincia"
                                prop:value=move || estado.get()
                                on:input=move |ev| on_change_estado.with_value(|f| f(event_target_value(&ev))) />
                        </div>
                        <div>
                            <label class="form-label flex items-center gap-2 text-xs font-black uppercase tracking-widest" style="color:var(--uci-muted);">
                                <i class="fa-solid fa-city"></i>"Ciudad"
                            </label>
                            <input class="form-input" type="text" placeholder="Ciudad"
                                prop:value=move || ciudad.get()
                                on:input=move |ev| on_change_ciudad.with_value(|f| f(event_target_value(&ev))) />
                        </div>
                    </>
                }.into_any()
            } else {
                view! {
                    <>
                        <div>
                            <label class="form-label flex items-center gap-2 text-xs font-black uppercase tracking-widest" style="color:var(--uci-muted);">
                                <i class="fa-solid fa-map-location-dot"></i>"Estado / Provincia"
                            </label>
                            <select class="form-select"
                                prop:value=move || estado.get()
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    on_change_estado.with_value(|f| f(v.clone()));
                                    on_change_ciudad.with_value(|f| f(String::new()));
                                }
                            >
                                <option value="" disabled selected=estado.get().is_empty()>"Seleccione un estado"</option>
                                {move || {
                                    let list = estados_lista.get();
                                    let current = estado.get();
                                    list.iter().map(|nombre| {
                                        let selected = current == *nombre;
                                        view! {
                                            <option value=nombre.clone() selected=selected>{nombre.clone()}</option>
                                        }
                                    }).collect_view()
                                }}
                            </select>
                        </div>
                        <div>
                            <label class="form-label flex items-center gap-2 text-xs font-black uppercase tracking-widest" style="color:var(--uci-muted);">
                                <i class="fa-solid fa-city"></i>"Ciudad"
                            </label>
                            <select class="form-select"
                                prop:value=move || ciudad.get()
                                on:change=move |ev| on_change_ciudad.with_value(|f| f(event_target_value(&ev)))
                            >
                                <option value="" disabled selected=ciudad.get().is_empty()>"Seleccione una ciudad"</option>
                                {move || {
                                    let list = ciudades_lista.get();
                                    let current = ciudad.get();
                                    list.iter().map(|nombre| {
                                        let selected = current == *nombre;
                                        view! {
                                            <option value=nombre.clone() selected=selected>{nombre.clone()}</option>
                                        }
                                    }).collect_view()
                                }}
                            </select>
                        </div>
                    </>
                }.into_any()
            }}
        </div>
    }
}
