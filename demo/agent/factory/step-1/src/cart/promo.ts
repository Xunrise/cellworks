import { type Money, min, money, percentOf } from '../core/money';

export interface Promo {
  code: string;
  percent: number;
}

const PATTERN = /^SAVE(\d{1,2})$/;
export const MIN_PERCENT = 5;
export const MAX_PERCENT = 50;
/** No single code may take more than 100 EUR off an order. */
export const MAX_DISCOUNT = money(10000);

/**
 * Parses codes such as `SAVE10` (10 percent off), ignoring case and surrounding whitespace.
 * Returns null for unknown formats and for percentages outside 5 to 50.
 */
export function parsePromo(input: string): Promo | null {
  const code = input.trim().toUpperCase();
  const match = PATTERN.exec(code);
  if (!match) return null;
  const percent = Number(match[1]);
  if (percent < MIN_PERCENT || percent > MAX_PERCENT) return null;
  return { code, percent };
}

/** The discount for a subtotal, capped at `MAX_DISCOUNT` and never below zero. */
export function discountFor(promo: Promo, subtotal: Money): Money {
  if (subtotal.cents <= 0) return money(0, subtotal.currency);
  return min(percentOf(subtotal, promo.percent), MAX_DISCOUNT);
}
