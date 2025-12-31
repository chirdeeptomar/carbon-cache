use std::time::Duration;

use dioxus::prelude::*;

use crate::Route;
use crate::api::ApiClient;
use crate::components::{button::Button, input::Input};
use dioxus_primitives::toast::{ToastOptions, use_toast};
use gloo_timers::future::TimeoutFuture;

const HEADER_SVG: Asset = asset!("/assets/header.svg");

#[component]
pub fn Login() -> Element {
    let mut username: Signal<String> = use_signal(|| "".to_string());
    let mut password: Signal<String> = use_signal(|| "".to_string());
    let toaster = use_toast();
    let nav = navigator();

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
                            onclick: move |_| {
                                let username_val = username();
                                let password_val = password();

                                spawn(async move {
                                    let login_result = use_context::<ApiClient>()
                                        .login(&username_val, &password_val)
                                        .await;

                                    match login_result {
                                        Result::Ok(response) => {
                                            // Show success toast for 1 second
                                            let success_options = ToastOptions::new()
                                                .duration(Duration::from_secs(1))
                                                .permanent(false);

                                            toaster
                                                .success(
                                                    format!(
                                                        "Login successful! Welcome back, {}",
                                                        response.username,
                                                    ),
                                                    success_options,
                                                );
                                            TimeoutFuture::new(1000).await;
                                            nav.push(Route::DataContainer {});
                                        }
                                        Result::Err(_) => {
                                            let error_options = ToastOptions::new().permanent(true);
                                            toaster
                                                .error(
                                                    "Login failed. Invalid username or password.".to_string(),
                                                    error_options,
                                                );
                                        }
                                    }
                                });
                            },
                            r#type: "button",
                            "Login"
                        }
                    }
                }
            }
        }
    }
}
