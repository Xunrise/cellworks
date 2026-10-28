/** Money is held as integer minor units (cents) to avoid floating point drift. */
export interface Money {
  readonly cents: number;
  readonly currency: string;
}

export const CURRENCY = 'EUR';

export function money(cents: number, currency: string = CURRENCY): Money {
  return { cents: Math.round(cents), currency };
}

export function zero(currency: string = CURRENCY): Money {
  return money(0, currency);
}

function assertSameCurrency(a: Money, b: Money): void {
  if (a.currency !== b.currency) {
    throw new Error(`Currency mismatch: ${a.currency} vs ${b.currency}`);
  }
}

export function add(a: Money, b: Money): Money {
  assertSameCurrency(a, b);
  return money(a.cents + b.cents, a.currency);
}

export function sub(a: Money, b: Money): Money {
  assertSameCurrency(a, b);
  return money(a.cents - b.cents, a.currency);
}

export function times(a: Money, quantity: number): Money {
  return money(a.cents * quantity, a.currency);
}

/** `percent` of an amount, rounded to the nearest cent. */
export function percentOf(a: Money, percent: number): Money {
  return money((a.cents * percent) / 100, a.currency);
}

export function min(a: Money, b: Money): Money {
  assertSameCurrency(a, b);
  return a.cents <= b.cents ? a : b;
}

export function max(a: Money, b: Money): Money {
  assertSameCurrency(a, b);
  return a.cents >= b.cents ? a : b;
}

export function format(a: Money): string {
  return new Intl.NumberFormat('en-IE', { style: 'currency', currency: a.currency }).format(
    a.cents / 100,
  );
}
