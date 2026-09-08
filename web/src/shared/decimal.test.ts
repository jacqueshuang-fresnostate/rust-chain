import { describe, expect, it } from 'vitest';
import { decimalFitsPrecision } from './decimal';

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
