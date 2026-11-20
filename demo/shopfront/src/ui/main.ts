import './styles.css';
import { mountCartView } from './cartView';
import { mountCatalogView } from './catalogView';
import { mountCheckoutView } from './checkoutView';

function mount(id: string, fn: (el: HTMLElement) => void): void {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Missing #${id} container`);
  fn(el);
}

mount('catalog', mountCatalogView);
mount('cart', mountCartView);
mount('checkout', mountCheckoutView);
