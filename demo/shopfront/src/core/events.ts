/** Application events and their payloads. */
export interface AppEvents {
  'cart:changed': undefined;
  'order:placed': { orderId: string };
}

type Handler<K extends keyof AppEvents> = (payload: AppEvents[K]) => void;

const handlers: { [K in keyof AppEvents]?: Handler<K>[] } = {};

export function on<K extends keyof AppEvents>(event: K, handler: Handler<K>): () => void {
  const list = (handlers[event] ??= []) as Handler<K>[];
  list.push(handler);
  return () => {
    const index = list.indexOf(handler);
    if (index >= 0) list.splice(index, 1);
  };
}

export function emit<K extends keyof AppEvents>(event: K, payload: AppEvents[K]): void {
  const list = (handlers[event] ?? []) as Handler<K>[];
  for (const handler of [...list]) handler(payload);
}
