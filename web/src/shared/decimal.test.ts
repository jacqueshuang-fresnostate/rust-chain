import { describe, expect, it } from 'vitest';
import { decimalFitsPrecision, decimalFitsStorage, requiredDecimalText, formatDecimalText } from './decimal';
import { parseSafeInteger } from './integer';

describe('exact decimal precision validation', () => {
  it.each([
    ['1.2300', 2, true], ['1.231', 2, false], ['1.000', 0, true], ['0.1', 0, false],
    ['1e-8', 8, true], ['1e-9', 8, false], ['0.99999999', 8, true], ['0.999999999', 8, false],
    ['0.000000000000000001', 18, true], ['1e-19', 18, false], ['1000e-3', 0, true],
    ['NaN', 2, false], ['', 2, false], ['1', -1, false], ['1', 19, false], ['1', 1.5, false]
  ] as const)('checks %s at precision %s without rounding', (value, precision, fits) => {
    expect(decimalFitsPrecision(value, precision)).toBe(fits);
  });
  it('does not guess missing or invalid precision', () => {
    expect(decimalFitsPrecision('1', undefined)).toBe(false);
    expect(decimalFitsPrecision('1', Number.NaN)).toBe(false);
  });
});

describe('storage and safe integer boundaries', () => {
  it.each([
    ['1e-18', true], ['1.0000000000000000000', true],
    ['9007199254740993.000000000000000001', true],
    ['99999999999999999999.999999999999999999', true],
    ['100000000000000000000', false], ['-100000000000000000000', false],
    ['1e-19', false], ['1e1000000', false], ['1e-1000000', false], ['NaN', false],
    ['Infinity', false], ['', false], ['0e9999', false], ['0e-257', false],
    ['0'.repeat(257), false]
  ])('validates DECIMAL(38,18) %s', (value, expected) => {
    expect(decimalFitsStorage(value)).toBe(expected);
  });
  it('preserves exact source text and validates narrower rates', () => {
    expect(requiredDecimalText(' 9007199254740993.000000000000000001 ', '金额')).toBe('9007199254740993.000000000000000001');
    expect(decimalFitsStorage('0.1234567800', 18, 8)).toBe(true);
    expect(decimalFitsStorage('0.123456789', 18, 8)).toBe(false);
    expect(decimalFitsStorage('10000000000', 18, 8)).toBe(false);
    expect(decimalFitsStorage(1)).toBe(false);
    expect(decimalFitsStorage(JSON.parse('0.123456789123456789'))).toBe(false);
    expect(() => requiredDecimalText(JSON.parse('0.123456789123456789'), '金额')).toThrow();
    expect(formatDecimalText(0.5)).toBe('0.50');
    expect(formatDecimalText(Number.MAX_SAFE_INTEGER + 1)).toBeNull();
    expect(formatDecimalText('1e-18')).not.toBe('0.00');
  });
  it.each(['9007199254740993', '9007199254740991.1', '1.000000000000000001', '1e3', '0x10', '12junk', 'Infinity', 'NaN', ''])(
    'does not round or truncate integer input %s', (value) => expect(parseSafeInteger(value)).toBeNull()
  );
  it('allows exact safe integer limits and rejects overflow', () => {
    expect(parseSafeInteger('9007199254740991')).toBe(Number.MAX_SAFE_INTEGER);
    expect(parseSafeInteger('4294967295', 0, 4294967295)).toBe(4294967295);
    expect(parseSafeInteger('4294967296', 0, 4294967295)).toBeNull();
  });
});
