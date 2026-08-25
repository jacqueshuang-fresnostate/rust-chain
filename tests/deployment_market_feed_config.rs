const ONEPANEL_EXAMPLE_COMPOSE: &str = include_str!("../docker-compose.1panel.example.yml");
const STANDARD_EXAMPLE_COMPOSE: &str = include_str!("../docker-compose.example.yml");
const ONEPANEL_ENV_EXAMPLE: &str = include_str!("../docker-compose.1panel.env.example");
const STANDARD_ENV_EXAMPLE: &str = include_str!("../docker-compose.env.example");

#[test]
fn deployment_examples_keep_market_feed_restart_fallbacks() {
    for (name, source) in [
        (
            "docker-compose.1panel.example.yml",
            ONEPANEL_EXAMPLE_COMPOSE,
        ),
        ("docker-compose.example.yml", STANDARD_EXAMPLE_COMPOSE),
    ] {
        assert!(
            source.contains("MARKET_FEED_SYMBOLS:"),
            "{name} must provide restart fallback symbols"
        );
        assert!(
            source.contains("MARKET_FEED_INTERVALS:"),
            "{name} must provide restart fallback intervals"
        );
        assert!(
            source.contains("MARKET_FEED_PROVIDERS:"),
            "{name} must provide a restart fallback provider"
        );
    }

    for (name, source) in [
        ("docker-compose.1panel.env.example", ONEPANEL_ENV_EXAMPLE),
        ("docker-compose.env.example", STANDARD_ENV_EXAMPLE),
    ] {
        assert!(
            source.contains("MARKET_FEED_SYMBOLS=BTCUSDT"),
            "{name} must document the fallback symbol format"
        );
        assert!(
            source.contains("MARKET_FEED_INTERVALS=1m,5m,15m,1h,1d"),
            "{name} must document supported fallback intervals"
        );
        assert!(
            source.contains("MARKET_FEED_PROVIDERS=bitget"),
            "{name} must document the fallback provider"
        );
    }
}
