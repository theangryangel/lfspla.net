/**
 * The few SPR header fields a browser can read for itself.
 *
 * The header is fixed-layout, so these sit at known offsets, pinned on the Rust
 * side by `the_fields_a_client_reads_stay_at_their_documented_offsets` in
 * `crates/lfsplanet_spr/src/header.rs`. Reading them lets the uploader refuse
 * an obvious mistake before transferring a large replay.
 *
 * Anything needing judgement stays with the API - the era version comparator,
 * mod vehicle resolution, HLVC - and the API validates all of this again
 * regardless. Nothing here is a security boundary.
 */

/** Enough of the file to cover the last field read below. */
const PREFIX_BYTES = 28;

export interface SprSummary {
  /** LFS version the replay was recorded with, such as `0.7E`. */
  version: string;
  /** Canonical track configuration code, such as `BL2R`. */
  track: string;
}

/**
 * Reads the header fields, or `null` when the file is not an SPR at all.
 *
 * A file too short to hold a header, or without the `LFSSPR` signature, is not
 * a replay whatever its name says.
 */
export async function readSprHeader(file: File): Promise<SprSummary | null> {
  const bytes = new Uint8Array(await file.slice(0, PREFIX_BYTES).arrayBuffer());
  if (bytes.length < PREFIX_BYTES) return null;

  // Each field is NUL-terminated ASCII within its fixed width.
  const text = (from: number, to: number) => {
    const field = bytes.subarray(from, to);
    const end = field.indexOf(0);
    return String.fromCharCode(
      ...(end === -1 ? field : field.subarray(0, end)),
    ).trim();
  };

  if (text(0, 6) !== "LFSSPR") return null;
  return { version: text(16, 24), track: text(24, 28) };
}
