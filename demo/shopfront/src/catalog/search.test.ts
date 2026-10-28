import { describe, expect, it } from 'vitest';
import { searchProducts } from './search';

describe('searchProducts', () => {
  it('returns every product for an empty query', () => {
    expect(searchProducts('  ').length).toBeGreaterThanOrEqual(6);
  });

  it('matches by name', () => {
    const names = searchProducts('mug').map((p) => p.name);
    expect(names[0]).toBe('Ceramic Mug');
  });

  it('matches by tag and ranks name matches first', () => {
    const names = searchProducts('coffee').map((p) => p.name);
    expect(names).toContain('Pour-over Kettle');
    expect(names).toContain('Coffee Grinder');
    expect(names[0]).toBe('Coffee Grinder');
  });

  it('returns nothing when no product matches', () => {
    expect(searchProducts('zeppelin')).toEqual([]);
  });
});
