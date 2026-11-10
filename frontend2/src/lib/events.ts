import type { EventSummary } from "#lib/api.js";

export function eventHref(event: EventSummary): string {
  if (event.source === "lfs_net" && event.source_url) {
    try {
      const target = new URL(event.source_url);
      if (
        target.protocol === "https:" &&
        (target.hostname === "www.lfs.net" || target.hostname === "lfs.net")
      )
        return target.href;
    } catch {
      // The detail route handles unavailable upstream links.
    }
  }
  return `/events/${event.id}`;
}

/** Only call an event live when its scheduled duration is known. */
export function eventIsLive(
  event: { starts_at: string; ends_at: string | null },
  now = Date.now(),
): boolean {
  return (
    event.ends_at !== null &&
    Date.parse(event.starts_at) <= now &&
    now < Date.parse(event.ends_at)
  );
}
