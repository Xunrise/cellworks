import type { Product } from '../catalog/product';
import { emit } from '../core/events';

export interface CartItem {
  product: Product;
  quantity: number;
}

const items = new Map<string, CartItem>();

export function addItem(product: Product, quantity = 1): void {
  const existing = items.get(product.id);
  if (existing) {
    existing.quantity += quantity;
  } else {
    items.set(product.id, { product, quantity });
  }
  emit('cart:changed', undefined);
}

export function removeItem(productId: string): void {
  if (items.delete(productId)) emit('cart:changed', undefined);
}

export function setQuantity(productId: string, quantity: number): void {
  const item = items.get(productId);
  if (!item) return;
  if (quantity <= 0) {
    items.delete(productId);
  } else {
    item.quantity = quantity;
  }
  emit('cart:changed', undefined);
}

export function listItems(): CartItem[] {
  return [...items.values()];
}

export function itemCount(): number {
  return listItems().reduce((sum, item) => sum + item.quantity, 0);
}

export function clearCart(): void {
  items.clear();
  emit('cart:changed', undefined);
}
