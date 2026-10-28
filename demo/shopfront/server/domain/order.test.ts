import { describe, expect, it } from 'vitest';
import { buildOrder } from './order';
import { applyPromo, findPromo } from './promo';

describe('promo rules', () => {
  it('applies a percentage and never goes below zero', () => {
    expect(applyPromo(2000, findPromo('spring10'))).toBe(1800);
    expect(applyPromo(300, findPromo('WELCOME5'))).toBe(0);
  });
});

describe('buildOrder', () => {
  it('totals the lines after the promo', () => {
    const order = buildOrder('ord_1', [{ productId: 'p-001', unitCents: 1000, quantity: 2 }], findPromo('SPRING10'));
    expect(order.totalCents).toBe(1800);
  });
});
