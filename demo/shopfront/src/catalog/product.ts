import { type Money, money } from '../core/money';

export interface Product {
  id: string;
  name: string;
  description: string;
  tags: string[];
  price: Money;
}

const PRODUCTS: Product[] = [
  { id: 'p-001', name: 'Ceramic Mug', description: 'Hand-glazed 350 ml mug.', tags: ['kitchen', 'mug'], price: money(1490) },
  { id: 'p-002', name: 'Desk Lamp', description: 'Dimmable LED lamp with USB port.', tags: ['desk', 'light'], price: money(3900) },
  { id: 'p-003', name: 'Notebook A5', description: 'Dotted paper, 160 pages.', tags: ['desk', 'paper'], price: money(990) },
  { id: 'p-004', name: 'Pour-over Kettle', description: 'Gooseneck kettle, 0.9 l.', tags: ['kitchen', 'coffee'], price: money(5400) },
  { id: 'p-005', name: 'Wool Desk Mat', description: 'Felt mat, 80 x 40 cm.', tags: ['desk', 'mat'], price: money(2900) },
  { id: 'p-006', name: 'Coffee Grinder', description: 'Manual burr grinder.', tags: ['kitchen', 'coffee'], price: money(4500) },
];

export function listProducts(): Product[] {
  return PRODUCTS;
}

export function getProduct(id: string): Product | undefined {
  return PRODUCTS.find((product) => product.id === id);
}
