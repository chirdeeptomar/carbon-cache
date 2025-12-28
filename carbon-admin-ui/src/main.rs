use dioxus::prelude::*;

mod components;
use components::{button::Button, input::Input};
use dioxus_primitives::label::Label;

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

    let health_resource = use_resource(move || async move {
        use_context::<ApiClient>()
            .check_health()
            .await
            .map(|json| json.to_string())
            .unwrap_or_else(|err| format!("Error: {}", err))
    });

    rsx! {
        div { id: "login",
            h1 { "Welcome to Carbon Admin" }
            h2 {
                "Server Status: "
                match health_resource.read().as_ref() {
                    Some(health) => rsx! {
                    "{health}"
                    },
                    None => rsx! { "Loading..." },
                }
            }
            img { src: HEADER_SVG, alt: "Logo" }
            form {
                Label { html_for: "username", "Username" }
                Input {
                    id: "username",
                    r#type: "text",
                    name: "username",
                    placeholder: "Username here",
                    required: true,
                    oninput: move |e: Event<FormData>| username.set(e.value()),
                }
                br {}
                Label { html_for: "password", "Password" }
                Input {
                    id: "password",
                    r#type: "password",
                    name: "password",
                    placeholder: "Password here",
                    required: true,
                    oninput: move |e: Event<FormData>| password.set(e.value()),
                }
                br {}
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

/// Home page
#[component]
fn Home() -> Element {
    rsx! {
        Login {}
    }
}

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
