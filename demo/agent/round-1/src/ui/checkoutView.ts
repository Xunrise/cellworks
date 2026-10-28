import { clearCart, listItems } from '../cart/cart';
import { type Promo, parsePromo } from '../cart/promo';
import { computeTotals } from '../cart/totals';
import { placeOrder } from '../checkout/order';
import { shippingFor } from '../checkout/shipping';
import { on } from '../core/events';
import { format } from '../core/money';

export function mountCheckoutView(root: HTMLElement): void {
  let confirmation = '';
  let promo: Promo | null = null;
  let promoMessage = '';
  let promoError = false;

  function render(): void {
    const items = listItems();
    if (items.length === 0) {
      root.innerHTML = `<h2>Checkout</h2><p class="empty">Add something to start checkout.</p>${confirmation}`;
      return;
    }
    const totals = computeTotals(items, shippingFor(items));
    root.innerHTML = `
      <h2>Checkout</h2>
      <div class="promo">
        <input id="promo-input" placeholder="Promo code" aria-label="Promo code" value="${promo?.code ?? ''}" />
        <button id="apply-promo" class="secondary">Apply</button>
      </div>
      <div class="promo-message${promoError ? ' error' : ''}">${promoMessage}</div>
      <div class="totals">
        <div class="row"><span>Subtotal</span><span>${format(totals.subtotal)}</span></div>
        <div class="row"><span>Shipping</span><span>${format(totals.shipping)}</span></div>
        <div class="row grand"><span>Total</span><span>${format(totals.total)}</span></div>
      </div>
      <div class="checkout-actions"><button id="place-order">Place order</button></div>`;
  }

  root.addEventListener('click', async (event) => {
    const id = (event.target as HTMLElement).id;
    if (id === 'apply-promo') {
      const input = root.querySelector<HTMLInputElement>('#promo-input')!;
      promo = parsePromo(input.value);
      promoError = promo === null;
      promoMessage = promo ? 'Code applied' : 'Invalid promo code';
      render();
      return;
    }
    if (id !== 'place-order') return;
    try {
      const order = await placeOrder(listItems());
      confirmation = `<div class="confirmation">Order ${order.id} placed. Thank you!</div>`;
      promo = null;
      promoMessage = '';
      clearCart();
    } catch (error) {
      confirmation = `<div class="confirmation">${(error as Error).message}</div>`;
      render();
    }
  });
  on('cart:changed', render);
  render();
}
