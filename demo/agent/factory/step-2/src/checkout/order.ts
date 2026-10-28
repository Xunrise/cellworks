import type { CartItem } from '../cart/cart';
import { discountFor, parsePromo } from '../cart/promo';
import { type Totals, computeTotals } from '../cart/totals';
import { newId } from '../core/ids';
import { emit } from '../core/events';
import { type Money, max, sub, zero } from '../core/money';
import { charge } from './payment';
import { shippingFor } from './shipping';

export interface Order {
  id: string;
  items: CartItem[];
  totals: Totals;
  /** Promo code applied to this order, if any. */
  promoCode?: string;
  /** Amount taken off the order by the promo code. */
  discount: Money;
  paymentReference: string;
  placedAt: Date;
}

export async function placeOrder(items: CartItem[], promoCode?: string): Promise<Order> {
  if (items.length === 0) throw new Error('Cannot place an order with an empty cart');
  const base = computeTotals(items, shippingFor(items));
  const promo = promoCode ? parsePromo(promoCode) : null;
  const discount = promo ? discountFor(promo, base.subtotal) : zero();
  const totals: Totals = { ...base, total: max(zero(), sub(base.total, discount)) };
  const payment = await charge(totals.total);
  if (!payment.approved) throw new Error('Payment was declined');
  const order: Order = {
    id: newId('ord'),
    items: items.map((item) => ({ ...item })),
    totals,
    promoCode: promo?.code,
    discount,
    paymentReference: payment.reference,
    placedAt: new Date(),
  };
  emit('order:placed', { orderId: order.id });
  return order;
}
