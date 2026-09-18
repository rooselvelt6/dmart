use crate::api;
use crate::stores::current_user;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

const B64URL_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | (*chunk.get(2).unwrap_or(&0) as u32);
        out.push(B64URL_TABLE[((n >> 18) & 63) as usize] as char);
        out.push(B64URL_TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64URL_TABLE[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64URL_TABLE[(n & 63) as usize] as char);
        }
    }
    out
}

fn b64url_decode_bytes(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input.trim().chars().filter(|c| *c != '=').collect();
    let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
    let mut acc = 0u32;
    let mut acc_bits = 0u8;
    for c in cleaned.bytes() {
        let v = B64URL_TABLE
            .iter()
            .position(|b| *b == c)
            .ok_or("base64url inválido")? as u32;
        acc = (acc << 6) | v;
        acc_bits += 6;
        if acc_bits >= 8 {
            acc_bits -= 8;
            out.push((acc >> acc_bits) as u8);
        }
    }
    Ok(out)
}

/// Navegador con soporte Web Push y registration del Service Worker lista.
async fn push_registration() -> Result<web_sys::ServiceWorkerRegistration, String> {
    let window = web_sys::window().ok_or_else(|| "sin ventana".to_string())?;
    let container = window.navigator().service_worker();
    let promise = container.ready().map_err(|e| format!("{e:?}"))?;
    let value = JsFuture::from(promise).await.map_err(|e| format!("{e:?}"))?;
    value.dyn_into().map_err(|e| format!("{e:?}"))
}

/// Página "Mi Perfil": identidad activa + cambio de contraseña.
///
/// El cambio de contraseña revoca todas las sesiones en el servidor, así que
/// tras aplicarlo se limpia la sesión local y se vuelve al login.
#[component]
pub fn PerfilPage() -> impl IntoView {
    let user = current_user();
    let has_user = user.is_some();
    let (u_nombre, u_username, u_rol, u_initial) = match user {
        Some(u) => (
            u.nombre.clone(),
            u.username.clone(),
            u.rol.label().to_string(),
            u.nombre
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default(),
        ),
        None => (String::new(), String::new(), String::new(), String::new()),
    };
    let (current, set_current) = signal(String::new());
    let (new_pass, set_new_pass) = signal(String::new());
    let (confirm, set_confirm) = signal(String::new());
    let (error_msg, set_error_msg) = signal::<Option<String>>(None);
    let (info_msg, set_info_msg) = signal::<Option<String>>(None);
    let (saving, set_saving) = signal(false);

    // MFA (segundo factor)
    let (mfa_enabled, set_mfa_enabled) = signal::<Option<bool>>(None);
    let (mfa_setup, set_mfa_setup) = signal::<Option<api::MfaSetupInfo>>(None);
    let (mfa_code, set_mfa_code) = signal(String::new());
    let (mfa_error, set_mfa_error) = signal::<Option<String>>(None);
    let (mfa_info, set_mfa_info) = signal::<Option<String>>(None);
    let (mfa_busy, set_mfa_busy) = signal(false);

    // Web Push (SPEC-052): activación/desactivación de notificaciones del navegador.
    let (push_enabled, set_push_enabled) = signal(false);
    let (push_busy, set_push_busy) = signal(false);
    let (push_msg, set_push_msg) = signal::<Option<(String, String)>>(None);

    spawn_local(async move {
        if let Ok(enabled) = api::mfa_status().await {
            set_mfa_enabled.set(Some(enabled));
        }
    });

    let on_start_setup = move |_| {
        set_mfa_error.set(None);
        set_mfa_info.set(None);
        set_mfa_busy.set(true);
        spawn_local(async move {
            match api::mfa_setup().await {
                Ok(info) => {
                    set_mfa_setup.set(Some(info));
                    set_mfa_code.set(String::new());
                    set_mfa_busy.set(false);
                }
                Err(e) => {
                    set_mfa_busy.set(false);
                    set_mfa_error.set(Some(e));
                }
            }
        });
    };

    let on_confirm_mfa = move |_| {
        let code = mfa_code.get();
        if code.trim().is_empty() {
            set_mfa_error.set(Some("Ingresa el código de tu autenticador".to_string()));
            return;
        }
        set_mfa_error.set(None);
        set_mfa_busy.set(true);
        spawn_local(async move {
            match api::mfa_confirm(&code).await {
                Ok(()) => {
                    set_mfa_setup.set(None);
                    set_mfa_enabled.set(Some(true));
                    set_mfa_busy.set(false);
                    set_mfa_info.set(Some(
                        "Verificación en dos pasos activada correctamente.".to_string(),
                    ));
                }
                Err(e) => {
                    set_mfa_busy.set(false);
                    set_mfa_error.set(Some(e));
                }
            }
        });
    };

    let on_disable_mfa = move |_| {
        let code = mfa_code.get();
        if code.trim().is_empty() {
            set_mfa_error.set(Some("Ingresa un código actual para desactivar".to_string()));
            return;
        }
        set_mfa_error.set(None);
        set_mfa_busy.set(true);
        spawn_local(async move {
            match api::mfa_disable(&code).await {
                Ok(()) => {
                    set_mfa_enabled.set(Some(false));
                    set_mfa_code.set(String::new());
                    set_mfa_busy.set(false);
                    set_mfa_info.set(Some("Verificación en dos pasos desactivada.".to_string()));
                }
                Err(e) => {
                    set_mfa_busy.set(false);
                    set_mfa_error.set(Some(e));
                }
            }
        });
    };

    let navigate = use_navigate();
    let set_is_auth = use_context::<WriteSignal<bool>>();

    let on_enable_push = move |_| {
        set_push_msg.set(None);
        set_push_busy.set(true);
        spawn_local(async move {
            let result: Result<(), String> = async {
                let reg = push_registration().await?;
                let pub_key_b64 = api::push_public_key().await?;
                let key_bytes = b64url_decode_bytes(&pub_key_b64)?;
                let key_arr = js_sys::Uint8Array::from(&key_bytes[..]);
                let options = web_sys::PushSubscriptionOptionsInit::new();
                options.set_user_visible_only(true);
                options.set_application_server_key(&key_arr.into());
                let promise = reg.push_manager().map_err(|e| format!("{e:?}"))?
                    .subscribe_with_options(&options)
                    .map_err(|e| format!("{e:?}"))?;
                let value = JsFuture::from(promise).await.map_err(|e| format!("{e:?}"))?;
                let sub: web_sys::PushSubscription = value.dyn_into().map_err(|e| format!("{e:?}"))?;
                let endpoint = sub.endpoint();
                let p256dh = sub
                    .get_key(web_sys::PushEncryptionKeyName::P256dh)
                    .map_err(|e| format!("{e:?}"))?
                    .map(|buf| js_sys::Uint8Array::new(&buf).to_vec())
                    .map(|b| b64url_encode_bytes(&b))
                    .unwrap_or_default();
                let auth = sub
                    .get_key(web_sys::PushEncryptionKeyName::Auth)
                    .map_err(|e| format!("{e:?}"))?
                    .map(|buf| js_sys::Uint8Array::new(&buf).to_vec())
                    .map(|b| b64url_encode_bytes(&b))
                    .unwrap_or_default();
                if endpoint.is_empty() || p256dh.is_empty() || auth.is_empty() {
                    return Err("claves de suscripción incompletas".to_string());
                }
                api::push_subscribe(&endpoint, &p256dh, &auth).await?;
                Ok(())
            }
            .await;
            set_push_busy.set(false);
            match result {
                Ok(()) => {
                    set_push_enabled.set(true);
                    set_push_msg.set(Some((
                        "success".to_string(),
                        "Notificaciones del navegador activadas. Recibirás alertas de escalamiento aunque la app esté en segundo plano.".to_string(),
                    )));
                }
                Err(e) => {
                    set_push_msg.set(Some((
                        "error".to_string(),
                        format!("No se pudo activar: {e}"),
                    )));
                }
            }
        });
    };

    let on_disable_push = move |_| {
        set_push_msg.set(None);
        set_push_busy.set(true);
        spawn_local(async move {
            let result: Result<(), String> = async {
                let reg = push_registration().await?;
                let pm = reg.push_manager().map_err(|e| format!("{e:?}"))?;
                let value = JsFuture::from(pm.get_subscription().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
                if !value.is_null() && !value.is_undefined() {
                    let sub: web_sys::PushSubscription = value.dyn_into().map_err(|e| format!("{e:?}"))?;
                    let endpoint = sub.endpoint();
                    JsFuture::from(sub.unsubscribe().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
                    api::push_unsubscribe(&endpoint).await?;
                }
                Ok(())
            }
            .await;
            set_push_busy.set(false);
            match result {
                Ok(()) => {
                    set_push_enabled.set(false);
                    set_push_msg.set(Some((
                        "success".to_string(),
                        "Notificaciones del navegador desactivadas.".to_string(),
                    )));
                }
                Err(e) => {
                    set_push_msg.set(Some((
                        "error".to_string(),
                        format!("No se pudo desactivar: {e}"),
                    )));
                }
            }
        });
    };

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        set_error_msg.set(None);
        set_info_msg.set(None);

        let cur = current.get();
        let newp = new_pass.get();
        let conf = confirm.get();

        if cur.is_empty() || newp.is_empty() {
            set_error_msg.set(Some("Completa todos los campos".to_string()));
            return;
        }
        if newp.len() < 8 {
            set_error_msg.set(Some(
                "La nueva contraseña debe tener al menos 8 caracteres".to_string(),
            ));
            return;
        }
        if newp != conf {
            set_error_msg.set(Some(
                "La confirmación no coincide con la nueva contraseña".to_string(),
            ));
            return;
        }
        if newp == cur {
            set_error_msg.set(Some(
                "La nueva contraseña debe ser distinta a la actual".to_string(),
            ));
            return;
        }

        set_saving.set(true);
        let nav = navigate.clone();
        let set_auth = set_is_auth;
        let c = cur.clone();
        let n = newp.clone();

        spawn_local(async move {
            match api::change_password(&c, &n).await {
                Ok(()) => {
                    crate::stores::clear_session();
                    if let Some(setter) = set_auth {
                        setter.set(false);
                    }
                    nav("/login", Default::default());
                }
                Err(e) => {
                    set_saving.set(false);
                    set_error_msg.set(Some(e));
                }
            }
        });
    };

    view! {
        <div class="max-w-2xl mx-auto">
            <div class="mb-6">
                <h1 class="text-2xl font-bold mb-1" style="color:var(--uci-text);">
                    <i class="fa-solid fa-user-gear mr-2" style="color:var(--uci-accent);"></i>"Mi Perfil"
                </h1>
                <p class="text-sm" style="color:var(--uci-muted);">
                    "Sesión, identidad y cambio de contraseña."
                </p>
            </div>

            <Show when=move || has_user fallback=|| ()>
                <div class="rounded-xl p-5 mb-6" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                    <div class="flex items-center gap-4">
                        <div class="w-14 h-14 rounded-full flex items-center justify-center text-xl font-black"
                            style="background:linear-gradient(135deg,#0EA5E9,#6366F1); color:white; flex-shrink:0;">
                            {u_initial.clone()}
                        </div>
                        <div>
                            <div class="text-lg font-bold" style="color:var(--uci-text);">{u_nombre.clone()}</div>
                            <div class="text-sm" style="color:var(--uci-muted);">"@"{u_username.clone()}</div>
                            <span class="inline-block mt-1 px-2 py-0.5 rounded-full text-xs font-bold"
                                style="background:rgba(14,165,233,0.12); color:#0EA5E9;">
                                {u_rol.clone()}
                            </span>
                        </div>
                    </div>
                </div>
            </Show>

            <div class="rounded-xl p-5" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <h2 class="text-base font-bold mb-4" style="color:var(--uci-text);">
                    <i class="fa-solid fa-key mr-2" style="color:var(--uci-accent);"></i>"Cambiar contraseña"
                </h2>

                {move || error_msg.get().map(|e| view! {
                    <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                        style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                        <i class="fa-solid fa-triangle-exclamation"></i>{e}
                    </div>
                })}

                {move || info_msg.get().map(|e| view! {
                    <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                        style="background:rgba(16,185,129,0.1); border:1px solid rgba(16,185,129,0.3); color:#059669;">
                        <i class="fa-solid fa-circle-check"></i>{e}
                    </div>
                })}

                <form on:submit=on_submit class="space-y-4">
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Contraseña actual"</label>
                        <input class="form-input" type="password" autocomplete="current-password"
                            placeholder="••••••••" required
                            prop:value=move || current.get()
                            on:input=move |ev| set_current.set(event_target_value(&ev)) />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Nueva contraseña"</label>
                        <input class="form-input" type="password" autocomplete="new-password"
                            placeholder="Mínimo 8 caracteres" required
                            prop:value=move || new_pass.get()
                            on:input=move |ev| set_new_pass.set(event_target_value(&ev)) />
                    </div>
                    <div>
                        <label class="block text-xs font-bold mb-1" style="color:var(--uci-muted);">"Confirmar nueva contraseña"</label>
                        <input class="form-input" type="password" autocomplete="new-password"
                            placeholder="Repite la nueva contraseña" required
                            prop:value=move || confirm.get()
                            on:input=move |ev| set_confirm.set(event_target_value(&ev)) />
                    </div>

                    <button type="submit" class="btn-primary w-full py-3 text-sm font-bold" disabled=saving>
                        {move || if saving.get() { "Guardando..." } else { "Cambiar contraseña" }}
                    </button>
                </form>

                <p class="mt-4 text-xs" style="color:var(--uci-muted);">
                    "Al cambiar la contraseña todas tus sesiones se cierran y deberás iniciar sesión de nuevo."
                </p>
            </div>

            <div class="rounded-xl p-5 mt-6" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <h2 class="text-base font-bold mb-1" style="color:var(--uci-text);">
                    <i class="fa-solid fa-shield-halved mr-2" style="color:var(--uci-accent);"></i>"Verificación en dos pasos"
                </h2>
                <p class="text-xs mb-4" style="color:var(--uci-muted);">
                    "Protege tu cuenta con un código temporal (TOTP) generado por una aplicación autenticadora."
                </p>

                {move || mfa_error.get().map(|e| view! {
                    <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                        style="background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;">
                        <i class="fa-solid fa-triangle-exclamation"></i>{e}
                    </div>
                })}
                {move || mfa_info.get().map(|e| view! {
                    <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                        style="background:rgba(16,185,129,0.1); border:1px solid rgba(16,185,129,0.3); color:#059669;">
                        <i class="fa-solid fa-circle-check"></i>{e}
                    </div>
                })}

                // Estado: MFA desactivado (o aún cargando) → flujo de activación
                <div class=move || if mfa_enabled.get() == Some(true) { "hidden" } else { "" }>
                    <button
                        type="button"
                        class=move || if mfa_setup.get().is_some() { "hidden" } else { "btn-primary w-full py-3 text-sm font-bold" }
                        disabled=mfa_busy
                        on:click=on_start_setup
                    >
                        {move || if mfa_busy.get() { "Generando..." } else { "Activar verificación en dos pasos" }}
                    </button>

                    <div class=move || if mfa_setup.get().is_some() { "space-y-4" } else { "hidden" }>
                        <div class="text-xs font-bold" style="color:var(--uci-muted);">
                            "1. Agrega esta cuenta en tu autenticadora (Google Authenticator, Authy, FreeOTP)."
                        </div>
                        <div class="p-3 rounded-lg font-mono text-sm break-all"
                            style="background:var(--uci-bg); border:1px solid var(--uci-border); color:var(--uci-text);">
                            {move || mfa_setup.get().map(|i| i.secret).unwrap_or_default()}
                        </div>
                        <p class="text-[10px] break-all" style="color:var(--uci-muted);">
                            {move || mfa_setup.get().map(|i| i.otpauth_uri).unwrap_or_default()}
                        </p>
                        <div class="text-xs font-bold" style="color:var(--uci-muted);">
                            "Códigos de respaldo (guárdalos en un lugar seguro; se muestran una sola vez):"
                        </div>
                        <div class="grid grid-cols-2 gap-1 font-mono text-xs" style="color:var(--uci-text);">
                            {move || mfa_setup.get().map(|i| i.backup_codes).unwrap_or_default()
                                .into_iter()
                                .map(|c| view! { <span class="p-1 rounded" style="background:var(--uci-bg);">{c}</span> })
                                .collect_view()}
                        </div>
                        <div class="text-xs font-bold" style="color:var(--uci-muted);">
                            "2. Ingresa el código de 6 dígitos para confirmar:"
                        </div>
                        <input
                            class="form-input text-center tracking-[0.4em] font-mono"
                            type="text"
                            inputmode="numeric"
                            autocomplete="one-time-code"
                            placeholder="000000"
                            prop:value=move || mfa_code.get()
                            on:input=move |ev| set_mfa_code.set(event_target_value(&ev))
                        />
                        <button type="button" class="btn-primary w-full py-3 text-sm font-bold"
                            disabled=mfa_busy on:click=on_confirm_mfa>
                            {move || if mfa_busy.get() { "Verificando..." } else { "Confirmar y activar" }}
                        </button>
                    </div>
                </div>

                // Estado: MFA activado → flujo de desactivación
                <div class=move || if mfa_enabled.get() == Some(true) { "space-y-4" } else { "hidden" }>
                    <span class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold"
                        style="background:rgba(16,185,129,0.12); color:#059669;">
                        <i class="fa-solid fa-lock"></i>"Activado"
                    </span>
                    <p class="text-xs" style="color:var(--uci-muted);">
                        "Para desactivarlo, ingresa un código válido de tu autenticadora."
                    </p>
                    <input
                        class="form-input text-center tracking-[0.4em] font-mono"
                        type="text"
                        inputmode="numeric"
                        autocomplete="one-time-code"
                        placeholder="000000"
                        prop:value=move || mfa_code.get()
                        on:input=move |ev| set_mfa_code.set(event_target_value(&ev))
                    />
                    <button type="button"
                        class="w-full py-3 text-sm font-bold rounded-lg"
                        style="background:rgba(239,68,68,0.12); color:#DC2626; border:1px solid rgba(239,68,68,0.3);"
                        disabled=mfa_busy on:click=on_disable_mfa>
                        {move || if mfa_busy.get() { "Desactivando..." } else { "Desactivar verificación en dos pasos" }}
                    </button>
                </div>
            </div>
        <div class="rounded-xl p-5 mt-6" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <h2 class="text-base font-bold mb-1" style="color:var(--uci-text);">
                    <i class="fa-solid fa-bell mr-2" style="color:var(--uci-accent);"></i>"Notificaciones del navegador"
                </h2>
                <p class="text-xs mb-4" style="color:var(--uci-muted);">
                    "Recibe alertas de escalamiento clínico aunque no tengas la app abierta. Las notificaciones no incluyen datos del paciente (solo tipo y nivel)."
                </p>

                {move || push_msg.get().map(|(kind, text)| view! {
                    <div class="p-3 rounded-lg mb-4 text-sm font-semibold flex items-center gap-2"
                        style=match kind.as_str() {
                            "success" => "background:rgba(16,185,129,0.1); border:1px solid rgba(16,185,129,0.3); color:#059669;",
                            _ => "background:rgba(239,68,68,0.1); border:1px solid rgba(239,68,68,0.3); color:#DC2626;",
                        }>
                        <i class=match kind.as_str() {
                            "success" => "fa-solid fa-circle-check",
                            _ => "fa-solid fa-triangle-exclamation",
                        }></i>{text}
                    </div>
                })}

                <div class="flex items-center gap-3">
                    <button type="button"
                        class=move || if push_enabled.get() { "hidden" } else { "btn-primary flex-1 py-3 text-sm font-bold" }
                        disabled=push_busy
                        on:click=on_enable_push>
                        {move || if push_busy.get() { "Configurando..." } else { "Activar notificaciones" }}
                    </button>
                    <button type="button"
                        class=move || if push_enabled.get() { "flex-1 py-3 text-sm font-bold rounded-lg" } else { "hidden" }
                        style="background:rgba(239,68,68,0.12); color:#DC2626; border:1px solid rgba(239,68,68,0.3);"
                        disabled=push_busy
                        on:click=on_disable_push>
                        {move || if push_busy.get() { "Desactivando..." } else { "Desactivar notificaciones" }}
                    </button>
                </div>
                <p class="mt-3 text-xs" style="color:var(--uci-muted);">
                    {move || if push_enabled.get() {
                        "Estado: activadas. Debes permitir las notificaciones en el navegador la primera vez."
                    } else {
                        "¿No ves el botón? Asegúrate de estar en una conexión segura (https) y de que el servidor tenga Web Push habilitado."
                    }}
                </p>
            </div>
        </div>
    }
}
