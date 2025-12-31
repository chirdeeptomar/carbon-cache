use crate::components::select::{
    Select, SelectGroup, SelectItemIndicator, SelectList, SelectOption, SelectTrigger, SelectValue,
};
use dioxus::prelude::*;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumCount, EnumIter};

#[derive(Debug, Clone, Copy, PartialEq, EnumCount, EnumIter, Display)]
enum AdminAction {
    #[strum(to_string = "admin")]
    Admin,
    Logout,
}

#[component]
pub fn Header() -> Element {
    let mut selected_value = use_signal(|| Option::<Option<AdminAction>>::None);
    let nav = navigator();

    // Handle logout when Logout option is selected
    use_effect(move || {
        if let Some(Some(AdminAction::Logout)) = selected_value() {
            info!("User logged out");
            // Reset selection
            selected_value.set(None);
            // Navigate to login page
            nav.push(crate::Route::Home {});
        }
    });

    let actions = AdminAction::iter().enumerate().map(|(i, action)| {
        rsx! {
            SelectOption::<AdminAction> { index: i, value: action, text_value: "{action}",
                {format!("{action}")}
                SelectItemIndicator {}
            }
        }
    });

    rsx! {
        header { class: "app-header",
            h1 { class: "header-title", "Server Management Console" }
            div { class: "header-actions",
                Select::<AdminAction> {
                    placeholder: "admin",
                    value: selected_value,
                    on_value_change: move |value: Option<AdminAction>| {
                        selected_value.set(Some(value));
                    },
                    SelectTrigger { aria_label: "Admin Menu", SelectValue {} }
                    SelectList { aria_label: "Admin Actions",
                        SelectGroup { {actions} }
                    }
                }
            }
        }
    }
}
