use leptos::prelude::*;
use leptos::either::Either;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
use gloo_storage::{LocalStorage, Storage};

#[component]
pub fn LoginPage() -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(false);
    let (loading, set_loading) = signal(false);
    
    let navigate = use_navigate();

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        set_loading.set(true);
        set_error.set(false);
        
        // Simular autenticación premium
        let u = username.get();
        let p = password.get();
        let nav = navigate.clone();
        
        spawn_local(async move {
            // Delay cosmético para efecto premium
            gloo_timers::future::TimeoutFuture::new(800).await;
            
            if !u.is_empty() && !p.is_empty() {
                let _ = LocalStorage::set("dmart_auth", "true");
                set_loading.set(false);
                nav("/", Default::default());
            } else {
                set_loading.set(false);
                set_error.set(true);
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
                    <div class="bg-uci-critical/10 border border-uci-critical/30 p-4 rounded-xl mb-6 text-uci-critical text-xs font-semibold animate-pulse">
                        "⚠ Credenciales inválidas. Por favor intente de nuevo."
                    </div>
                })}

                <form on:submit=on_submit class="space-y-6">
                    <div>
                        <label class="form-label">"Usuario / Identificación"</label>
                        <input 
                            type="text" 
                            class="form-input py-3" 
                            placeholder="admin_uci"
                            prop:value=username
                            on:input=move |ev| set_username.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    <div>
                        <div class="flex justify-between items-center mb-2">
                            <label class="form-label mb-0">"Contraseña"</label>
                            <a href="#" class="text-[10px] text-uci-accent hover:underline">"¿Olvido su clave?"</a>
                        </div>
                        <input 
                            type="password" 
                            class="form-input py-3" 
                            placeholder="••••••••"
                            prop:value=password
                            on:input=move |ev| set_password.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    <button 
                        type="submit" 
                        class="btn-primary w-full py-4 text-base font-bold tracking-wide mt-4 relative overflow-hidden group"
                        disabled=loading
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

                <div class="mt-8 pt-8 border-t border-uci-border/50 text-center">
                    <p class="text-[10px] text-uci-muted uppercase tracking-widest font-bold">
                        "V.1.9 — Acceso Restringido"
                    </p>
                </div>
            </div>
        </div>
    }
}
