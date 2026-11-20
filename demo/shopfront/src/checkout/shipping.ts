import { lineTotal } from '../catalog/price';
import type { CartItem } from '../cart/cart';
import { type Money, add, money, zero } from '../core/money';

const FREE_SHIPPING_FROM = money(5000);
const FLAT_RATE = money(490);

/** Flat rate, free from 50 EUR, nothing for an empty cart. */
export function shippingFor(items: CartItem[]): Money {
  if (items.length === 0) return zero();
  const subtotal = items.reduce((sum, item) => add(sum, lineTotal(item.product, item.quantity)), zero());
  return subtotal.cents >= FREE_SHIPPING_FROM.cents ? zero() : FLAT_RATE;
}
