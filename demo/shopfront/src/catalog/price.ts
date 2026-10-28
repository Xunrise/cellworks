import { type Money, percentOf, sub, times } from '../core/money';
import type { Product } from './product';

const BULK_THRESHOLD = 10;
const BULK_PERCENT = 5;

/** Unit price after the bulk discount for ten or more of the same product. */
export function unitPrice(product: Product, quantity: number): Money {
  if (quantity >= BULK_THRESHOLD) {
    return sub(product.price, percentOf(product.price, BULK_PERCENT));
  }
  return product.price;
}

export function lineTotal(product: Product, quantity: number): Money {
  return times(unitPrice(product, quantity), quantity);
}
