use dioxus::prelude::*;

mod components;
use components::toast::ToastProvider;

mod api;
mod config;
mod pages;
mod widgets;

use pages::dashboard::Dashboard;
use pages::login::Login;
use widgets::server_status::ServerStatus;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/dashboard")]
    Dashboard {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
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

        ToastProvider { Router::<Route> {} }
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

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
