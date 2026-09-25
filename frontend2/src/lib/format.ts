export function lapTime(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "-";
  const minutes = Math.floor(ms / 60000);
  const seconds = String(Math.floor(ms / 1000) % 60).padStart(2, "0");
  return `${minutes}:${seconds}.${String(ms % 1000).padStart(3, "0")}`;
}

/** Format signed gaps; zero is meaningful. */
export function delta(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "-";
  const sign = ms > 0 ? "+" : ms < 0 ? "−" : "";
  return sign + lapTime(Math.abs(ms));
}

export function fileSize(bytes: number): string {
  const mebibyte = 1024 * 1024;
  return bytes < mebibyte
    ? `${(bytes / 1024).toFixed(1)} KiB`
    : `${(bytes / mebibyte).toFixed(1)} MiB`;
}

export function dateTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

export const hotlapStates: Record<
  string,
  {
    label: string;
    variant: "default" | "secondary" | "outline" | "destructive";
  }
> = {
  pending: { label: "Pending", variant: "outline" },
  valid: { label: "Published", variant: "secondary" },
  invalid: { label: "Rejected", variant: "destructive" },
};

export const steeringLabels: Record<string, string> = {
  wheel: "Wheel",
  mouse: "Mouse",
  keyboard: "Keyboard",
  keyboard_stabilised: "Keyboard (stabilised)",
};

export function pageItems<T>(items: T[], requested: number, size = 50) {
  const pages = Math.max(1, Math.ceil(items.length / size));
  const page = Math.min(
    pages,
    Math.max(1, Number.isFinite(requested) ? Math.floor(requested) : 1),
  );
  return {
    items: items.slice((page - 1) * size, page * size),
    pagination: {
      page,
      per_page: size,
      total_items: items.length,
      total_pages: pages,
    },
  };
}

/** Format reported controls. Unknown legacy flags are omitted. */
export function controlTags(hotlap: {
  brake_help_enabled: boolean;
  automatic_gears: boolean;
  manual_shifter: boolean | null;
  axis_clutch: boolean;
  automatic_clutch: boolean;
  driver_side: string;
  abs_enabled: boolean | null;
}): string[] {
  const tags = [hotlap.automatic_gears ? "Auto gears" : "Manual gears"];
  if (hotlap.manual_shifter) tags.push("H-shifter");
  if (hotlap.axis_clutch) tags.push("Axis clutch");
  if (hotlap.automatic_clutch) tags.push("Auto clutch");
  if (hotlap.brake_help_enabled) tags.push("Brake help");
  if (hotlap.abs_enabled) tags.push("ABS");
  tags.push(hotlap.driver_side === "left" ? "LHD" : "RHD");
  return tags;
}
