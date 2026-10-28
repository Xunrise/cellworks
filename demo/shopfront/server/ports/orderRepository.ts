import type { Order } from '../domain/order';

export interface OrderRepository {
  save(order: Order): Promise<void>;
  find(id: string): Promise<Order | undefined>;
  list(): Promise<Order[]>;
}
