#[cfg(any(target_os = "windows", test))]
use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};
#[cfg(any(target_os = "windows", test))]
use once_cell::sync::Lazy;

// Automatic triggering must preserve "unknown", instead of the English fallback
// used by the manual translation command. Cache the detector across selections.
#[cfg(any(target_os = "windows", test))]
static AUTO_DETECTOR: Lazy<LanguageDetector> = Lazy::new(|| {
    LanguageDetectorBuilder::from_languages(&[
        Language::Chinese,
        Language::Japanese,
        Language::English,
        Language::Korean,
        Language::French,
        Language::Spanish,
        Language::German,
        Language::Russian,
        Language::Italian,
        Language::Portuguese,
        Language::Turkish,
        Language::Arabic,
        Language::Vietnamese,
        Language::Thai,
        Language::Indonesian,
        Language::Malay,
        Language::Hindi,
        Language::Mongolian,
        Language::Bokmal,
        Language::Nynorsk,
        Language::Persian,
        Language::Ukrainian,
    ])
    .with_minimum_relative_distance(0.1)
    .build()
});

#[cfg(any(target_os = "windows", test))]
pub fn should_auto_translate(text: &str) -> bool {
    if !text.chars().any(char::is_alphabetic) {
        return false;
    }
    matches!(AUTO_DETECTOR.detect_language_of(text), Some(language) if language != Language::Chinese)
}

pub fn init_lang_detect() {
    // https://crates.io/crates/lingua
    use lingua::{Language, LanguageDetectorBuilder};
    let languages = vec![
        Language::Chinese,
        Language::Japanese,
        Language::English,
        Language::Korean,
        Language::French,
        Language::Spanish,
        Language::German,
        Language::Russian,
        Language::Italian,
        Language::Portuguese,
        Language::Turkish,
        Language::Arabic,
        Language::Vietnamese,
        Language::Thai,
        Language::Indonesian,
        Language::Malay,
        Language::Hindi,
        Language::Mongolian,
        Language::Bokmal,
        Language::Nynorsk,
        Language::Persian,
        Language::Ukrainian,
    ];
    let detector = LanguageDetectorBuilder::from_languages(&languages).build();
    let _ = detector.detect_language_of("Hello Language");
}

#[cfg(test)]
mod tests {
    use super::should_auto_translate;

    #[test]
    fn skips_empty_numeric_and_punctuation_selections() {
        for text in ["", "  \n\t", "123.45", "!?—", "😀"] {
            assert!(!should_auto_translate(text), "{text:?}");
        }
    }

    #[test]
    fn skips_both_simplified_and_traditional_chinese() {
        for text in ["这是一个中文句子。", "這是一個中文句子。", "中文"] {
            assert!(!should_auto_translate(text), "{text:?}");
        }
    }

    #[test]
    fn translates_recognizable_foreign_text() {
        for text in [
            "This is an English sentence.",
            "これは日本語の文章です。",
            "안녕하세요",
        ] {
            assert!(should_auto_translate(text), "{text:?}");
        }
    }
}
#[tauri::command]
pub fn lang_detect(text: &str) -> Result<&str, ()> {
    use lingua::{Language, LanguageDetectorBuilder};
    let languages = vec![
        Language::Chinese,
        Language::Japanese,
        Language::English,
        Language::Korean,
        Language::French,
        Language::Spanish,
        Language::German,
        Language::Russian,
        Language::Italian,
        Language::Portuguese,
        Language::Turkish,
        Language::Arabic,
        Language::Vietnamese,
        Language::Thai,
        Language::Indonesian,
        Language::Malay,
        Language::Hindi,
        Language::Mongolian,
        Language::Bokmal,
        Language::Nynorsk,
        Language::Persian,
    ];
    let detector = LanguageDetectorBuilder::from_languages(&languages).build();
    if let Some(lang) = detector.detect_language_of(text) {
        match lang {
            Language::Chinese => Ok("zh_cn"),
            Language::Japanese => Ok("ja"),
            Language::English => Ok("en"),
            Language::Korean => Ok("ko"),
            Language::French => Ok("fr"),
            Language::Spanish => Ok("es"),
            Language::German => Ok("de"),
            Language::Russian => Ok("ru"),
            Language::Italian => Ok("it"),
            Language::Portuguese => Ok("pt_pt"),
            Language::Turkish => Ok("tr"),
            Language::Arabic => Ok("ar"),
            Language::Vietnamese => Ok("vi"),
            Language::Thai => Ok("th"),
            Language::Indonesian => Ok("id"),
            Language::Malay => Ok("ms"),
            Language::Hindi => Ok("hi"),
            Language::Mongolian => Ok("mn_cy"),
            Language::Bokmal => Ok("nb_no"),
            Language::Nynorsk => Ok("nn_no"),
            Language::Persian => Ok("fa"),
            Language::Ukrainian => Ok("uk"),
        }
    } else {
        return Ok("en");
    }
}
