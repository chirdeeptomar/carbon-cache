use dioxus::prelude::*;

use crate::api::ApiClient;

#[component]
pub fn ServerStatus() -> Element {
    let health_resource = use_resource(move || async move {
        use_context::<ApiClient>()
            .check_health()
            .await
            .map(|json| json["message"].as_str().unwrap_or("Unknown").to_string())
            .unwrap_or_else(|_| "Down".to_string())
    });

    rsx! {
        div { class: "server-status",
            match health_resource.read().as_ref() {
                Some(status) => {
                    let is_error = status.eq("Down");
                    let dot_class = if is_error {
                        "status-dot error"
                    } else {
                        "status-dot healthy"
                    };
                    rsx! {
                        span { class: "{dot_class}" }
                        span { "Server Status: {status}" }
                    }
                }
                None => rsx! {
                    span { class: "status-dot loading" }
                    span { "Server Status: Checking..." }
                },
            }
        }
    }
}
