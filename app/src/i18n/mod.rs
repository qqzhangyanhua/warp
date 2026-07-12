mod message;
mod table;

use settings::Setting as _;
use std::sync::atomic::{AtomicU8, Ordering};
use warpui::{AppContext, SingletonEntity};

pub use message::Message;

use crate::settings::{LocalePreference, LocalizationSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
    En,
    ZhCn,
}

pub fn active_locale(ctx: &AppContext) -> Locale {
    let locale = match LocalizationSettings::as_ref(ctx).locale_preference.value() {
        LocalePreference::System => system_locale(),
        LocalePreference::En => Locale::En,
        LocalePreference::ZhCn => Locale::ZhCn,
    };
    remember_locale(locale);
    locale
}

/// Resolve the OS-preferred locale into a supported Warp UI locale.
///
/// Unsupported languages fall back to English. Chinese variants that are not
/// explicitly traditional (`zh-TW` / `zh-HK` / `zh-Hant`, etc.) map to Simplified
/// Chinese so mainland/system defaults that report `zh` or `zh-CN` work.
pub fn system_locale() -> Locale {
    locale_from_system_tag(sys_locale::get_locale().as_deref())
}

pub(crate) fn locale_from_system_tag(tag: Option<&str>) -> Locale {
    let Some(tag) = tag.map(str::trim).filter(|tag| !tag.is_empty()) else {
        return Locale::En;
    };

    // BCP-47 / POSIX-style tags: `zh-CN`, `zh_CN.UTF-8`, `zh-Hans-CN`, …
    let normalized = tag.replace('_', "-").to_ascii_lowercase();
    let primary = normalized.split(['-', '.']).next().unwrap_or(&normalized);

    match primary {
        "zh" => {
            // Prefer traditional only when the tag is explicitly traditional.
            let is_traditional = normalized
                .split(['-', '.'])
                .any(|part| matches!(part, "tw" | "hk" | "mo" | "hant"));
            if is_traditional {
                Locale::En
            } else {
                Locale::ZhCn
            }
        }
        _ => Locale::En,
    }
}

pub fn tr(ctx: &AppContext, message: Message) -> &'static str {
    let locale = active_locale(ctx);
    remember_locale(locale);
    table::text(message, locale)
}

/// Translate using the most recently observed UI locale.
///
/// Prefer [`tr`] when an `AppContext` is available. This fallback is for shared
/// settings chrome helpers that are called deep in pure render paths without a
/// context handle. It is updated whenever [`tr`] / [`active_locale`] runs.
pub fn tr_cached(message: Message) -> &'static str {
    table::text(message, last_locale())
}

fn remember_locale(locale: Locale) {
    LAST_LOCALE.store(locale_to_u8(locale), Ordering::Relaxed);
}

fn last_locale() -> Locale {
    locale_from_u8(LAST_LOCALE.load(Ordering::Relaxed)).unwrap_or_else(system_locale)
}

fn locale_to_u8(locale: Locale) -> u8 {
    match locale {
        Locale::En => 0,
        Locale::ZhCn => 1,
    }
}

fn locale_from_u8(value: u8) -> Option<Locale> {
    match value {
        0 => Some(Locale::En),
        1 => Some(Locale::ZhCn),
        _ => None,
    }
}

static LAST_LOCALE: AtomicU8 = AtomicU8::new(u8::MAX);

#[cfg(test)]
mod tests {
    use super::{locale_from_system_tag, Locale};

    #[test]
    fn system_tag_maps_chinese_to_zh_cn() {
        assert_eq!(locale_from_system_tag(Some("zh-CN")), Locale::ZhCn);
        assert_eq!(locale_from_system_tag(Some("zh_CN.UTF-8")), Locale::ZhCn);
        assert_eq!(locale_from_system_tag(Some("zh-Hans-CN")), Locale::ZhCn);
        assert_eq!(locale_from_system_tag(Some("zh")), Locale::ZhCn);
    }

    #[test]
    fn system_tag_maps_explicit_traditional_chinese_to_english_fallback() {
        assert_eq!(locale_from_system_tag(Some("zh-TW")), Locale::En);
        assert_eq!(locale_from_system_tag(Some("zh-HK")), Locale::En);
        assert_eq!(locale_from_system_tag(Some("zh-Hant")), Locale::En);
    }

    #[test]
    fn system_tag_defaults_unknown_and_empty_to_english() {
        assert_eq!(locale_from_system_tag(None), Locale::En);
        assert_eq!(locale_from_system_tag(Some("")), Locale::En);
        assert_eq!(locale_from_system_tag(Some("en-US")), Locale::En);
        assert_eq!(locale_from_system_tag(Some("ja-JP")), Locale::En);
    }
}
