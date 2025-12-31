use dioxus::prelude::*;

#[component]
pub fn ConnectedClients() -> Element {
    rsx! {
        div { class: "page-container",
            div { class: "page-header",
                h1 { "Connected Clients" }
            }
            div { class: "page-content",
                p { "View and manage currently connected clients to the Carbon server." }
            }
        }
    }
}
