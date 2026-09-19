// Run from repository root after cargo test --lib. No database or network.
// Compile actual source functions; only surrounding DTO/error types are fixtures.
import assert from 'node:assert/strict'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const root = process.cwd()
const deps = resolve(root, 'target/debug/deps')
function latestLibrary(name) {
  return readdirSync(deps)
    .filter((file) => file.startsWith(`lib${name}-`) && file.endsWith('.rlib'))
    .map((file) => resolve(deps, file))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0]
}
function functionSource(file, name) {
  const text = readFileSync(resolve(root, file), 'utf8')
  const start = text.indexOf(`fn ${name}(`)
  assert.ok(start >= 0, name)
  const open = text.indexOf('{', start)
  let depth = 1
  let end = open + 1
  for (; depth && end < text.length; end++) {
    if (text[end] === '{') depth++
    else if (text[end] === '}') depth--
  }
  assert.equal(depth, 0)
  return text.slice(start, end)
}
const functions = [
  ['src/modules/margin/application/open_position.rs', 'validate_product_margin'],
  ['src/modules/margin/application/open_position.rs', 'validate_open_product_leverage'],
  ['src/modules/margin/application/support.rs', 'validate_positive_decimal'],
  ['src/modules/margin/application/support.rs', 'decimal_matches_string'],
  ['src/modules/margin/application/product_config.rs', 'validate_hourly_interest_rate'],
  ['src/modules/margin/application/product_config.rs', 'validate_decimal_storage'],
  ['src/workers/margin_interest.rs', 'margin_interest_delta'],
  ['src/modules/admin/service/market.rs', 'validate_trading_pair_config'],
].map(([file, name]) => functionSource(file, name)).join('\n')
const source = `
#![allow(dead_code)]
use bigdecimal::BigDecimal;
use std::str::FromStr;
mod modules { pub mod wallet {
  #[derive(Debug, Clone, PartialEq, Eq)] pub struct WalletServiceError;
}}
#[path = ${JSON.stringify(resolve(root, 'src/modules/spot/domain.rs'))}]
mod spot;
#[derive(Debug)] enum AppError { NotFound, Validation(String) }
type AppResult<T> = Result<T, AppError>;
struct Levels(Vec<String>);
struct MarginOpenProductRule {
  status: String, min_margin: BigDecimal, max_margin: Option<BigDecimal>,
  leverage_levels: Levels, hourly_interest_rate: BigDecimal,
}
const MARGIN_RATE_MAX_SCALE: i64 = 8;
const MARGIN_RATE_MAX_INTEGER_DIGITS: usize = 10;
${functions}
fn d(value: &str) -> BigDecimal { BigDecimal::from_str(value).unwrap() }
fn main() {
  let product = MarginOpenProductRule {
    status: "active".into(), min_margin: d("1"), max_margin: Some(d("100")),
    leverage_levels: Levels(vec!["2".into()]), hourly_interest_rate: d("0"),
  };
  let amount = d("1.0000000000000000001");
  validate_positive_decimal(&amount, "margin amount").unwrap();
  validate_product_margin(&amount, &d("2"), &product).unwrap();
  println!("margin request accepted by actual validators: {}", amount);
  for (precision, value) in [(8, "1.00000001"), (10, "1.0000000001")] {
    validate_trading_pair_config(precision as i32, precision as i32, &d("1")).unwrap();
    let pair = spot::TradingPairRule {
      pair_id: "REVIEW".into(), price_precision: precision,
      quantity_precision: precision, min_order_value: d("1"), enabled: true,
    };
    let order = spot::create_limit_order("1", spot::OrderSide::Buy, d(value), d(value), &pair).unwrap();
    let reserved = spot::spot_reservation_amount(order.side, order.price.as_ref().unwrap(), &order.quantity);
    assert!(reserved.normalized().as_bigint_and_exponent().1 > i64::from(precision));
    println!("spot {}+{} digits -> reserve {} (scale {})",
      precision, precision, reserved, reserved.normalized().as_bigint_and_exponent().1);
  }
  let hourly = margin_interest_delta(&d("0.00000000015"), &d("0.00000001"), 1);
  let combined = margin_interest_delta(&d("0.00000000015"), &d("0.00000001"), 2);
  assert_ne!(&hourly + &hourly, combined);
  println!("interest two hourly batches: {}, one two-hour batch: {}", &hourly + &hourly, combined);
}
`
const binary = resolve('/private/tmp', `rust-chain-precision-review-${process.pid}`)
const result = spawnSync('rustc', [
  '--edition=2024', '--crate-name', 'precision_review', '-',
  '-L', `dependency=${deps}`,
  ...['bigdecimal', 'serde', 'chrono'].flatMap((name) => ['--extern', `${name}=${latestLibrary(name)}`]),
  '-o', binary,
], { input: source, encoding: 'utf8', maxBuffer: 2_000_000 })
assert.equal(result.status, 0, result.stderr)
const run = spawnSync(binary, [], { encoding: 'utf8' })
process.stdout.write(run.stdout)
assert.equal(run.status, 0, run.stderr)
