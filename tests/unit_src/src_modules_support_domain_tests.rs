use super::*;

#[test]
fn support_message_validation_trims_and_counts_unicode_scalars() {
    let body = format!("  {}  ", "🦛".repeat(SUPPORT_MESSAGE_MAX_SCALARS));
    let validated = validate_support_message(body, "client_key-1234".to_owned()).unwrap();
    assert_eq!(validated.body.chars().count(), SUPPORT_MESSAGE_MAX_SCALARS);
    assert_eq!(
        validated.preview.chars().count(),
        SUPPORT_MESSAGE_PREVIEW_SCALARS
    );

    let too_long = "客".repeat(SUPPORT_MESSAGE_MAX_SCALARS + 1);
    assert!(matches!(
        validate_support_message(too_long, "client_key-1234".to_owned()),
        Err(AppError::Validation(message)) if message.contains("at most 2000")
    ));
    assert!(matches!(
        validate_support_message("  \n\t ".to_owned(), "client_key-1234".to_owned()),
        Err(AppError::Validation(message)) if message.contains("must not be empty")
    ));
}

#[test]
fn support_client_message_id_requires_a_bounded_safe_token() {
    for invalid in [
        "short",
        "contains space",
        "contains/slash",
        "不是ascii-token",
        "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklm",
    ] {
        assert!(matches!(
            validate_support_message("你好".to_owned(), invalid.to_owned()),
            Err(AppError::Validation(message)) if message.contains("safe token")
        ));
    }

    for valid in ["abcdefgh", "ABC_1234", "uuid-like_1234-5678"] {
        assert!(validate_support_message("你好".to_owned(), valid.to_owned()).is_ok());
    }
}

#[test]
fn support_status_and_pagination_are_server_bounded() {
    assert_eq!(
        SupportConversationStatus::parse("open").unwrap(),
        SupportConversationStatus::Open
    );
    assert_eq!(
        SupportConversationStatus::parse("closed").unwrap(),
        SupportConversationStatus::Closed
    );
    assert!(SupportConversationStatus::parse("OPEN").is_err());

    assert_eq!(
        support_offset_page(Some(0), Some(u32::MAX)),
        SupportOffsetPage {
            limit: 1,
            offset: SUPPORT_PAGE_MAX_OFFSET,
        }
    );
    assert_eq!(
        support_message_page(Some(u32::MAX), Some(42)).unwrap(),
        SupportMessagePage {
            limit: SUPPORT_PAGE_MAX_LIMIT,
            before_id: Some(42),
        }
    );
    assert!(support_message_page(None, Some(0)).is_err());
}
