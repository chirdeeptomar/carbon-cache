use dioxus::prelude::*;

use crate::widgets::server_status::ServerStatus;

#[component]
pub fn Dashboard() -> Element {
    rsx! {
        ServerStatus {}
        div { class: "dashboard",
            h1 { "Dashboard" }
            p { "Welcome! You are logged in." }
        }
    }
}
