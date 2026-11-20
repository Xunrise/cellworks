import type { PaymentGateway } from '../../ports/paymentGateway';

export const STRIPE_FEE_BPS = 150;

/** Stands in for the payment provider's hosted API; the secret comes from the environment. */
export class StripeGateway implements PaymentGateway {
  constructor(private readonly secret: string = 'sk_test_placeholder') {}

  async charge(amountCents: number, _currency: string): Promise<{ reference: string; approved: boolean }> {
    return { reference: `ch_${this.secret.slice(-4)}_${amountCents}`, approved: amountCents > 0 };
  }
}
