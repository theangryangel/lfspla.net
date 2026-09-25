/**
 * Artwork indexed by catalogue code.
 *
 * Inline SVGs inherit theme colours. Other formats use image URLs. Missing
 * artwork uses a placeholder.
 */
const markup = import.meta.glob("$assets/tracks/*.svg", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const urls = import.meta.glob("$assets/tracks/*.{png,jpg,webp,avif}", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

/** Which catalogue a piece of artwork belongs to. */
export type ArtworkKind = "track" | "vehicle";

/** Artwork to inline, or artwork to point an `<img>` at. */
export type Artwork = { markup: string } | { url: string };

const artwork: Record<ArtworkKind, Record<string, Artwork>> = {
  track: {},
  vehicle: {},
};

/**
 * Indexes files by catalogue and code.
 */
function index(files: Record<string, string>, wrap: (file: string) => Artwork) {
  for (const [path, file] of Object.entries(files)) {
    const name = path.split("/").at(-1);
    if (!name) continue;
    artwork.track[name.replace(/\.[^.]+$/, "").toUpperCase()] = wrap(file);
  }
}

index(markup, (file) => ({ markup: file }));
index(urls, (file) => ({ url: file }));

/** The artwork for one code, or `undefined` when there is none. */
export function artworkFor(
  kind: ArtworkKind,
  code: string,
): Artwork | undefined {
  return artwork[kind][code.toUpperCase()];
}
