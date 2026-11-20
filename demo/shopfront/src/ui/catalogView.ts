import { addItem } from '../cart/cart';
import { unitPrice } from '../catalog/price';
import { searchProducts } from '../catalog/search';
import { format } from '../core/money';

export function mountCatalogView(root: HTMLElement): void {
  root.innerHTML = `
    <h2>Catalog</h2>
    <input type="search" placeholder="Search products" aria-label="Search products" />
    <div class="grid"></div>`;
  const input = root.querySelector('input')!;
  const grid = root.querySelector('.grid')!;

  function render(): void {
    const products = searchProducts(input.value);
    if (products.length === 0) {
      grid.innerHTML = '<p class="empty">No products match your search.</p>';
      return;
    }
    grid.innerHTML = products
      .map(
        (product) => `
        <article class="card">
          <div class="swatch"></div>
          <h3>${product.name}</h3>
          <p>${product.description}</p>
          <span class="price">${format(unitPrice(product, 1))}</span>
          <button data-id="${product.id}">Add to cart</button>
        </article>`,
      )
      .join('');
  }

  grid.addEventListener('click', (event) => {
    const id = (event.target as HTMLElement).closest('button')?.dataset.id;
    const product = id ? searchProducts('').find((p) => p.id === id) : undefined;
    if (product) addItem(product);
  });
  input.addEventListener('input', render);
  render();
}
