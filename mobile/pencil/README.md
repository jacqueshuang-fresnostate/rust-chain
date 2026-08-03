# HIPPO Mobile UI/UX — Pencil Source

`hippo-mobile-uiux.pen` is the editable UI/UX blueprint for the production mobile client.
It is generated with the official Pencil CLI and uses the restored Home screen as the
visual source of truth.

## Visual language

- cool-white / near-black canvas;
- faint technical grid and hairline separators;
- mint primary actions and positive data;
- coral risk, sell and negative data;
- Geist for interface copy and Geist Mono for market/account data;
- 0–4px operational surfaces, restrained 12–20px editorial surfaces;
- Lucide icons only and no emoji;
- 44–52px interaction targets;
- five selected Dock entries, with spot, contract and seconds operational routes kept separate.

## Files

- `hippo-mobile-uiux.pen` — editable Pencil document.
- `artboards.json` — ordered artboard IDs used by review exports.
- `screen-inventory.md` — route-to-artboard and UX-state mapping.
- `scripts/*.js` — incremental Pencil `execute` inputs and audited corrections.
- `exports/` — key PNG previews and the multi-page review PDF.

The current source contains 43 top-level artboards. The selected production
baseline is `FwNBM` / `W1cWyh` / `miHnt` / `CvipW` for Home and `ftTny` /
`VoZfE` for Market Detail; dedicated light/dark Markets and Spot workstations
are also tracked in `artboards.json`.

The current spot workstation is `yzOPc` / `bo8k5`. Scripts
`13-rebuild-spot.js` and `14-fix-spot-submit.js` preserve the preceding rebuild
history; `artboards.json` and the live `.pen` source own the current IDs. The workstation defines the local candlestick texture, shared
`1m / 5m / 15m / 1h / 1d` interval rail, REST + WebSocket telemetry, order-book / recent
trade switcher, focused form state and separate light/dark review surfaces.

## Continue editing

Open the checked-in `.pen` file with Pencil, or apply a new incremental script with:

```bash
mobile/pencil/run-execute.sh \
  mobile/pencil/hippo-mobile-uiux.pen \
  mobile/pencil/scripts/NEXT-SLICE.js
```

Each JavaScript file is passed to Pencil's `execute` MCP tool. Save after every slice
and export only after the structural check reports no placeholder, zero-size or
horizontal-overflow nodes. The scripts preserve the creation and correction history;
`artboards.json` is the canonical ordered ID map for the current document.

The document is a design blueprint. Production data, routing, authentication,
WebSocket sessions and order behavior remain owned by `mobile/src/`; the spot blueprint
is mirrored by `mobile/src/views/TradeView.vue` without replacing real API data.
