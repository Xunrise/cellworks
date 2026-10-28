import { lineTotal } from '../catalog/price';
import { type Money, add, max, sub, zero } from '../core/money';
import type { CartItem } from './cart';

export interface Totals {
  subtotal: Money;
  shipping: Money;
  discount: Money;
  /** What the total would be without the discount. */
  totalBeforeDiscount: Money;
  total: Money;
}

export function computeSubtotal(items: CartItem[]): Money {
  return items.reduce((sum, item) => add(sum, lineTotal(item.product, item.quantity)), zero());
}

export function computeTotals(items: CartItem[], shipping: Money, discount: Money = zero()): Totals {
  const subtotal = computeSubtotal(items);
  const totalBeforeDiscount = add(subtotal, shipping);
  return {
    subtotal,
    shipping,
    discount,
    totalBeforeDiscount,
    total: max(zero(), sub(totalBeforeDiscount, discount)),
  };
}
