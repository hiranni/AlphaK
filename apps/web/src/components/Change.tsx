import { changeClass, pct } from "../format";

export function Change({ value, points = false }: { value: number | null | undefined; points?: boolean }) {
  return <span className={`change ${changeClass(value)}`}>{points ? value == null ? "— 点" : `${value > 0 ? "+" : ""}${value.toFixed(2)} 点` : pct(value)}</span>;
}
