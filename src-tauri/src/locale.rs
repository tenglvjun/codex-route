use crate::state::LanguagePreference;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedLocale {
    ZhCn,
    ZhTw,
    En,
}

pub fn resolve_language(preference: LanguagePreference, system_locale: Option<&str>) -> ResolvedLocale {
    match preference {
        LanguagePreference::ZhCn => ResolvedLocale::ZhCn,
        LanguagePreference::ZhTw => ResolvedLocale::ZhTw,
        LanguagePreference::En => ResolvedLocale::En,
        LanguagePreference::System => system_locale
            .and_then(locale_family)
            .unwrap_or(ResolvedLocale::En),
    }
}

pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

fn locale_family(locale: &str) -> Option<ResolvedLocale> {
    let normalized = locale.trim().replace('_', "-").to_ascii_lowercase();
    let mut parts = normalized.split('-');
    let language = parts.next()?;
    if language != "zh" {
        return None;
    }
    match parts.next() {
        Some("tw" | "hk" | "mo") => Some(ResolvedLocale::ZhTw),
        Some("cn" | "sg" | "my") => Some(ResolvedLocale::ZhCn),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_language, ResolvedLocale};
    use crate::state::LanguagePreference;

    #[test]
    fn explicit_preferences_win_over_system_locale() {
        assert_eq!(
            resolve_language(LanguagePreference::ZhCn, Some("zh-TW")),
            ResolvedLocale::ZhCn
        );
        assert_eq!(
            resolve_language(LanguagePreference::ZhTw, Some("en-US")),
            ResolvedLocale::ZhTw
        );
        assert_eq!(
            resolve_language(LanguagePreference::En, Some("zh-CN")),
            ResolvedLocale::En
        );
    }

    #[test]
    fn system_locale_families_and_fallback_are_supported() {
        assert_eq!(
            resolve_language(LanguagePreference::System, Some("zh-CN")),
            ResolvedLocale::ZhCn
        );
        assert_eq!(
            resolve_language(LanguagePreference::System, Some("zh_SG")),
            ResolvedLocale::ZhCn
        );
        assert_eq!(
            resolve_language(LanguagePreference::System, Some("zh-HK")),
            ResolvedLocale::ZhTw
        );
        assert_eq!(
            resolve_language(LanguagePreference::System, Some("zh-MO")),
            ResolvedLocale::ZhTw
        );
        assert_eq!(
            resolve_language(LanguagePreference::System, Some("fr-FR")),
            ResolvedLocale::En
        );
        assert_eq!(
            resolve_language(LanguagePreference::System, None),
            ResolvedLocale::En
        );
    }
}
