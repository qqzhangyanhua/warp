mod message;
mod table;

use settings::Setting as _;
use warpui::{AppContext, SingletonEntity};

pub use message::Message;

use crate::settings::{LocalePreference, LocalizationSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
    En,
    ZhCn,
}

pub fn active_locale(ctx: &AppContext) -> Locale {
    match LocalizationSettings::as_ref(ctx).locale_preference.value() {
        LocalePreference::System | LocalePreference::En => Locale::En,
        LocalePreference::ZhCn => Locale::ZhCn,
    }
}

pub fn tr(ctx: &AppContext, message: Message) -> &'static str {
    table::text(message, active_locale(ctx))
}
