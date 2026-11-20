# Cart

Holds what the customer intends to buy and computes what it costs.

- Code: `src/cart`
- Responsibilities: item list, quantities, subtotal and totals
- Exposes: `addItem`, `removeItem`, `setQuantity`, `listItems`, `computeTotals`
- Emits: `cart:changed`
