import type { HotLapDiscipline } from "#lib/api.js";

export interface ChartSelection {
  track: string;
  vehicle: string;
}

function key(era: string, discipline?: HotLapDiscipline) {
  return `hotlap-chart:${era}:${discipline ?? "all"}`;
}

export function lastChart(
  era: string,
  discipline?: HotLapDiscipline,
): ChartSelection | null {
  try {
    const value = JSON.parse(
      localStorage.getItem(key(era, discipline)) ?? "null",
    );
    return typeof value?.track === "string" &&
      typeof value?.vehicle === "string"
      ? { track: value.track, vehicle: value.vehicle }
      : null;
  } catch {
    return null;
  }
}

export function rememberChart(
  era: string,
  selection: ChartSelection,
  discipline?: HotLapDiscipline,
) {
  try {
    localStorage.setItem(key(era, discipline), JSON.stringify(selection));
  } catch {
    // Browsing still works when storage is unavailable.
  }
}
