use dioxus::prelude::*;

mod components;
use components::{button::Button, input::Input};
use dioxus_primitives::label::Label;

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
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: DX_THEME_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn Login() -> Element {
    rsx! {
        div { id: "login",
            h1 { "Welcome to Carbon Admin" }
            img { src: HEADER_SVG, alt: "Logo" }
            form {
                Label { html_for: "username", "Username" }
                Input {
                    id: "username",
                    r#type: "text",
                    name: "username",
                    placeholder: "Username here",
                    required: true,
                }
                br {}
                Label { html_for: "password", "Password" }
                Input {
                    id: "password",
                    r#type: "password",
                    name: "password",
                    placeholder: "Password here",
                    required: true,
                }
                br {}
                Button {
                    onclick: move |_| {
                        info!("Login button clicked again");
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
