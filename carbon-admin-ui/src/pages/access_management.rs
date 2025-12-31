use dioxus::prelude::*;

#[component]
pub fn AccessManagement() -> Element {
    rsx! {
        div { class: "page-container",
            div { class: "page-header",
                h1 { "Access Management" }
            }
            div { class: "page-content",
                p { "Manage users, roles, and permissions for the Carbon server." }
            }
        }
    }
}
