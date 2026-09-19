use super::*;

pub(super) async fn cleanup_pair_journal(pool: &MySqlPool, pair: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE j FROM platform_financial_journal j JOIN trading_pairs p ON j.asset_id IN (p.base_asset, p.quote_asset) WHERE p.symbol = ? AND j.context = 'spot'",
    ).bind(pair).execute(pool).await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_journal_automatic_inventory_is_atomic_and_replay_stable()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    for side in ["buy", "sell"] {
        let user = create_user(&pool).await;
        let (base, base_symbol) = create_asset(&pool, "JB").await;
        let (quote, quote_symbol) = create_asset(&pool, "JQ").await;
        let pair = create_pair(&pool, base, quote, &base_symbol, &quote_symbol).await;
        let order = seed_open_order(&pool, user, &pair, side, "10", "2").await?;
        let (reserve_asset, inventory_asset, reserve) = if side == "buy" {
            (quote, base, "20")
        } else {
            (base, quote, "2")
        };
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, frozen) VALUES (?, ?, ?)")
            .bind(user)
            .bind(reserve_asset)
            .bind(decimal(reserve))
            .execute(&pool)
            .await?;
        fund_system_spot_liquidity(&pool, inventory_asset, &decimal("100")).await?;

        let trigger = format!("spot_journal_failure_{base}");
        sqlx::raw_sql(&format!(
            "CREATE TRIGGER {trigger} BEFORE INSERT ON platform_financial_journal FOR EACH ROW BEGIN \
             IF NEW.context = 'spot' AND NEW.asset_id = {base} THEN \
             SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'spot journal failure injection'; END IF; END"
        )).execute(&pool).await?;
        let failed =
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal("10"), None).await;
        sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
            .execute(&pool)
            .await?;
        assert_eq!(failed?, 0);
        let (state, filled): (String, BigDecimal) =
            sqlx::query_as("SELECT status, filled_quantity FROM spot_orders WHERE id = ?")
                .bind(&order)
                .fetch_one(&pool)
                .await?;
        assert_eq!(state, "open");
        assert_eq!(filled, 0);
        let (frozen,): (BigDecimal,) =
            sqlx::query_as("SELECT frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
                .bind(user)
                .bind(reserve_asset)
                .fetch_one(&pool)
                .await?;
        assert_eq!(frozen, decimal(reserve));
        let (trade_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM spot_trades WHERE pair_id = ?")
                .bind(pair_id(&pool, &pair).await?)
                .fetch_one(&pool)
                .await?;
        assert_eq!(trade_count, 0);
        let (journal_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM platform_financial_journal WHERE asset_id IN (?, ?)",
        )
        .bind(base)
        .bind(quote)
        .fetch_one(&pool)
        .await?;
        assert_eq!(journal_count, 0);

        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal("10"), None)
                .await?,
            1
        );
        let rows: Vec<(u64, String, BigDecimal)> = sqlx::query_as(
            "SELECT asset_id, account_code, amount FROM platform_financial_journal WHERE context = 'spot' AND asset_id IN (?, ?) ORDER BY asset_id, account_code",
        ).bind(base).bind(quote).fetch_all(&pool).await?;
        assert_eq!(rows.len(), 4);
        for (asset, amount, user_receives) in [
            (base, decimal("2"), side == "buy"),
            (quote, decimal("20"), side == "sell"),
        ] {
            let actual: Vec<_> = rows
                .iter()
                .filter(|(id, _, _)| *id == asset)
                .map(|(_, code, amount)| (code.as_str(), amount.clone()))
                .collect();
            let expected = if user_receives {
                vec![
                    ("platform_spot_inventory", amount.clone()),
                    ("user_spot_target_liability", -amount),
                ]
            } else {
                vec![
                    ("platform_spot_inventory", -amount.clone()),
                    ("user_spot_source_liability", amount),
                ]
            };
            assert_eq!(actual, expected);
        }
        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal("10"), None)
                .await?,
            0
        );
        let replay_rows: Vec<(u64, String, BigDecimal)> = sqlx::query_as(
            "SELECT asset_id, account_code, amount FROM platform_financial_journal WHERE context = 'spot' AND asset_id IN (?, ?) ORDER BY asset_id, account_code",
        ).bind(base).bind(quote).fetch_all(&pool).await?;
        assert_eq!(replay_rows, rows);
        cleanup_fixture(&pool, user, base, quote, &pair, &order).await?;
    }
    Ok(())
}
