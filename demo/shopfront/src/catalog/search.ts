import { type Product, listProducts } from './product';

function tokens(text: string): string[] {
  return text.toLowerCase().split(/\s+/).filter(Boolean);
}

function score(product: Product, terms: string[]): number {
  const name = product.name.toLowerCase();
  let total = 0;
  for (const term of terms) {
    if (name.includes(term)) total += 3;
    if (product.tags.some((tag) => tag.includes(term))) total += 2;
    if (product.description.toLowerCase().includes(term)) total += 1;
  }
  return total;
}

/** Products matching every term loosely, best match first. Empty query returns all. */
export function searchProducts(query: string, source: Product[] = listProducts()): Product[] {
  const terms = tokens(query);
  if (terms.length === 0) return source;
  return source
    .map((product) => ({ product, score: score(product, terms) }))
    .filter((entry) => entry.score > 0)
    .sort((a, b) => b.score - a.score || a.product.name.localeCompare(b.product.name))
    .map((entry) => entry.product);
}
