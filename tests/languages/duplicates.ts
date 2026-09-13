export function sumPositive(values: number[]): number {
  let total = 0;
  for (const value of values) { if (value > 0) { total += value; } }
  return total;
}
export function addPositive(items: number[]): number {
  let result = 0;
  for (const item of items) { if (item > 0) { result += item; } }
  return result;
}
export const doubled = (value: number) => value * 2;
