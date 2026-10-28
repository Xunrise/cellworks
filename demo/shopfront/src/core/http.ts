export class HttpError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function parse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    throw new HttpError(response.status, `${response.status} ${response.statusText}`);
  }
  return (await response.json()) as T;
}

export async function getJson<T>(url: string): Promise<T> {
  return parse<T>(await fetch(url, { headers: { Accept: 'application/json' } }));
}

export async function postJson<T>(url: string, body: unknown): Promise<T> {
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(body),
  });
  return parse<T>(response);
}

/** The orders routes of the backend (server/adapters/http/routes.ts). */
export const ORDERS_URL = '/api/orders';

export interface OrderSummary {
  id: string;
  totalCents: number;
}

export const listOrders = () => getJson<OrderSummary[]>(ORDERS_URL);
export const getOrder = (id: string) => getJson<OrderSummary>(`${ORDERS_URL}/${id}`);
export const createOrder = (lines: { productId: string; unitCents: number; quantity: number }[], promo?: string) =>
  postJson<OrderSummary>(ORDERS_URL, { lines, promo });
