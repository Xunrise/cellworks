# Shopfront

A small web shop: product grid with search, a cart and a checkout panel with totals.
Vanilla TypeScript on Vite, no UI framework (see ADR-0001).

## Run

```
npm install
npm run dev      # http://localhost:5173
npm test
npm run build
```

## Layout

- `src/core` - money, ids, events, http helpers
- `src/catalog` - products, search, pricing
- `src/cart` - cart state and totals
- `src/checkout` - shipping, payment, orders
- `src/ui` - DOM views and styles
- `docs` - domain model, architecture decisions
- `infra` - Terraform and Kubernetes manifests
