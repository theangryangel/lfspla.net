import type { BestHotlapResponse } from "./api.js";

export type ComparisonLap = Pick<
  BestHotlapResponse,
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
