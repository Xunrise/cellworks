import { clearCart, listItems } from '../cart/cart';
import { computeTotals } from '../cart/totals';
import { placeOrder } from '../checkout/order';
import { shippingFor } from '../checkout/shipping';
import { on } from '../core/events';
import { format } from '../core/money';

export function mountCheckoutView(root: HTMLElement): void {
  let confirmation = '';

  function render(): void {
    const items = listItems();
    if (items.length === 0) {
      root.innerHTML = `<h2>Checkout</h2><p class="empty">Add something to start checkout.</p>${confirmation}`;
      return;
    }
    const totals = computeTotals(items, shippingFor(items));
    root.innerHTML = `
      <h2>Checkout</h2>
      <div class="totals">
        <div class="row"><span>Subtotal</span><span>${format(totals.subtotal)}</span></div>
        <div class="row"><span>Shipping</span><span>${format(totals.shipping)}</span></div>
        <div class="row grand"><span>Total</span><span>${format(totals.total)}</span></div>
      </div>
      <div class="checkout-actions"><button id="place-order">Place order</button></div>`;
  }

  root.addEventListener('click', async (event) => {
    if ((event.target as HTMLElement).id !== 'place-order') return;
    try {
      const order = await placeOrder(listItems());
      confirmation = `<div class="confirmation">Order ${order.id} placed. Thank you!</div>`;
      clearCart();
    } catch (error) {
      confirmation = `<div class="confirmation">${(error as Error).message}</div>`;
      render();
    }
  });
  on('cart:changed', render);
  render();
}
