use super::*;

#[test]
fn turnstile_policy_requires_secret_and_site_key() {
    let disabled = LoginTurnstilePolicy::new(false, false, true);
    assert!(!disabled.enabled());
    assert!(!disabled.requires_verification(false));

    let missing_site_key = LoginTurnstilePolicy::new(true, false, true);
    assert!(!missing_site_key.enabled());
    assert!(!missing_site_key.requires_verification(false));

    let missing_secret = LoginTurnstilePolicy::new(false, true, true);
    assert!(!missing_secret.enabled());
    assert!(!missing_secret.requires_verification(false));

    assert!(LoginTurnstilePolicy::new(true, true, false).enabled());
}

#[test]
fn turnstile_policy_preserves_clearance_compatibility_and_enforcement() {
    let compatible = LoginTurnstilePolicy::new(true, true, false);
    assert!(compatible.requires_verification(false));
    assert!(!compatible.requires_verification(true));

    let enforced = LoginTurnstilePolicy::new(true, true, true);
    assert!(enforced.requires_verification(false));
    assert!(enforced.requires_verification(true));
}
