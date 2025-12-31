use dioxus::prelude::*;

#[component]
pub fn ClusterMembership() -> Element {
    rsx! {
        div { class: "page-container",
            div { class: "page-header",
                h1 { "Cluster Membership" }
            }
            div { class: "page-content",
                p { "View and manage cluster nodes and membership information." }
            }
        }
    }
}
