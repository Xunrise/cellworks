export type PromoRule =
  | { kind: 'percent'; code: string; percent: number }
  | { kind: 'fixed'; code: string; cents: number };

const RULES: PromoRule[] = [
  { kind: 'percent', code: 'SPRING10', percent: 10 },
  { kind: 'fixed', code: 'WELCOME5', cents: 500 },
];

export function findPromo(code: string): PromoRule | undefined {
  return RULES.find((rule) => rule.code === code.trim().toUpperCase());
}

/** Never goes below zero. */
export function applyPromo(totalCents: number, rule?: PromoRule): number {
  if (!rule) return totalCents;
  const off = rule.kind === 'percent' ? Math.round((totalCents * rule.percent) / 100) : rule.cents;
  return Math.max(0, totalCents - off);
}
