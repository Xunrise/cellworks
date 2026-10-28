# 0002. Payments through a hosted provider

Status: Accepted

## Context
Handling card data ourselves would put the shop in PCI scope.

## Decision
Card data goes only to the payment provider. The shop calls `POST /api/payments` and stores the returned reference.
The call is isolated in `checkout/payment.ts`.

## Consequences
- No card data in our code or database.
- Local development simulates the charge when no backend is available.
