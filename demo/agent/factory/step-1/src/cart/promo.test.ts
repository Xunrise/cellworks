import { describe, expect, it } from 'vitest';
import { money } from '../core/money';
import { discountFor, parsePromo } from './promo';

describe('parsePromo', () => {
  it('parses a valid code, ignoring case and whitespace', () => {
    expect(parsePromo(' save10 ')).toEqual({ code: 'SAVE10', percent: 10 });
  });

  it('rejects unknown formats', () => {
    expect(parsePromo('')).toBeNull();
    expect(parsePromo('WELCOME')).toBeNull();
    expect(parsePromo('SAVE')).toBeNull();
  });

  it('rejects percentages outside 5 to 50', () => {
    expect(parsePromo('SAVE4')).toBeNull();
    expect(parsePromo('SAVE51')).toBeNull();
    expect(parsePromo('SAVE50')?.percent).toBe(50);
  });
});

describe('discountFor', () => {
  it('takes the percentage off the subtotal', () => {
    expect(discountFor({ code: 'SAVE10', percent: 10 }, money(5000)).cents).toBe(500);
  });

  it('caps the discount at 100 EUR', () => {
    expect(discountFor({ code: 'SAVE50', percent: 50 }, money(100000)).cents).toBe(10000);
  });

  it('is zero for an empty subtotal', () => {
    expect(discountFor({ code: 'SAVE10', percent: 10 }, money(0)).cents).toBe(0);
  });
});
