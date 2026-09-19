// Toast / Snackbar component for dMart UCI
// Auto-dismiss notifications with contextual types

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use std::sync::Mutex;
use std::collections::VecDeque;
use rand::Rng;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Toast {
    pub id: u32,
    pub kind: ToastKind,
    pub message: String,
    pub persistent: bool, // If true, doesn't auto-dismiss
}

impl Toast {
    pub fn success(msg: impl Into<String>) -> Self {
        Self { id: rand::thread_rng().r#gen(), kind: ToastKind::Success, message: msg.into(), persistent: false }
    }
    
    pub fn error(msg: impl Into<String>) -> Self {
        Self { id: rand::thread_rng().r#gen(), kind: ToastKind::Error, message: msg.into(), persistent: false }
    }
    
    pub fn warning(msg: impl Into<String>) -> Self {
        Self { id: rand::thread_rng().r#gen(), kind: ToastKind::Warning, message: msg.into(), persistent: false }
    }
    
    pub fn info(msg: impl Into<String>) -> Self {
        Self { id: rand::thread_rng().r#gen(), kind: ToastKind::Info, message: msg.into(), persistent: false }
    }
    
    pub fn persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }
}

/// Global toast queue (thread-safe for WASM single-threaded context)
static TOAST_QUEUE: Mutex<VecDeque<Toast>> = Mutex::new(VecDeque::new());

/// Add a toast to the global queue
pub fn push_toast(toast: Toast) {
    if let Ok(mut queue) = TOAST_QUEUE.lock() {
        queue.push_back(toast);
    }
}

/// Convenience functions for common toast types
pub fn toast_success(msg: impl Into<String>) {
    push_toast(Toast::success(msg));
}

pub fn toast_error(msg: impl Into<String>) {
    push_toast(Toast::error(msg).persistent(true)); // Errors persist until dismissed
}

pub fn toast_warning(msg: impl Into<String>) {
    push_toast(Toast::warning(msg));
}

pub fn toast_info(msg: impl Into<String>) {
    push_toast(Toast::info(msg));
}

/// Remove a toast by ID
pub fn remove_toast(id: u32) {
    if let Ok(mut queue) = TOAST_QUEUE.lock() {
        queue.retain(|t| t.id != id);
    }
}

/// Clear all toasts
pub fn clear_toasts() {
    if let Ok(mut queue) = TOAST_QUEUE.lock() {
        queue.clear();
    }
}

/// Get current toasts (for rendering)
pub fn get_toasts() -> Vec<Toast> {
    TOAST_QUEUE.lock().map(|q| q.iter().cloned().collect()).unwrap_or_default()
}

/// Toast container component - place at root of app
#[component]
pub fn ToastContainer() -> impl IntoView {
    let (toasts, set_toasts) = signal(Vec::<Toast>::new());
    
    // Poll for new toasts
    Effect::new(move |_| {
        let current = get_toasts();
        set_toasts.set(current);
    });
    
    // Poll interval for new toasts
    let _timer = set_interval_with_handle(move || {
        let current = get_toasts();
        set_toasts.set(current);
    }, std::time::Duration::from_millis(300));
    
    view! {
        <div class="toast-container" role="status" aria-live="polite" aria-atomic="true">
            <For
                each=move || toasts.get()
                key=|t| t.id
                children=move |toast| {
                    let id = toast.id;
                    let kind_class = match toast.kind {
                        ToastKind::Success => "toast-success",
                        ToastKind::Error => "toast-error",
                        ToastKind::Warning => "toast-warning",
                        ToastKind::Info => "toast-info",
                    };
                    
                    let icon = match toast.kind {
                        ToastKind::Success => "fa-check-circle",
                        ToastKind::Error => "fa-times-circle",
                        ToastKind::Warning => "fa-exclamation-triangle",
                        ToastKind::Info => "fa-info-circle",
                    };
                    
                    view! {
                        <div class=format!("toast {}", kind_class) role="alert" aria-live="assertive">
                            <i class=format!("fa-solid {} toast-icon", icon) aria-hidden="true"></i>
                            <span class="toast-message">{toast.message}</span>
                            {if !toast.persistent {
                                view! { <button class="toast-close" on:click=move |_| remove_toast(id) aria-label="Cerrar"><i class="fa-solid fa-times"></i></button> }.into_any()
                            } else {
                                view! { }.into_any()
                            }}
                        </div>
                    }
                }
            />
        </div>
    }
}

/// Auto-clear toasts after duration (for non-persistent)
pub fn auto_dismiss(id: u32, duration_ms: u64) {
    use gloo_timers::future::TimeoutFuture;
    spawn_local(async move {
        TimeoutFuture::new(duration_ms as u32).await;
        remove_toast(id);
    });
}

/// Hook to use toasts in components
pub fn use_toasts() -> impl Fn(Toast) + Clone {
    let push = move |toast: Toast| push_toast(toast);
    push
}