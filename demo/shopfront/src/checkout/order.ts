import type { CartItem } from '../cart/cart';
import { type Totals, computeTotals } from '../cart/totals';
import { newId } from '../core/ids';
import { emit } from '../core/events';
import { charge } from './payment';
import { shippingFor } from './shipping';

export interface Order {
  id: string;
  items: CartItem[];
  totals: Totals;
  paymentReference: string;
  placedAt: Date;
}

export async function placeOrder(items: CartItem[]): Promise<Order> {
  if (items.length === 0) throw new Error('Cannot place an order with an empty cart');
  const totals = computeTotals(items, shippingFor(items));
  const payment = await charge(totals.total);
  if (!payment.approved) throw new Error('Payment was declined');
  const order: Order = {
    id: newId('ord'),
    items: items.map((item) => ({ ...item })),
    totals,
    paymentReference: payment.reference,
    placedAt: new Date(),
  };
  emit('order:placed', { orderId: order.id });
  return order;
}
