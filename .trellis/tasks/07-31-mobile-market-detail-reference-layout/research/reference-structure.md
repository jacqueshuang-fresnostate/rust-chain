# Reference Structure Analysis

## Observed hierarchy

1. Compact symbol Header with two utility actions.
2. One-line domain navigation with a thin active indicator.
3. Dense quote block: dominant last price left, four concise statistics right.
4. Interval and chart-tool rail directly attached to an edge-to-edge chart.
5. Indicator legend integrated into the chart rather than placed in a separate card.
6. Market-data rail below the chart; order book and recent trades occupy one shared viewport.
7. Fixed bottom instrument deck with one dominant trade action and compact secondary actions.

## Material cues to adapt

- Near-black graphite rather than soft gray cards in dark mode.
- Hairline separators and subtle inset highlights provide structure.
- Large tabular numerals and tightly spaced small labels create professional density.
- Market colors are luminous data signals, not general decoration.
- Controls use compact filled chips or precision icon bezels; large generic rounded cards are absent.
- Chart and order book are nearly edge-to-edge, creating one continuous instrument canvas.

## Deliberate non-copy items

- Reference brand marks and watermarks.
- Leverage selector, mark price, ranking, strategy, grid and alert controls.
- Market statistics that HIPPO does not receive from the backend.
- Proprietary icons or image assets.

## HIPPO mapping

| Reference surface | HIPPO implementation |
| --- | --- |
| Symbol Header | 44px back + asset mark/pair selector + local favorite + share; no badge or microquote |
| Domain tabs | Two truthful sections: market workstation and real base-asset overview |
| Quote block | Real last/high/low/volume/change only |
| Indicator chart | Real candles, volume, MA5/10/20 overlays and legend |
| Order book rail | Real seven-row paired depth panel and real latest-trades tab |
| Bottom deck | Spot trade, futures and orders routes |
