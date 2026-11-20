# Context map

| Upstream | Downstream | Relationship |
|----------|------------|--------------|
| Catalog  | Cart       | Customer/supplier: Cart reads product data and prices |
| Cart     | Checkout   | Customer/supplier: Checkout reads cart items and totals |
| Checkout | Payment provider | Anticorruption layer in `checkout/payment.ts` (ADR-0002) |

Crates of code follow the same direction: `checkout` imports `cart`, `cart` imports `catalog`, all import `core`.
