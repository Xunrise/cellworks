import { describe, expect, it } from 'vitest';
import { getProduct } from '../catalog/product';
import type { CartItem } from '../cart/cart';
import { placeOrder } from './order';
import { shippingFor } from './shipping';

const item = (id: string, quantity: number): CartItem => ({ product: getProduct(id)!, quantity });

describe('shippingFor', () => {
  it('is free for an empty cart and from 50 EUR', () => {
    expect(shippingFor([]).cents).toBe(0);
    expect(shippingFor([item('p-004', 1)]).cents).toBe(0);
  });

  it('charges the flat rate below the threshold', () => {
    expect(shippingFor([item('p-001', 1)]).cents).toBe(490);
  });
});

describe('placeOrder', () => {
  it('rejects an empty cart', async () => {
    await expect(placeOrder([])).rejects.toThrow('empty cart');
  });

  it('creates an order with totals including shipping', async () => {
    const order = await placeOrder([item('p-001', 1)]);
    expect(order.id).toMatch(/^ord_/);
    expect(order.totals.total.cents).toBe(1490 + 490);
  });
});
