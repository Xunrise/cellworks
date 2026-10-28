import { beforeEach, describe, expect, it } from 'vitest';
import { getProduct } from '../catalog/product';
import { money } from '../core/money';
import { addItem, clearCart, itemCount, listItems, removeItem, setQuantity } from './cart';
import { computeTotals } from './totals';

const mug = getProduct('p-001')!;
const lamp = getProduct('p-002')!;

describe('cart', () => {
  beforeEach(() => clearCart());

  it('merges repeated additions of the same product', () => {
    addItem(mug);
    addItem(mug, 2);
    expect(listItems()).toHaveLength(1);
    expect(itemCount()).toBe(3);
  });

  it('removes items and ignores unknown ids', () => {
    addItem(mug);
    removeItem('nope');
    removeItem(mug.id);
    expect(listItems()).toEqual([]);
  });

  it('drops an item when its quantity is set to zero', () => {
    addItem(lamp, 2);
    setQuantity(lamp.id, 0);
    expect(itemCount()).toBe(0);
  });
});

describe('totals', () => {
  beforeEach(() => clearCart());

  it('adds subtotal and shipping', () => {
    addItem(mug, 2);
    addItem(lamp);
    const totals = computeTotals(listItems(), money(490));
    expect(totals.subtotal.cents).toBe(2 * 1490 + 3900);
    expect(totals.total.cents).toBe(totals.subtotal.cents + 490);
  });

  it('applies the bulk price from ten units', () => {
    addItem(mug, 10);
    const totals = computeTotals(listItems(), money(0));
    expect(totals.subtotal.cents).toBe(10 * 1415);
  });
});
