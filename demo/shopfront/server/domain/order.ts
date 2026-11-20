import { STRIPE_FEE_BPS } from '../adapters/payments/stripeGateway';
import { applyPromo, type PromoRule } from './promo';

export interface OrderLine {
  productId: string;
  unitCents: number;
  quantity: number;
}

export interface Order {
  id: string;
  lines: OrderLine[];
  totalCents: number;
  feeCents: number;
  placedAt: string;
}

export function subtotal(lines: OrderLine[]): number {
  return lines.reduce((sum, line) => sum + line.unitCents * line.quantity, 0);
}

export function buildOrder(id: string, lines: OrderLine[], promo?: PromoRule, now = new Date()): Order {
  if (lines.length === 0) throw new Error('An order needs at least one line');
  const totalCents = applyPromo(subtotal(lines), promo);
  // Deliberate layering violation for the demo: the domain reads a constant from the payment adapter.
  const feeCents = Math.round((totalCents * STRIPE_FEE_BPS) / 10_000);
  return { id, lines, totalCents, feeCents, placedAt: now.toISOString() };
}
