let counter = 0;

/**
 * Short, readable identifier such as `ord_k3j9x2-1`.
 * The counter keeps ids unique even when called twice in the same millisecond.
 */
export function newId(prefix: string): string {
  counter += 1;
  return `${prefix}_${Date.now().toString(36)}-${counter}`;
}
