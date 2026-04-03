use crate::scanner::{SortMethod, SortOrder};
use egui_phosphor::regular as icons;

#[derive(Clone, Copy)]
pub struct SortOptionUi {
    pub method: SortMethod,
    pub icon: &'static str,
    pub label: &'static str,
}

pub const SORT_OPTIONS: [SortOptionUi; 5] = [
    SortOptionUi {
        method: SortMethod::Alphabetical,
        icon: icons::LIST_BULLETS,
        label: "Name (Alphabetical)",
    },
    SortOptionUi {
        method: SortMethod::Natural,
        icon: icons::LIST_NUMBERS,
        label: "Name (Natural)",
    },
    SortOptionUi {
        method: SortMethod::Size,
        icon: icons::FILE,
        label: "Size",
    },
    SortOptionUi {
        method: SortMethod::DateModified,
        icon: icons::CLOCK_COUNTER_CLOCKWISE,
        label: "Date Modified",
    },
    SortOptionUi {
        method: SortMethod::DateCreated,
        icon: icons::CALENDAR_PLUS,
        label: "Date Created",
    },
];

pub fn method_ui(method: SortMethod) -> SortOptionUi {
    SORT_OPTIONS
        .iter()
        .copied()
        .find(|option| option.method == method)
        .unwrap_or(SORT_OPTIONS[0])
}

pub fn topbar_method_label(method: SortMethod) -> String {
    let option = method_ui(method);
    format!("{} {}", option.icon, option.label)
}

pub fn popup_item_label(method: SortMethod) -> String {
    let option = method_ui(method);
    format!("{} {}", option.icon, option.label)
}

pub fn order_icon(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Ascending => icons::SORT_ASCENDING,
        SortOrder::Descending => icons::SORT_DESCENDING,
    }
}

pub fn order_tooltip(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Ascending => "Sort order: ascending",
        SortOrder::Descending => "Sort order: descending",
    }
}
