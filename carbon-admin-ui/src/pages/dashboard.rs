use dioxus::prelude::*;

#[component]
pub fn Dashboard() -> Element {
    rsx! {
        div { class: "dashboard",
            h1 { "Dashboard" }
            p { "Welcome! You are logged in." }
        }
    }
}
