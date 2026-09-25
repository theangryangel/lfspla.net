# lfsplanet_track_gen

Draws one SVG track outline from each Live for Speed path (`.pth`) file. The
output is for chart-picker thumbnails: a centre line with timing markers.

Based on the `pth2svg` example from [insim.rs][insim].

```sh
cargo run -p lfsplanet_track_gen -- \
  --output assets/tracks/ \
  ~/LFS/data/pth/*.pth
```

The site reads track artwork from that directory. See
[docs/development.md](../../docs/development.md).

Each output keeps its input name: `BL1.pth` becomes `BL1.svg`. LFS uses upper
case, matching API codes. Normalise lookups if needed.

Unreadable files do not stop the run. Failures are listed at the end and the
command exits non-zero.

Straight-line paths for open configurations are skipped. This is not a failure,
so you can pass the whole `data/pth` directory.

## What it draws

Both path formats are supported. `LFSPTH` describes circuits and their finish
line. `SRPATH` also gives splits and whether the path loops.

Each track is scaled to fit the thumbnail. The picker shows shape, not length.

Path files contain more points than a thumbnail needs. `--simplify` removes
points without changing the shape. Timing markers stay on the line.

## Colours

Each colour has a theme token and a fallback.

```svg
<path class="track" stroke="var(--muted-foreground, oklch(0.556 0 0))" …/>
```

Inline SVGs use the site's theme. The track uses `--muted-foreground`, the
finish line uses `--destructive`, and splits use `--foreground`. A wider
`--muted` marker cuts through the centre line.

In an `<img>`, the page theme cannot reach the SVG. Fallback colours use the
reader's system theme. Inline SVGs where the theme matters.

[insim]: https://github.com/theangryangel/insim.rs
