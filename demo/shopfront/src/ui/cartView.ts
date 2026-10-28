import { listItems, removeItem } from '../cart/cart';
import { computeSubtotal } from '../cart/totals';
import { lineTotal } from '../catalog/price';
import { on } from '../core/events';
import { format } from '../core/money';

export function mountCartView(root: HTMLElement): void {
  function render(): void {
    const items = listItems();
    if (items.length === 0) {
      root.innerHTML = '<h2>Cart</h2><p class="empty">Your cart is empty.</p>';
      return;
    }
    root.innerHTML = `
      <h2>Cart</h2>
      ${items
        .map(
          ({ product, quantity }) => `
        <div class="cart-line">
          <span>${product.name} <span class="qty">x ${quantity}</span></span>
          <span>${format(lineTotal(product, quantity))}
            <button class="secondary" data-remove="${product.id}" aria-label="Remove ${product.name}">x</button>
          </span>
        </div>`,
        )
        .join('')}
      <div class="cart-line"><strong>Subtotal</strong><strong>${format(computeSubtotal(items))}</strong></div>`;
  }

  root.addEventListener('click', (event) => {
    const id = (event.target as HTMLElement).dataset.remove;
    if (id) removeItem(id);
  });
  on('cart:changed', render);
  render();
}
