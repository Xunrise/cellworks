import type { Order } from '../../domain/order';
import type { OrderRepository } from '../../ports/orderRepository';

/** Stands in for the orders table of the shop database; rows are kept in memory. */
export class PostgresOrderRepository implements OrderRepository {
  private readonly rows = new Map<string, Order>();

  async save(order: Order): Promise<void> {
    this.rows.set(order.id, order); // INSERT INTO orders ...
  }

  async find(id: string): Promise<Order | undefined> {
    return this.rows.get(id); // SELECT * FROM orders WHERE id = $1
  }

  async list(): Promise<Order[]> {
    return [...this.rows.values()]; // SELECT * FROM orders ORDER BY placed_at
  }
}
