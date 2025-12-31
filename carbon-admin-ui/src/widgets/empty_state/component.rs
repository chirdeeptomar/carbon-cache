use dioxus::prelude::*;
use crate::components::button::Button;

#[component]
pub fn EmptyState(
    icon: String,
    title: String,
    description: String,
    action_label: Option<String>,
    on_action: Option<EventHandler<MouseEvent>>
) -> Element {
    rsx! {
        div { class: "empty-state",
            div { class: "empty-icon", "{icon}" }
            h3 { class: "empty-title", "{title}" }
            p { class: "empty-description", "{description}" }
            if let Some(label) = action_label {
                Button {
                    onclick: move |e| {
                        if let Some(handler) = &on_action {
                            handler.call(e);
                        }
                    },
                    "{label}"
                }
            }
        }
    }
}
