use leptos::prelude::*;
use dmart_app::app::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
