# Polymarket Page Reference

Source: https://polymarket.com/zh and one market detail page inspected on 2026-06-17.

## Observed List Page Patterns

- Primary navigation groups markets by Browse filters such as New, Trending, Popular, Liquid, Ending Soon, Competitive.
- Topic navigation exposes high-frequency categories such as Live Crypto, Politics, Crypto, Sports, Pop Culture, Tech, AI, Economy, Weather, Elections, and more.
- The home page leads with Featured markets, market cards show title, category/topic chips, outcome prices/probabilities, volume, and end/live metadata.
- The page also includes a markets-by-category/topics section for discovery.

## Observed Detail Page Patterns

- Detail pages keep the category/topic chips and market title at the top.
- The detail body shows implied probability, volume, end date, order book/trade area, rules, market context, and related markets.
- The trading control is outcome-oriented: choose Yes/No or one of the available outcomes, then place the trade from the detail page.

## Mapping To This Repo

- The backend already exposes list and detail APIs: GET /api/v1/prediction/markets and GET /api/v1/prediction/markets/:id.
- MVP should reuse the existing local quote/order API and virtual-asset settlement contract.
- Current PC page already localizes dynamic Polymarket text and has order-ticket logic; move this into a Polymarket-style list/detail layout rather than creating a new product module.
