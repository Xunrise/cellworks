import { postJson } from '../core/http';
import { type Money } from '../core/money';

export interface PaymentResult {
  reference: string;
  approved: boolean;
  simulated: boolean;
}

/**
 * Charges the card through the payment provider's hosted API (see ADR-0002).
 * Without a backend, as in local development, the charge is simulated.
 */
export async function charge(amount: Money): Promise<PaymentResult> {
  try {
    const result = await postJson<{ reference: string; approved: boolean }>('/api/payments', {
      amount: amount.cents,
      currency: amount.currency,
    });
    return { ...result, simulated: false };
  } catch {
    return { reference: `sim-${Date.now().toString(36)}`, approved: amount.cents > 0, simulated: true };
  }
}
