use std::collections::BTreeSet;

const COUNTRY_LOCALE_MIGRATION: &str = include_str!("../migrations/0042_country_locale_config.sql");
const SCHEMA_COMMENT_MIGRATION: &str =
    include_str!("../migrations/0052_schema_column_comments_zh.sql");
const COUNTRY_CODES_MIGRATION: &str = include_str!("../migrations/0054_seed_country_codes.sql");
const COUNTRY_REMARK_BACKFILL_MIGRATION: &str =
    include_str!("../migrations/0055_country_config_local_names_and_remark.sql");

fn seeded_country_rows() -> Vec<&'static str> {
    COUNTRY_CODES_MIGRATION
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("('"))
        .collect()
}

fn parse_country_code(row: &str) -> &str {
    row.strip_prefix("('")
        .and_then(|rest| rest.split_once("',"))
        .map(|(code, _)| code)
        .expect("country seed row should start with a quoted country code")
}

fn parse_quoted_fields(row: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut chars = row.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\'' {
            continue;
        }

        let mut field = String::new();
        while let Some(value) = chars.next() {
            if value == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    field.push('\'');
                    continue;
                }
                break;
            }
            field.push(value);
        }
        fields.push(field);
    }
    fields
}

#[test]
fn country_seed_migration_preserves_existing_configs() {
    assert!(
        COUNTRY_CODES_MIGRATION.contains("INSERT IGNORE INTO country_configs"),
        "country seed should avoid overwriting existing country_configs rows"
    );
    assert!(
        !COUNTRY_CODES_MIGRATION.contains("ON DUPLICATE KEY UPDATE"),
        "country seed should not update customized country settings"
    );
}

#[test]
fn country_locale_base_migration_keeps_applied_schema_stable() {
    assert!(
        !COUNTRY_LOCALE_MIGRATION.contains("remark VARCHAR"),
        "0042 has already been applied in existing databases and must not be rewritten with remark"
    );
    assert!(
        COUNTRY_REMARK_BACKFILL_MIGRATION.contains("ADD COLUMN remark"),
        "remark must be introduced by a later additive migration"
    );
}

#[test]
fn schema_comment_migration_keeps_applied_country_comments_stable() {
    assert!(
        SCHEMA_COMMENT_MIGRATION
            .contains("WHEN c.COLUMN_NAME = 'country_name' THEN '国家或地区名称'"),
        "0052 has already been applied and should keep the original country_name comment"
    );
    assert!(
        !SCHEMA_COMMENT_MIGRATION.contains("WHEN c.COLUMN_NAME = 'remark'"),
        "0052 should not describe country_configs.remark before 0055 adds it"
    );
    assert!(
        COUNTRY_REMARK_BACKFILL_MIGRATION.contains("COMMENT ''中文国家或地区名称备注''"),
        "0055 should own the remark column comment"
    );
}

#[test]
fn country_seed_migration_keeps_applied_english_names_until_backfill() {
    assert!(
        COUNTRY_CODES_MIGRATION.contains("(country_code, country_name, default_locale"),
        "country seed should use the schema available before the remark backfill"
    );
    assert!(
        !COUNTRY_CODES_MIGRATION.contains("country_name, remark"),
        "country seed should not depend on the remark column before 0055 runs"
    );
    assert!(
        !COUNTRY_CODES_MIGRATION.contains("Chinese remarks are added"),
        "0054 has already been applied and should not gain later explanatory comments"
    );

    let rows = seeded_country_rows();
    let entries = rows
        .iter()
        .map(|row| {
            let fields = parse_quoted_fields(row);
            (fields[0].clone(), fields[1].clone())
        })
        .collect::<BTreeSet<_>>();

    for expected in [
        ("CN".to_owned(), "China".to_owned()),
        ("DE".to_owned(), "Germany".to_owned()),
        ("JP".to_owned(), "Japan".to_owned()),
        ("KR".to_owned(), "South Korea".to_owned()),
        ("US".to_owned(), "United States".to_owned()),
    ] {
        assert!(
            entries.contains(&expected),
            "missing original seed country name for {}",
            expected.0
        );
    }
}

#[test]
fn country_remark_backfill_adds_column_and_updates_old_seed_names() {
    assert!(
        COUNTRY_REMARK_BACKFILL_MIGRATION.contains("ADD COLUMN remark"),
        "backfill migration should add the remark column for existing databases"
    );
    assert!(
        COUNTRY_REMARK_BACKFILL_MIGRATION.contains("previous_country_name"),
        "backfill migration should only rewrite old seeded English names"
    );
    assert!(
        COUNTRY_REMARK_BACKFILL_MIGRATION.contains("'China' AS previous_country_name"),
        "backfill migration should migrate the original China seed name"
    );
    assert!(
        COUNTRY_REMARK_BACKFILL_MIGRATION.contains("'美国' AS remark"),
        "backfill migration should populate Chinese country remarks"
    );
}

#[test]
fn country_seed_migration_covers_common_registration_markets() {
    let rows = seeded_country_rows();
    let codes = rows
        .iter()
        .map(|row| parse_country_code(row))
        .collect::<BTreeSet<_>>();

    assert!(
        rows.len() >= 240,
        "expected most ISO 3166-1 alpha-2 countries/regions to be seeded"
    );

    for code in [
        "AE", "AU", "BR", "CA", "CN", "DE", "FR", "GB", "HK", "IN", "JP", "KR", "SG", "TW", "US",
        "ZA",
    ] {
        assert!(codes.contains(code), "missing seeded country code {code}");
    }
}

#[test]
fn country_seed_migration_uses_unique_alpha_2_codes() {
    let rows = seeded_country_rows();
    let mut codes = BTreeSet::new();

    for row in &rows {
        let code = parse_country_code(row);
        assert_eq!(code.len(), 2, "country code should be alpha-2: {code}");
        assert!(
            code.chars().all(|ch| ch.is_ascii_uppercase()),
            "country code should be uppercase ASCII: {code}"
        );
        assert!(codes.insert(code), "duplicate country code seed: {code}");
    }

    assert_eq!(
        codes.len(),
        rows.len(),
        "every country seed row should have a unique country code"
    );
}
