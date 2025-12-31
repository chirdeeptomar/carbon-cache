use dioxus::prelude::*;

#[component]
pub fn Statistics() -> Element {
    rsx! {
        div { class: "page-container",
            div { class: "page-header",
                h1 { "Carbon Statistics" }
            }
            div { class: "page-content",
                p { "View carbon server statistics and metrics." }
            }
        }
    }
}
