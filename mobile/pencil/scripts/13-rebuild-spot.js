const staleSpotArtboards = [];
Get((node) => {
  if (node.name === "05 / Spot Trading" || node.name === "05B / Spot Trading / Dark") {
    staleSpotArtboards.push(node.id);
  }
});
for (const id of staleSpotArtboards) Delete(id);

function T(parent, name, content, size, weight, fill, width, family, align) {
  return Insert(parent, {
    type: "text",
    name,
    content,
    fontFamily: family || "$font-sans",
    fontSize: size,
    fontWeight: weight || "400",
    fill: fill || "$text",
    textGrowth: width ? "fixed-width" : "auto",
    width: width || undefined,
    textAlign: align || undefined,
    lineHeight: 1.25,
  });
}

function I(parent, name, icon, fill, size) {
  return Insert(parent, {
    type: "icon",
    name,
    library: "lucide",
    icon,
    width: size || 20,
    height: size || 20,
    fill: fill || "$text",
  });
}

function screen(name, mode) {
  const position = FindEmptySpace({ width: 390, height: 1680, padding: 100 });
  return Insert(document, {
    type: "frame",
    name,
    x: position.x,
    y: position.y,
    width: 390,
    height: "fit_content(1680)",
    layout: "vertical",
    fill: "$canvas",
    theme: { mode },
    clip: true,
    placeholder: true,
  });
}

function status(parent) {
  const bar = Insert(parent, {
    type: "frame",
    name: "Status Bar",
    layout: "horizontal",
    width: "fill_container",
    height: 28,
    padding: [0, 16],
    justifyContent: "space_between",
    alignItems: "center",
  });
  T(bar, "Time", "09:41", 11, "650", "$text", undefined, "$font-data");
  const signals = Insert(bar, {
    type: "frame",
    name: "Status Signals",
    layout: "horizontal",
    gap: 8,
    alignItems: "center",
  });
  T(signals, "Network", "4G+", 10, "550", "$muted", undefined, "$font-data");
  I(signals, "Wifi", "wifi", "$text", 14);
  T(signals, "Battery", "82%", 10, "550", "$text", undefined, "$font-data");
}

function circle(parent, name, icon, active) {
  const control = Insert(parent, {
    type: "frame",
    name,
    layout: "horizontal",
    width: 44,
    height: 44,
    justifyContent: "center",
    alignItems: "center",
    fill: active ? "$mint-soft" : "$surface-2",
    stroke: active ? "$mint" : "$border",
    strokeWidth: 1,
    strokeAlignment: "inner",
    cornerRadius: 22,
  });
  I(control, `${name} Icon`, icon, active ? "$mint-strong" : "$text", 19);
  return control;
}

function header(parent) {
  const shell = Insert(parent, {
    type: "frame",
    name: "Spot Header",
    layout: "horizontal",
    width: "fill_container",
    height: 64,
    padding: [0, 16],
    gap: 10,
    alignItems: "center",
    fill: "$surface",
    stroke: "$border",
    strokeWidth: { bottom: 1 },
    strokeAlignment: "inner",
  });
  circle(shell, "Back", "arrow-left", false);
  const identity = Insert(shell, {
    type: "frame",
    name: "Spot Identity",
    layout: "vertical",
    width: "fill_container",
    gap: 3,
  });
  const context = Insert(identity, {
    type: "frame",
    name: "Spot Context",
    layout: "horizontal",
    gap: 6,
    alignItems: "center",
  });
  const badge = Insert(context, {
    type: "frame",
    name: "Spot Badge",
    layout: "horizontal",
    height: 20,
    padding: [0, 7],
    alignItems: "center",
    justifyContent: "center",
    fill: "$mint-soft",
    stroke: "$mint",
    strokeWidth: 1,
    strokeAlignment: "inner",
    cornerRadius: 10,
  });
  T(badge, "Spot Badge Label", "现货", 9, "700", "$mint-strong", undefined, "$font-data");
  T(context, "Spot Context Label", "SPOT / LIVE", 9, "600", "$muted", undefined, "$font-data");
  T(identity, "Spot Pair", "BTC/USDT", 19, "750", "$text", undefined, "$font-data");
  circle(shell, "Spot Orders", "list", false);
}

function liveDot(parent, name, label, color) {
  const item = Insert(parent, {
    type: "frame",
    name,
    layout: "horizontal",
    gap: 5,
    alignItems: "center",
  });
  Insert(item, { type: "ellipse", name: `${name} Dot`, width: 6, height: 6, fill: color });
  T(item, `${name} Label`, label, 9, "600", "$muted", undefined, "$font-data");
}

function quote(parent) {
  const shell = Insert(parent, {
    type: "frame",
    name: "Spot Quote Hero",
    layout: "vertical",
    width: "fill_container",
    padding: [14, 16, 10, 16],
    gap: 12,
    fill: "$surface-2",
    stroke: "$border",
    strokeWidth: { bottom: 1 },
    strokeAlignment: "inner",
  });
  const main = Insert(shell, {
    type: "frame",
    name: "Spot Quote Main",
    layout: "horizontal",
    width: "fill_container",
    justifyContent: "space_between",
    alignItems: "end",
  });
  const latest = Insert(main, {
    type: "frame",
    name: "Spot Latest Price",
    layout: "vertical",
    gap: 5,
  });
  T(latest, "Spot Latest Label", "最新价格", 10, "600", "$muted");
  T(latest, "Spot Latest Value", "63,085.00", 34, "750", "$mint-strong", undefined, "$font-data");
  T(latest, "Spot Latest Change", "+0.58%  /  +365.20", 11, "650", "$mint-strong", undefined, "$font-data");
  const stats = Insert(main, {
    type: "frame",
    name: "Spot 24h Stats",
    layout: "vertical",
    width: 152,
    gap: 5,
  });
  for (const row of [
    ["24h 最高", "65,408.68"],
    ["24h 最低", "62,460.01"],
    ["24h 成交量", "4,949.43 BTC"],
  ]) {
    const line = Insert(stats, {
      type: "frame",
      name: `Spot ${row[0]}`,
      layout: "horizontal",
      width: "fill_container",
      justifyContent: "space_between",
      alignItems: "center",
    });
    T(line, `${row[0]} Label`, row[0], 9, "500", "$muted");
    T(line, `${row[0]} Value`, row[1], 10, "650", "$text", undefined, "$font-data");
  }
  const telemetry = Insert(shell, {
    type: "frame",
    name: "Spot Live Telemetry",
    layout: "horizontal",
    width: "fill_container",
    height: 24,
    gap: 14,
    alignItems: "center",
    stroke: "$hairline",
    strokeWidth: { top: 1 },
    strokeAlignment: "inner",
  });
  liveDot(telemetry, "Spot Socket Status", "REST + WS", "$mint");
  liveDot(telemetry, "Spot Depth Status", "深度实时", "$mint");
  liveDot(telemetry, "Spot Kline Status", "K线实时", "$mint");
}

function intervalTools(parent) {
  const tools = Insert(parent, {
    type: "frame",
    name: "Spot Chart Tools",
    layout: "horizontal",
    width: "fill_container",
    height: 48,
    padding: [0, 12],
    gap: 4,
    alignItems: "center",
    fill: "$surface",
    stroke: "$border",
    strokeWidth: { bottom: 1 },
    strokeAlignment: "inner",
  });
  for (const interval of ["1m", "5m", "15m", "1h", "1d"]) {
    const active = interval === "15m";
    const item = Insert(tools, {
      type: "frame",
      name: `Spot Interval ${interval}`,
      layout: "horizontal",
      width: 42,
      height: 32,
      justifyContent: "center",
      alignItems: "center",
      fill: active ? "$mint-soft" : "$surface",
      stroke: active ? "$mint" : "$surface",
      strokeWidth: 1,
      strokeAlignment: "inner",
      cornerRadius: 2,
    });
    T(item, `${interval} Label`, interval, 10, active ? "700" : "500", active ? "$mint-strong" : "$muted", undefined, "$font-data");
  }
  const engine = Insert(tools, {
    type: "frame",
    name: "Spot Chart Engine",
    layout: "horizontal",
    width: "fill_container",
    height: 32,
    padding: [0, 8],
    gap: 5,
    justifyContent: "end",
    alignItems: "center",
  });
  I(engine, "Spot Engine Icon", "chart-line", "$muted", 15);
  T(engine, "Spot Engine Label", "LOCAL", 9, "650", "$muted", undefined, "$font-data");
  const expand = Insert(tools, {
    type: "frame",
    name: "Spot Chart Expand",
    layout: "horizontal",
    width: 36,
    height: 36,
    justifyContent: "center",
    alignItems: "center",
    fill: "$surface-2",
    stroke: "$border",
    strokeWidth: 1,
    strokeAlignment: "inner",
    cornerRadius: 18,
  });
  I(expand, "Spot Chart Expand Icon", "maximize-2", "$text", 16);
}

function klineChart(parent) {
  const chart = Insert(parent, {
    type: "frame",
    name: "Spot Local Kline Chart",
    layout: "none",
    width: "fill_container",
    height: 252,
    fill: "$surface",
    clip: true,
  });
  for (const y of [42, 84, 126, 168, 210]) {
    Insert(chart, { type: "rectangle", name: `Spot Chart H ${y}`, x: 0, y, width: 390, height: 1, fill: "$hairline", opacity: 0.75, layoutPosition: "absolute" });
  }
  for (const x of [66, 132, 198, 264, 330]) {
    Insert(chart, { type: "rectangle", name: `Spot Chart V ${x}`, x, y: 0, width: 1, height: 252, fill: "$hairline", opacity: 0.55, layoutPosition: "absolute" });
  }
  const ma5 = T(chart, "Spot MA5", "MA5 63,725.5", 9, "650", "$warning", 96, "$font-data");
  Update(ma5, { x: 12, y: 9, layoutPosition: "absolute" });
  const ma10 = T(chart, "Spot MA10", "MA10 63,767.3", 9, "650", "$coral", 104, "$font-data");
  Update(ma10, { x: 112, y: 9, layoutPosition: "absolute" });
  const volume = T(chart, "Spot VOL", "VOL 55.2", 9, "650", "$mint-strong", 78, "$font-data");
  Update(volume, { x: 224, y: 9, layoutPosition: "absolute" });
  const candles = [
    [168, 148, 132, 182], [148, 158, 139, 172], [158, 132, 122, 169], [132, 112, 101, 146],
    [112, 126, 103, 140], [126, 104, 91, 135], [104, 86, 72, 116], [86, 96, 78, 109],
    [96, 75, 62, 104], [75, 68, 56, 86], [68, 83, 61, 94], [83, 72, 63, 91],
    [72, 92, 64, 103], [92, 112, 82, 124], [112, 128, 101, 142], [128, 119, 108, 139],
    [119, 143, 111, 156], [143, 116, 103, 151],
  ];
  candles.forEach((candle, index) => {
    const x = 14 + index * 18;
    const open = candle[0];
    const close = candle[1];
    const high = candle[2];
    const low = candle[3];
    const up = close < open;
    const color = up ? "$mint-strong" : "$coral";
    Insert(chart, { type: "rectangle", name: `Spot Candle Wick ${index}`, x: x + 3, y: high, width: 1, height: low - high, fill: color, layoutPosition: "absolute" });
    Insert(chart, { type: "rectangle", name: `Spot Candle Body ${index}`, x, y: Math.min(open, close), width: 7, height: Math.max(3, Math.abs(close - open)), fill: color, layoutPosition: "absolute" });
    Insert(chart, { type: "rectangle", name: `Spot Volume ${index}`, x, y: 224 - ((index * 7) % 22), width: 7, height: 10 + ((index * 7) % 22), fill: color, opacity: 0.42, layoutPosition: "absolute" });
  });
  Insert(chart, {
    type: "path",
    name: "Spot Moving Average Fast",
    geometry: "M 14 151 C 48 144 64 119 90 121 S 132 80 158 88 S 200 62 228 79 S 274 92 306 124 S 326 123 338 117",
    viewBox: [0, 0, 390, 252],
    width: 390,
    height: 252,
    stroke: "$warning",
    strokeWidth: 1.5,
    strokeLinecap: "round",
    fill: "#00000000",
    layoutPosition: "absolute",
  });
  Insert(chart, {
    type: "path",
    name: "Spot Moving Average Slow",
    geometry: "M 14 164 C 54 148 78 132 106 128 S 148 102 176 96 S 218 84 246 91 S 292 109 338 127",
    viewBox: [0, 0, 390, 252],
    width: 390,
    height: 252,
    stroke: "$blue",
    strokeWidth: 1.5,
    strokeLinecap: "round",
    fill: "#00000000",
    layoutPosition: "absolute",
  });
  for (const y of [44, 86, 128, 170]) {
    const axis = T(chart, `Spot Price Axis ${y}`, ["65,400", "64,600", "63,800", "63,000"][[44, 86, 128, 170].indexOf(y)], 9, "500", "$muted", 48, "$font-data", "right");
    Update(axis, { x: 336, y: y - 7, layoutPosition: "absolute" });
  }
  for (const x of [8, 116, 224]) {
    const axis = T(chart, `Spot Time Axis ${x}`, ["09:00", "12:00", "15:00"][[8, 116, 224].indexOf(x)], 9, "500", "$muted", 56, "$font-data");
    Update(axis, { x, y: 237, layoutPosition: "absolute" });
  }
  for (let x = 0; x < 332; x += 12) {
    Insert(chart, { type: "rectangle", name: `Spot Live Price Dash ${x}`, x, y: 117, width: 7, height: 1, fill: "$mint-strong", opacity: 0.7, layoutPosition: "absolute" });
  }
  const priceTag = Insert(chart, {
    type: "frame",
    name: "Spot Live Price Tag",
    x: 326,
    y: 103,
    width: 64,
    height: 28,
    layout: "horizontal",
    justifyContent: "center",
    alignItems: "center",
    fill: "$mint-soft",
    stroke: "$mint",
    strokeWidth: 1,
    strokeAlignment: "inner",
    cornerRadius: 2,
    layoutPosition: "absolute",
  });
  T(priceTag, "Spot Live Price Tag Label", "63,085", 9, "700", "$mint-strong", undefined, "$font-data");
}

function marketTabs(parent) {
  const tabs = Insert(parent, {
    type: "frame",
    name: "Spot Market Data Tabs",
    layout: "horizontal",
    width: "fill_container",
    height: 48,
    fill: "$surface",
    stroke: "$border",
    strokeWidth: { top: 1, bottom: 1 },
    strokeAlignment: "inner",
  });
  for (const label of ["订单簿", "最新成交"]) {
    const active = label === "订单簿";
    const tab = Insert(tabs, {
      type: "frame",
      name: `Spot ${label} Tab`,
      layout: "vertical",
      width: "fill_container",
      height: 48,
      justifyContent: "center",
      alignItems: "center",
      stroke: active ? "$mint-strong" : "$surface",
      strokeWidth: { bottom: active ? 2 : 0 },
      strokeAlignment: "inner",
    });
    T(tab, `Spot ${label} Tab Label`, label, 12, active ? "700" : "500", active ? "$text" : "$muted");
  }
}

function orderBook(parent) {
  const book = Insert(parent, {
    type: "frame",
    name: "Spot Live Order Book",
    layout: "vertical",
    width: "fill_container",
    fill: "$surface",
    padding: [0, 10, 10, 10],
  });
  const latest = Insert(book, {
    type: "frame",
    name: "Spot Book Latest",
    layout: "horizontal",
    width: "fill_container",
    height: 40,
    padding: [0, 2],
    justifyContent: "space_between",
    alignItems: "center",
  });
  T(latest, "Spot Bid Header", "买入 (BTC)", 10, "650", "$mint-strong");
  const center = Insert(latest, { type: "frame", name: "Spot Last Price", layout: "vertical", gap: 1, alignItems: "center" });
  T(center, "Spot Last Price Label", "最新成交", 8, "500", "$muted");
  T(center, "Spot Last Price Value", "63,085.00", 12, "700", "$text", undefined, "$font-data");
  T(latest, "Spot Ask Header", "卖出 (BTC)", 10, "650", "$coral");
  const columns = Insert(book, {
    type: "frame",
    name: "Spot Book Columns",
    layout: "horizontal",
    width: "fill_container",
    height: 24,
    padding: [0, 2],
    gap: 4,
    alignItems: "center",
  });
  for (const column of [["数量", 74], ["买价", 88], ["卖价", 88], ["数量", 74]]) {
    T(columns, `Spot ${column[0]} ${column[1]}`, column[0], 8, "550", "$muted", column[1], "$font-data", "center");
  }
  const bids = ["2.21801", "0.04973", "0.16660", "0.03144", "0.00001", "0.00786"];
  const bidPrices = ["63,084.9", "63,084.2", "63,084.1", "63,082.0", "63,081.1", "63,077.9"];
  const askPrices = ["63,085.0", "63,087.8", "63,087.9", "63,088.0", "63,089.0", "63,089.2"];
  const asks = ["0.63975", "0.00001", "0.11318", "0.21595", "0.18798", "0.00284"];
  const widths = [126, 78, 112, 88, 60, 96];
  for (let index = 0; index < 6; index += 1) {
    const row = Insert(book, {
      type: "frame",
      name: `Spot Book Row ${index + 1}`,
      layout: "horizontal",
      width: "fill_container",
      height: 30,
      padding: [0, 2],
      gap: 4,
      alignItems: "center",
      stroke: "$hairline",
      strokeWidth: { bottom: 1 },
      strokeAlignment: "inner",
    });
    Insert(row, { type: "rectangle", name: `Spot Bid Depth ${index + 1}`, x: 40, y: 2, width: widths[index], height: 26, fill: "$mint-soft", opacity: 0.72, layoutPosition: "absolute" });
    Insert(row, { type: "rectangle", name: `Spot Ask Depth ${index + 1}`, x: 194, y: 2, width: widths[5 - index], height: 26, fill: "$coral-soft", opacity: 0.72, layoutPosition: "absolute" });
    T(row, `Spot Bid Quantity ${index + 1}`, bids[index], 9, "500", "$text", 74, "$font-data", "left");
    T(row, `Spot Bid Price ${index + 1}`, bidPrices[index], 9, "650", "$mint-strong", 88, "$font-data", "right");
    T(row, `Spot Ask Price ${index + 1}`, askPrices[index], 9, "650", "$coral", 88, "$font-data", "left");
    T(row, `Spot Ask Quantity ${index + 1}`, asks[index], 9, "500", "$text", 74, "$font-data", "right");
  }
}

function field(parent, name, label, value, unit, focused) {
  const shell = Insert(parent, {
    type: "frame",
    name,
    layout: "vertical",
    width: "fill_container",
    height: 66,
    padding: [8, 12],
    gap: 4,
    fill: focused ? "$surface-2" : "$surface",
    stroke: focused ? "$blue" : "$border",
    strokeWidth: focused ? 2 : 1,
    strokeAlignment: "inner",
    cornerRadius: 4,
  });
  const meta = Insert(shell, {
    type: "frame",
    name: `${name} Meta`,
    layout: "horizontal",
    width: "fill_container",
    justifyContent: "space_between",
    alignItems: "center",
  });
  T(meta, `${name} Label`, label, 9, "600", "$muted");
  T(meta, `${name} Unit`, unit, 9, "600", "$muted", undefined, "$font-data");
  T(shell, `${name} Value`, value, 17, "600", "$text", undefined, "$font-data");
}

function orderDesk(parent) {
  const desk = Insert(parent, {
    type: "frame",
    name: "Spot Order Desk",
    layout: "vertical",
    width: "fill_container",
    padding: [14, 16, 16, 16],
    gap: 10,
    fill: "$surface-2",
    stroke: "$border",
    strokeWidth: { top: 1, bottom: 1 },
    strokeAlignment: "inner",
  });
  const heading = Insert(desk, {
    type: "frame",
    name: "Spot Order Heading",
    layout: "horizontal",
    width: "fill_container",
    height: 42,
    justifyContent: "space_between",
    alignItems: "center",
  });
  const copy = Insert(heading, { type: "frame", name: "Spot Order Heading Copy", layout: "vertical", gap: 2 });
  T(copy, "Spot Order Eyebrow", "SPOT ORDER / 现货委托", 9, "650", "$mint-strong", undefined, "$font-data");
  T(copy, "Spot Order Title", "买入 BTC", 18, "750", "$text");
  const open = Insert(heading, {
    type: "frame",
    name: "Spot Open Orders Shortcut",
    layout: "horizontal",
    height: 36,
    padding: [0, 10],
    gap: 5,
    justifyContent: "center",
    alignItems: "center",
    fill: "$surface",
    stroke: "$border",
    strokeWidth: 1,
    strokeAlignment: "inner",
    cornerRadius: 18,
  });
  T(open, "Spot Open Orders Shortcut Label", "当前委托", 10, "650", "$text");
  I(open, "Spot Open Orders Shortcut Icon", "chevron-right", "$muted", 14);
  const sides = Insert(desk, {
    type: "frame",
    name: "Spot Buy Sell Switch",
    layout: "horizontal",
    width: "fill_container",
    height: 50,
    fill: "$surface",
    stroke: "$border",
    strokeWidth: 1,
    strokeAlignment: "inner",
    cornerRadius: 4,
  });
  for (const side of ["买入", "卖出"]) {
    const active = side === "买入";
    const item = Insert(sides, {
      type: "frame",
      name: `Spot ${side}`,
      layout: "horizontal",
      width: "fill_container",
      height: 50,
      justifyContent: "center",
      alignItems: "center",
      fill: active ? "$mint-soft" : "$surface",
      stroke: active ? "$mint" : "$surface",
      strokeWidth: active ? 1 : 0,
      strokeAlignment: "inner",
    });
    T(item, `Spot ${side} Label`, side, 13, active ? "700" : "550", active ? "$mint-strong" : "$muted");
  }
  const types = Insert(desk, {
    type: "frame",
    name: "Spot Order Type Switch",
    layout: "horizontal",
    width: "fill_container",
    height: 42,
    gap: 6,
    alignItems: "center",
  });
  for (const type of ["限价", "市价", "止盈止损"]) {
    const active = type === "限价";
    const item = Insert(types, {
      type: "frame",
      name: `Spot ${type} Type`,
      layout: "horizontal",
      width: "fill_container",
      height: 36,
      justifyContent: "center",
      alignItems: "center",
      fill: active ? "$surface" : "$surface-2",
      stroke: active ? "$mint" : "$border",
      strokeWidth: 1,
      strokeAlignment: "inner",
      cornerRadius: 2,
    });
    T(item, `Spot ${type} Type Label`, type, 10, active ? "700" : "500", active ? "$text" : "$muted");
  }
  const balance = Insert(desk, {
    type: "frame",
    name: "Spot Available Balance",
    layout: "horizontal",
    width: "fill_container",
    height: 32,
    justifyContent: "space_between",
    alignItems: "center",
  });
  T(balance, "Spot Available Balance Label", "现货账户可用", 10, "500", "$muted");
  T(balance, "Spot Available Balance Value", "1,284.00 USDT", 11, "650", "$text", undefined, "$font-data");
  field(desk, "Spot Price Field", "委托价格", "63,085.00", "USDT", true);
  field(desk, "Spot Quantity Field", "买入数量", "0.015", "BTC", false);
  const percentages = Insert(desk, {
    type: "frame",
    name: "Spot Balance Percentages",
    layout: "horizontal",
    width: "fill_container",
    height: 40,
    gap: 4,
  });
  for (const value of ["0%", "25%", "50%", "75%", "100%"]) {
    const active = value === "50%";
    const item = Insert(percentages, {
      type: "frame",
      name: `Spot Percentage ${value}`,
      layout: "horizontal",
      width: "fill_container",
      height: 40,
      justifyContent: "center",
      alignItems: "center",
      fill: active ? "$mint-soft" : "$surface",
      stroke: active ? "$mint" : "$border",
      strokeWidth: 1,
      strokeAlignment: "inner",
      cornerRadius: 2,
    });
    T(item, `Spot Percentage ${value} Label`, value, 9, active ? "700" : "500", active ? "$mint-strong" : "$muted", undefined, "$font-data");
  }
  field(desk, "Spot Total Field", "预计成交额", "946.28", "USDT", false);
  const submit = Insert(desk, {
    type: "frame",
    name: "Spot Buy Submit",
    layout: "horizontal",
    width: "fill_container",
    height: 54,
    padding: [0, 14],
    gap: 8,
    justifyContent: "center",
    alignItems: "center",
    fill: "$mint",
    stroke: "$mint",
    strokeWidth: 1,
    strokeAlignment: "inner",
    cornerRadius: 4,
  });
  T(submit, "Spot Buy Submit Label", "买入 BTC", 14, "750", "$text");
  I(submit, "Spot Buy Submit Icon", "arrow-up-right", "$text", 18);
  const note = Insert(desk, {
    type: "frame",
    name: "Spot Settlement Note",
    layout: "horizontal",
    width: "fill_container",
    height: 40,
    padding: [8, 0],
    gap: 7,
    alignItems: "center",
    stroke: "$hairline",
    strokeWidth: { top: 1 },
    strokeAlignment: "inner",
  });
  I(note, "Spot Settlement Icon", "shield-check", "$muted", 15);
  T(note, "Spot Settlement Copy", "成交资产直接计入现货账户，提交前请复核价格与数量。", 10, "500", "$muted", 330);
}

function openOrders(parent) {
  const shell = Insert(parent, {
    type: "frame",
    name: "Spot Open Orders",
    layout: "vertical",
    width: "fill_container",
    padding: [14, 16, 8, 16],
    gap: 4,
    fill: "$surface",
  });
  const heading = Insert(shell, {
    type: "frame",
    name: "Spot Open Orders Heading",
    layout: "horizontal",
    width: "fill_container",
    height: 38,
    justifyContent: "space_between",
    alignItems: "center",
  });
  const title = Insert(heading, { type: "frame", name: "Spot Open Orders Title Copy", layout: "vertical", gap: 2 });
  T(title, "Spot Open Orders Eyebrow", "ACCOUNT / OPEN", 9, "650", "$mint-strong", undefined, "$font-data");
  T(title, "Spot Open Orders Title", "当前委托 01", 16, "700", "$text");
  T(heading, "Spot All Orders", "全部委托 ›", 10, "600", "$muted");
  const row = Insert(shell, {
    type: "frame",
    name: "Spot Open Order BTC",
    layout: "horizontal",
    width: "fill_container",
    height: 54,
    justifyContent: "space_between",
    alignItems: "center",
    stroke: "$hairline",
    strokeWidth: { top: 1, bottom: 1 },
    strokeAlignment: "inner",
  });
  const copy = Insert(row, { type: "frame", name: "Spot Open Order Copy", layout: "vertical", gap: 4 });
  T(copy, "Spot Open Order Pair", "BTC/USDT · 限价买入", 11, "650", "$text", undefined, "$font-data");
  T(copy, "Spot Open Order Amount", "0.015 BTC  /  63,085.00 USDT", 9, "500", "$muted", undefined, "$font-data");
  T(row, "Spot Open Order State", "等待成交", 10, "650", "$warning");
}

function navigation(parent) {
  const nav = Insert(parent, {
    type: "frame",
    name: "Bottom Navigation",
    layout: "horizontal",
    width: "fill_container",
    height: 84,
    padding: [10, 8, 8, 8],
    gap: 2,
    fill: "$surface",
    stroke: "$border",
    strokeWidth: { top: 1 },
    strokeAlignment: "inner",
  });
  for (const item of [
    ["首页", "house"], ["行情", "chart-line"], ["现货", "arrow-left-right"],
    ["秒合约", "zap"], ["合约", "activity"], ["资产", "wallet-cards"], ["我的", "user-round"],
  ]) {
    const active = item[0] === "现货";
    const cell = Insert(nav, {
      type: "frame",
      name: `Nav ${item[0]}`,
      layout: "vertical",
      width: "fill_container",
      height: 64,
      gap: 4,
      justifyContent: "center",
      alignItems: "center",
      fill: active ? "$mint-soft" : "$surface",
      cornerRadius: item[0] === "秒合约" ? 20 : 4,
    });
    I(cell, `${item[0]} Icon`, item[1], active ? "$mint-strong" : "$muted", 20);
    T(cell, `${item[0]} Label`, item[0], 10, active ? "650" : "500", active ? "$text" : "$muted");
  }
}

function spot(name, mode) {
  const artboard = screen(name, mode);
  status(artboard);
  header(artboard);
  quote(artboard);
  intervalTools(artboard);
  klineChart(artboard);
  marketTabs(artboard);
  orderBook(artboard);
  orderDesk(artboard);
  openOrders(artboard);
  navigation(artboard);
  Update(artboard, { placeholder: false });
  return artboard;
}

const light = spot("05 / Spot Trading", "light");
const dark = spot("05B / Spot Trading / Dark", "dark");
Print(`LIGHT=${light}`);
Print(`DARK=${dark}`);
