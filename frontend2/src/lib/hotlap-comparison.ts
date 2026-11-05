import type { Hotlap } from "./api.js";

export type ComparisonLap = Pick<
  Hotlap,
  "lap_time_ms" | "split_1_ms" | "split_2_ms" | "split_3_ms" | "split_4_ms"
>;

/** Splits are cumulative; the last populated slot is the finish time. */
export function intermediateSplits(entry: ComparisonLap): number[] {
  return [
    entry.split_1_ms,
    entry.split_2_ms,
    entry.split_3_ms,
    entry.split_4_ms,
  ]
    .filter((split) => split > 0)
    .slice(0, -1);
}

/** Time between checkpoints, including the final checkpoint-to-finish segment. */
export function sectorTimes(entry: ComparisonLap): number[] {
  const checkpoints = [...intermediateSplits(entry), entry.lap_time_ms];
  return checkpoints.map((time, index) => time - (checkpoints[index - 1] ?? 0));
}

export interface SectorComparison {
  reference: number;
  delta: number;
}

/** Compare complete, monotonic timing sequences with matching checkpoints. */
export function compareSectors(
  lap: ComparisonLap,
  reference: ComparisonLap,
): SectorComparison[] | null {
  function completeSectors(entry: ComparisonLap): number[] | null {
    const slots = [
      entry.split_1_ms,
      entry.split_2_ms,
      entry.split_3_ms,
      entry.split_4_ms,
    ];
    const end = slots.findIndex((time) => time === 0);
    const checkpoints = end === -1 ? slots : slots.slice(0, end);
    if (
      checkpoints.length < 2 ||
      (end !== -1 && slots.slice(end).some((time) => time !== 0)) ||
      checkpoints.at(-1) !== entry.lap_time_ms ||
      checkpoints.some(
        (time, index) =>
          !Number.isSafeInteger(time) || time <= (checkpoints[index - 1] ?? 0),
      )
    )
      return null;
    return checkpoints.map(
      (time, index) => time - (checkpoints[index - 1] ?? 0),
    );
  }
  const own = completeSectors(lap);
  const other = completeSectors(reference);
  if (!own || !other || own.length !== other.length) return null;
  return own.map((time, index) => ({
    reference: other[index],
    delta: time - other[index],
  }));
}

/** Shared map and timing-legend colours; negative gaps are faster sectors. */
export function sectorColour(difference: number | undefined): string {
  return difference === undefined || difference === 0
    ? "var(--muted-foreground)"
    : difference < 0
      ? "var(--time-pb)"
      : "var(--destructive)";
}
