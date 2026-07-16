use crate::modules::news::domain::news_locale_search_patterns;

#[test]
fn news_locale_search_patterns_support_pc_and_region_locales() {
    assert_eq!(
        news_locale_search_patterns("zh").unwrap(),
        vec!["zh".to_owned(), "zh-%".to_owned()]
    );
    assert_eq!(
        news_locale_search_patterns("en-US").unwrap(),
        vec!["en-US".to_owned(), "en".to_owned()]
    );
    assert!(news_locale_search_patterns("../zh").is_err());
}
