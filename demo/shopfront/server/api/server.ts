import { createServer } from 'node:http';
import { route } from '../adapters/http/routes';
import { StripeGateway } from '../adapters/payments/stripeGateway';
import { PostgresOrderRepository } from '../adapters/postgres/orderRepo';

const deps = { orders: new PostgresOrderRepository(), payments: new StripeGateway() };

export function start(port = Number(process.env.PORT ?? 8080)) {
  return createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(chunk as Buffer);
    const raw = Buffer.concat(chunks).toString();
    const reply = await route(deps, req.method ?? 'GET', (req.url ?? '/').split('?')[0], raw ? JSON.parse(raw) : undefined);
    res.writeHead(reply.status, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(reply.body));
  }).listen(port);
}
