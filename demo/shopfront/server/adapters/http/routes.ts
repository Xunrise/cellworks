import { randomUUID } from 'node:crypto';
import { buildOrder, type OrderLine } from '../../domain/order';
import { findPromo } from '../../domain/promo';
import type { OrderRepository } from '../../ports/orderRepository';
import type { PaymentGateway } from '../../ports/paymentGateway';

export interface Reply {
  status: number;
  body: unknown;
}

export interface Deps {
  orders: OrderRepository;
  payments: PaymentGateway;
}

/** GET /api/orders, GET /api/orders/:id and POST /api/orders: what src/core/http.ts calls. */
export async function route(deps: Deps, method: string, path: string, body: unknown): Promise<Reply> {
  if (path === '/api/orders' && method === 'GET') {
    return { status: 200, body: await deps.orders.list() };
  }
  if (path === '/api/orders' && method === 'POST') {
    const { lines, promo } = body as { lines: OrderLine[]; promo?: string };
    const order = buildOrder(randomUUID(), lines, promo ? findPromo(promo) : undefined);
    const payment = await deps.payments.charge(order.totalCents, 'EUR');
    if (!payment.approved) return { status: 402, body: { error: 'Payment was declined' } };
    await deps.orders.save(order);
    return { status: 201, body: { ...order, paymentReference: payment.reference } };
  }
  const match = /^\/api\/orders\/([\w-]+)$/.exec(path);
  if (match && method === 'GET') {
    const order = await deps.orders.find(match[1]);
    return order ? { status: 200, body: order } : { status: 404, body: { error: 'Not found' } };
  }
  return { status: 404, body: { error: 'Not found' } };
}
