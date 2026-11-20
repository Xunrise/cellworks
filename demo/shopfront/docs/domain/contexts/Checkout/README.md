# Checkout

Turns a cart into a paid order.

- Code: `src/checkout`
- Responsibilities: shipping cost, payment through the provider, order creation
- Exposes: `shippingFor`, `charge`, `placeOrder`
- Emits: `order:placed`
