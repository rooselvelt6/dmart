use crate::api;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use leptos_router::hooks::*;

#[component]
pub fn LoginPage() -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(false);
    let (loading, set_loading) = signal(false);

    // Reto MFA (segundo factor)
    let (challenge, set_challenge) = signal(String::new());
    let (code, set_code) = signal(String::new());
    let (use_backup, set_use_backup) = signal(false);

    let navigate = use_navigate();
    let navigate_submit = navigate.clone();
    let navigate_mfa = navigate.clone();
    let set_is_auth = use_context::<WriteSignal<bool>>();

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        set_loading.set(true);
        set_error.set(false);

        let u = username.get();
        let p = password.get();
        let nav = navigate_submit.clone();
        let set_auth = set_is_auth;

        spawn_local(async move {
            match api::login(&u, &p).await {
                Ok(response) if response.mfa_required => {
                    // Primer factor correcto: el token es de reto, aún sin sesión.
                    set_challenge.set(response.token);
                    set_code.set(String::new());
                    set_loading.set(false);
                }
                Ok(response) => {
                    crate::stores::session::save_session(&response);
                    if let Some(setter) = set_auth {
                        setter.set(true);
                    }
                    set_loading.set(false);
                    nav("/", Default::default());
                }
                Err(_) => {
                    set_loading.set(false);
                    set_error.set(true);
                }
            }
        });
    };

    let on_mfa_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        set_loading.set(true);
        set_error.set(false);

        let token = challenge.get();
        let c = code.get();
        let backup = use_backup.get();
        let nav = navigate_mfa.clone();
        let set_auth = set_is_auth;

        spawn_local(async move {
            let (code_arg, backup_arg) = if backup {
                ("", Some(c.as_str()))
            } else {
                (c.as_str(), None)
            };
            match api::mfa_verify(&token, code_arg, backup_arg).await {
                Ok(response) => {
                    crate::stores::session::save_session(&response);
                    if let Some(setter) = set_auth {
                        setter.set(true);
                    }
                    set_loading.set(false);
                    nav("/", Default::default());
                }
                Err(_) => {
                    set_loading.set(false);
                    set_error.set(true);
                }
            }
        });
    };

    view! {
        <div class="min-h-screen flex items-center justify-center p-6 bg-uci-bg relative overflow-hidden">
            // Fondo decorativo con gradientes médicos
            <div class="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-uci-accent/10 blur-[120px] rounded-full"></div>
            <div class="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-uci-accent2/10 blur-[120px] rounded-full"></div>

            <div class="glass-card w-full max-w-md p-10 relative z-10 animate-fade-in">
                <div class="text-center mb-10">
                    <div class="w-20 h-20 bg-gradient-to-br from-[#e34a27] to-[#9b2a14] rounded-full mx-auto flex items-center justify-center shadow-lg shadow-[#e34a27]/40 mb-6 group transition-transform hover:scale-105 duration-300">
                        <span class="text-4xl text-white font-black group-hover:rotate-12 transition-transform">"D"</span>
                    </div>
                    <h1 class="text-5xl font-extrabold tracking-widest">
                        <span class="text-white drop-shadow-md">"D"</span>
                        <span class="text-[#e34a27] drop-shadow-md">"MART"</span>
                    </h1>
                </div>

                {move || error.get().then(|| view! {
                    <div role="alert" class="bg-uci-critical/10 border border-uci-critical/30 p-4 rounded-xl mb-6 text-uci-critical text-xs font-semibold">
                        {move || if challenge.get().is_empty() {
                            "⚠ Credenciales inválidas. Por favor intente de nuevo."
                        } else {
                            "⚠ Código inválido. Verifique e intente de nuevo."
                        }}
                    </div>
                })}

                // Formulario de credenciales (primer factor)
                <form
                    on:submit=on_submit
                    class=move || if challenge.get().is_empty() { "space-y-6" } else { "hidden" }
                >
                    <div>
                        <label for="login-username" class="form-label">"Usuario / Identificación"</label>
                        <input
                            id="login-username"
                            type="text"
                            class="form-input py-3 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-blue-500"
                            placeholder="admin_uci"
                            autocomplete="username"
                            prop:value=username
                            on:input=move |ev| set_username.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    <div>
                        <div class="flex justify-between items-center mb-2">
                            <label for="login-password" class="form-label mb-0">"Contraseña"</label>
                            <a href="#" class="text-[10px] text-uci-accent hover:underline">"¿Olvido su clave?"</a>
                        </div>
                        <input
                            id="login-password"
                            type="password"
                            class="form-input py-3 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-blue-500"
                            placeholder="••••••••"
                            autocomplete="current-password"
                            prop:value=password
                            on:input=move |ev| set_password.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    <button
                        type="submit"
                        class="btn-primary w-full py-4 text-base font-bold tracking-wide mt-4 relative overflow-hidden group focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500"
                        disabled=loading
                        aria-busy=loading
                    >
                        <span class=move || if loading.get() { "opacity-0" } else { "opacity-100" }>
                            "Iniciar Sesión"
                        </span>
                        {move || loading.get().then(|| view! {
                            <div class="absolute inset-0 flex items-center justify-center">
                                <div class="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
                            </div>
                        })}
                    </button>
                </form>

                // Formulario del segundo factor (TOTP / código de respaldo)
                <form
                    on:submit=on_mfa_submit
                    class=move || if challenge.get().is_empty() { "hidden" } else { "space-y-6" }
                >
                    <div class="text-center">
                        <h2 class="text-lg font-bold text-white">"Verificación en dos pasos"</h2>
                        <p class="text-xs text-uci-muted mt-1">
                            {move || if use_backup.get() {
                                "Ingresa uno de tus códigos de respaldo."
                            } else {
                                "Ingresa el código de 6 dígitos de tu aplicación autenticadora."
                            }}
                        </p>
                    </div>
                    <div>
                        <label for="login-mfa-code" class="form-label">
                            {move || if use_backup.get() { "Código de respaldo" } else { "Código de verificación" }}
                        </label>
                        <input
                            id="login-mfa-code"
                            type="text"
                            inputmode=move || if use_backup.get() { "text" } else { "numeric" }
                            autocomplete="one-time-code"
                            class="form-input py-3 text-center tracking-[0.4em] font-mono focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-blue-500"
                            placeholder=move || if use_backup.get() { "XXXXXXXX" } else { "000000" }
                            prop:value=code
                            on:input=move |ev| set_code.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    <button
                        type="submit"
                        class="btn-primary w-full py-4 text-base font-bold tracking-wide relative overflow-hidden group focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500"
                        disabled=loading
                        aria-busy=loading
                    >
                        <span class=move || if loading.get() { "opacity-0" } else { "opacity-100" }>
                            "Verificar"
                        </span>
                        {move || loading.get().then(|| view! {
                            <div class="absolute inset-0 flex items-center justify-center">
                                <div class="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
                            </div>
                        })}
                    </button>

                    <div class="flex items-center justify-between text-[10px]">
                        <button
                            type="button"
                            class="text-uci-accent hover:underline"
                            on:click=move |_| set_use_backup.update(|b| *b = !*b)
                        >
                            {move || if use_backup.get() { "Usar código TOTP" } else { "Usar código de respaldo" }}
                        </button>
                        <button
                            type="button"
                            class="text-uci-muted hover:underline"
                            on:click=move |_| {
                                set_challenge.set(String::new());
                                set_error.set(false);
                            }
                        >
                            "Volver"
                        </button>
                    </div>
                </form>

                <div class="mt-8 pt-8 border-t border-uci-border/50 text-center">
                    <p class="text-[10px] text-uci-muted uppercase tracking-widest font-bold">
                        "V.1.9 — Acceso Restringido"
                    </p>
                </div>
            </div>
        </div>
    }
}
