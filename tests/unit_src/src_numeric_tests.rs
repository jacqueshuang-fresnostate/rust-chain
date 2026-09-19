use super::*;

#[test]
fn amount_storage_checks_fraction_and_integer_capacity_without_rounding() {
    for source in [
        "99999999999999999999.999999999999999999",
        "-99999999999999999999.999999999999999999",
        "0.000000000000000001",
        "9007199254740993.000000000000000001",
        "1.000000000000000000000",
        "0",
        "1e19",
    ] {
        assert!(parse_decimal_input(source).is_ok(), "{source}");
    }
    for source in ["1e20", "1e-19", "1.0000000000000000001", "-1e20"] {
        assert!(parse_decimal_input(source).is_err(), "{source}");
    }
    assert!(
        ensure_decimal_storage(
            &BigDecimal::from_str("0.1234567800").unwrap(),
            18,
            8,
            "rate"
        )
        .is_ok()
    );
    assert!(
        ensure_decimal_storage(&BigDecimal::from_str("0.123456789").unwrap(), 18, 8, "rate")
            .is_err()
    );
}

#[test]
fn decimal_input_bounds_reject_invalid_or_pathological_representations() {
    for source in [
        "NaN",
        "Infinity",
        "",
        "1e9999999999999999999999",
        "1e257",
        "0e-257",
        "true",
        "1_000",
        "1 000",
    ] {
        assert!(parse_decimal_input(source).is_err(), "{source}");
    }
    assert!(parse_decimal_input(&"1".repeat(257)).is_err());
    let extreme = BigDecimal::new(10.into(), i64::MIN);
    assert!(ensure_amount_storage(&extreme, "amount").is_err());
    let tiny = BigDecimal::new(10.into(), i64::MAX);
    assert!(ensure_amount_storage(&tiny, "amount").is_err());
}

#[derive(Debug, Deserialize)]
struct Input {
    #[serde(deserialize_with = "deserialize_decimal")]
    amount: BigDecimal,
    #[serde(default, deserialize_with = "deserialize_optional_decimal")]
    optional: Option<BigDecimal>,
    #[serde(default, deserialize_with = "deserialize_patch_decimal")]
    patch: Option<Option<BigDecimal>>,
}

#[derive(Debug, Deserialize)]
struct DecimalListInput {
    #[serde(default, deserialize_with = "deserialize_optional_decimals")]
    levels: Option<Vec<BigDecimal>>,
}

#[test]
fn decimal_list_ingress_bounds_every_element_and_keeps_optional_semantics() {
    for source in ["{}", r#"{"levels":null}"#] {
        assert_eq!(
            serde_json::from_str::<DecimalListInput>(source)
                .unwrap()
                .levels,
            None
        );
    }
    let input: DecimalListInput =
        serde_json::from_str(r#"{"levels":["1.00",9007199254740993.000000000000000001]}"#).unwrap();
    let levels = input.levels.unwrap();
    assert_eq!(levels[0], BigDecimal::from(1));
    assert_eq!(
        levels[1],
        BigDecimal::from_str("9007199254740993.000000000000000001").unwrap()
    );
    assert_eq!(
        serde_json::from_str::<DecimalListInput>(r#"{"levels":[]}"#)
            .unwrap()
            .levels,
        Some(vec![])
    );
    for source in [
        r#"{"levels":["1","1e999999999"]}"#,
        r#"{"levels":[1e999999999]}"#,
        r#"{"levels":["1","1e-19"]}"#,
        r#"{"levels":[true]}"#,
        r#"{"levels":"1"}"#,
    ] {
        assert!(
            serde_json::from_str::<DecimalListInput>(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn decimal_dto_retains_json_literals_and_patch_null_semantics() {
    let input: Input =
        serde_json::from_str(r#"{"amount":9007199254740993.000000000000000001,"patch":null}"#)
            .unwrap();
    assert_eq!(
        input.amount,
        BigDecimal::from_str("9007199254740993.000000000000000001").unwrap()
    );
    assert_eq!(input.optional, None);
    assert_eq!(input.patch, Some(None));
    let missing: Input = serde_json::from_str(r#"{"amount":"1"}"#).unwrap();
    assert_eq!(missing.patch, None);
    for json in [
        r#"{"amount":"1e999999"}"#,
        r#"{"amount":1e999999}"#,
        r#"{"amount":"1","optional":"1e20"}"#,
        r#"{"amount":"1","patch":"1e-19"}"#,
        r#"{"amount":false}"#,
    ] {
        assert!(serde_json::from_str::<Input>(json).is_err(), "{json}");
    }
}
