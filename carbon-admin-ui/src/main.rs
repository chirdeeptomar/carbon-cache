use dioxus::prelude::*;

mod components;
use components::{button::Button, input::Input};

use crate::api::ApiClient;

mod api;
mod config;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const DX_THEME_CSS: Asset = asset!("/assets/dx-components-theme.css");

fn main() {
    // Load environment variables from .env file (if exists)
    match dotenvy::dotenv() {
        Ok(_) => info!("Loaded environment variables from .env file"),
        Err(_) => info!("No .env file found, using system environment variables"),
    }

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let api_client = api::ApiClient::new();
    use_context_provider(|| api_client);

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: DX_THEME_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {
        }
    }
}

#[component]
pub fn Login() -> Element {
    let mut username: Signal<String> = use_signal(|| "".to_string());
    let mut password: Signal<String> = use_signal(|| "".to_string());

    rsx! {
        div { id: "login",
            div { class: "login-card",
                img { src: HEADER_SVG, alt: "Logo", class: "logo" }
                h1 { "Welcome to Carbon Admin" }

                form { class: "login-form",
                    div { class: "form-group",
                        label { r#for: "username",
                            "Username"
                            span { class: "required", "*" }
                        }
                        Input {
                            id: "username",
                            r#type: "text",
                            name: "username",
                            placeholder: "Default or your username",
                            required: true,
                            oninput: move |e: Event<FormData>| username.set(e.value()),
                        }
                    }

                    div { class: "form-group",
                        label { r#for: "password",
                            "Password"
                            span { class: "required", "*" }
                        }
                        Input {
                            id: "password",
                            r#type: "password",
                            name: "password",
                            placeholder: "Enter password",
                            required: true,
                            oninput: move |e: Event<FormData>| password.set(e.value()),
                        }
                    }

                    div { class: "checkbox-group",
                        input {
                            r#type: "checkbox",
                            id: "remember-me",
                            name: "remember-me",
                        }
                        label { r#for: "remember-me", "Remember me" }
                    }

                    div { align_content: "center",
                        Button {
                            onclick: move |_| async move {
                                info!("Logging in...");
                                info!("Username: {}", username());
                                info!("Password: {}", "*".repeat(password().len()));
                            },
                            r#type: "submit",
                            "Login"
                        }
                    }
                }
            }
        }
    }
}

/// Home page
#[component]
fn Home() -> Element {
    rsx! {
        ServerStatus {}
        Login {}
    }
}

#[component]
fn ServerStatus() -> Element {
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

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
