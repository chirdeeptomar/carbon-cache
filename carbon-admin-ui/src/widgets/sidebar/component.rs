use crate::widgets::server_status::ServerStatus;
use dioxus::prelude::*;

#[component]
pub fn Sidebar() -> Element {
    let route = use_route::<crate::Route>();
    let current_path = route.to_string();

    rsx! {
        aside { class: "sidebar",
            div { class: "sidebar-header",
                h2 { "Carbon Admin" }
            }
            nav { class: "sidebar-nav",
                Link {
                    to: crate::Route::DataContainer {},
                    class: if current_path.contains("data-container") { "nav-item active" } else { "nav-item" },
                    "Data Container"
                }
                Link {
                    to: crate::Route::AccessManagement {},
                    class: if current_path.contains("access") { "nav-item active" } else { "nav-item" },
                    "Access Management"
                }
                Link {
                    to: crate::Route::ConnectedClients {},
                    class: if current_path.contains("clients") { "nav-item active" } else { "nav-item" },
                    "Connected Clients"
                }
                Link {
                    to: crate::Route::Statistics {},
                    class: if current_path.contains("statistics") { "nav-item active" } else { "nav-item" },
                    "Carbon Statistics"
                }
                Link {
                    to: crate::Route::ClusterMembership {},
                    class: if current_path.contains("cluster") { "nav-item active" } else { "nav-item" },
                    "Cluster Membership"
                }
            }
            div { class: "sidebar-footer",
                ServerStatus {}
            }
        }
    }
}
