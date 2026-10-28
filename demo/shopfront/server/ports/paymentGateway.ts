export interface PaymentGateway {
  charge(amountCents: number, currency: string): Promise<{ reference: string; approved: boolean }>;
}
