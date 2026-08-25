use super::*;

#[test]
fn bootstrap_mode_is_disabled_by_default_and_rejects_ambiguous_values() {
    assert_eq!(
        BootstrapAdminMode::from_optional_value(None).unwrap(),
        BootstrapAdminMode::Disabled
    );
    assert_eq!(
        BootstrapAdminMode::from_optional_value(Some("create_admin".to_owned())).unwrap(),
        BootstrapAdminMode::CreateAdmin
    );
    assert!(BootstrapAdminMode::from_optional_value(Some(String::new())).is_err());
    assert!(BootstrapAdminMode::from_optional_value(Some("enabled".to_owned())).is_err());
}

#[test]
fn bootstrap_password_source_accepts_one_non_blank_compose_value_only() {
    assert!(select_password_source(None, None).is_err());
    assert!(select_password_source(Some("   ".to_owned()), Some(String::new())).is_err());
    assert_eq!(
        select_password_source(Some("secret".to_owned()), Some(String::new())).unwrap(),
        BootstrapPasswordSource::Direct("secret".to_owned())
    );
    assert_eq!(
        select_password_source(
            Some(String::new()),
            Some(" /run/secrets/bootstrap_admin ".to_owned())
        )
        .unwrap(),
        BootstrapPasswordSource::File("/run/secrets/bootstrap_admin".to_owned())
    );
    assert!(
        select_password_source(
            Some("secret".to_owned()),
            Some("/run/secrets/bootstrap_admin".to_owned())
        )
        .is_err()
    );
}
