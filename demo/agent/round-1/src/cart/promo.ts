import { type Money, percentOf } from '../core/money';

export interface Promo {
  code: string;
  percent: number;
}

const PATTERN = /^SAVE(\d{1,2})$/;

/** Parses codes such as `SAVE10` (10 percent off). Returns null for anything else. */
export function parsePromo(input: string): Promo | null {
  const code = input.trim().toUpperCase();
  const match = PATTERN.exec(code);
  if (!match) return null;
  return { code, percent: Number(match[1]) };
}

export function discountFor(promo: Promo, subtotal: Money): Money {
  return percentOf(subtotal, promo.percent);
}
