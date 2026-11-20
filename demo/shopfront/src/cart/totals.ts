import { lineTotal } from '../catalog/price';
import { type Money, add, zero } from '../core/money';
import type { CartItem } from './cart';

export interface Totals {
  subtotal: Money;
  shipping: Money;
  total: Money;
}

export function computeSubtotal(items: CartItem[]): Money {
  return items.reduce((sum, item) => add(sum, lineTotal(item.product, item.quantity)), zero());
}

export function computeTotals(items: CartItem[], shipping: Money): Totals {
  const subtotal = computeSubtotal(items);
  return { subtotal, shipping, total: add(subtotal, shipping) };
}
