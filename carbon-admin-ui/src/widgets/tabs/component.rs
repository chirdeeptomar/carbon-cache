use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct TabItem {
    pub id: String,
    pub label: String,
    pub count: u32,
}

#[component]
pub fn Tabs(items: Vec<TabItem>, active_tab: String, on_tab_change: EventHandler<String>) -> Element {
    rsx! {
        div { class: "tabs",
            for item in items {
                button {
                    class: if item.id == active_tab { "tab active" } else { "tab" },
                    onclick: move |_| {
                        let id = item.id.clone();
                        on_tab_change.call(id);
                    },
                    "{item.label}"
                    span { class: "tab-count", " {item.count}" }
                }
            }
        }
    }
}
