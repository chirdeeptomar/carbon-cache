use dioxus::prelude::*;
use crate::widgets::tabs::{Tabs, TabItem};
use crate::widgets::empty_state::EmptyState;

#[component]
pub fn DataContainer() -> Element {
    let mut active_tab = use_signal(|| "caches".to_string());

    let tabs = vec![
        TabItem {
            id: "caches".to_string(),
            label: "Caches".to_string(),
            count: 0,
        },
        TabItem {
            id: "counters".to_string(),
            label: "Counters".to_string(),
            count: 0,
        },
        TabItem {
            id: "tasks".to_string(),
            label: "Tasks".to_string(),
            count: 0,
        },
        TabItem {
            id: "schemas".to_string(),
            label: "Schemas".to_string(),
            count: 0,
        },
    ];

    rsx! {
        div { class: "page-container",
            div { class: "page-header",
                h1 { "Data container" }
                div { class: "status-indicators",
                    span { class: "status-badge running", "✓ Running" }
                }
            }

            Tabs {
                items: tabs,
                active_tab: active_tab(),
                on_tab_change: move |tab_id| active_tab.set(tab_id)
            }

            div { class: "tab-content",
                match active_tab().as_str() {
                    "caches" => rsx! {
                        EmptyState {
                            icon: "🗄️".to_string(),
                            title: "No caches".to_string(),
                            description: "Click \"Create a cache\" and provide configuration in XML, JSON, or YAML format or use a custom template. You can also create caches from the CLI and remote clients.".to_string(),
                            action_label: Some("Create a cache".to_string()),
                            on_action: Some(EventHandler::new(move |_| {
                                // Placeholder action
                                info!("Create cache clicked");
                            }))
                        }
                    },
                    "counters" => rsx! {
                        EmptyState {
                            icon: "🔢".to_string(),
                            title: "No counters".to_string(),
                            description: "Counters are distributed, cluster-wide data structures for incrementing and decrementing values.".to_string(),
                            action_label: Some("Create a counter".to_string()),
                            on_action: Some(EventHandler::new(move |_| {
                                info!("Create counter clicked");
                            }))
                        }
                    },
                    "tasks" => rsx! {
                        EmptyState {
                            icon: "📋".to_string(),
                            title: "No tasks".to_string(),
                            description: "Tasks allow you to execute code across the cluster or on specific nodes.".to_string(),
                            action_label: None,
                            on_action: None
                        }
                    },
                    "schemas" => rsx! {
                        EmptyState {
                            icon: "📄".to_string(),
                            title: "No schemas".to_string(),
                            description: "Protobuf schemas define the structure of your cached data for efficient serialization.".to_string(),
                            action_label: Some("Upload schema".to_string()),
                            on_action: Some(EventHandler::new(move |_| {
                                info!("Upload schema clicked");
                            }))
                        }
                    },
                    _ => rsx! { div {} }
                }
            }
        }
    }
}
