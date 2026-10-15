# Catalogue seeding

Catalogue definitions are versioned YAML documents under `assets/tracks/`,
`assets/vehicles/`, and `assets/eras/`. PostgreSQL is the source of truth at
runtime; startup does not apply these files.

## Applying definitions

```sh
lfsplanet migrate
lfsplanet seed assets/tracks/*.yaml assets/vehicles/*.yaml assets/eras/*.yaml --dry-run
lfsplanet seed assets/tracks/*.yaml assets/vehicles/*.yaml assets/eras/*.yaml
```

`seed` accepts one or more explicit YAML file paths, including shell-expanded
globs. Each document requires `kind: Location`, `kind: Vehicle`, or `kind: Era`.
Untagged YAML and stdin are rejected. All supplied files are parsed and validated
before application, and database changes are applied in one transaction.

Omitted definitions leave stored data untouched. Changing an existing era
requires `--yes` and may remove laps outside its new combinations; see
[eras and rankings](eras.md) for policy and management commands. New eras need
no confirmation.

`--dry-run` validates proposed rows inside a transaction and rolls it back,
without uploading artwork or rebuilding personal bests or badges. PostgreSQL
sequences may advance. A configured, migrated database is required. Referenced
LFS installations must already exist; canonical tracks and standard vehicles
are populated when their definitions are seeded.

Seeding makes no LFS API calls and needs no LFS OAuth credentials.
`maintenance sync-mods` separately refreshes Vehicle Mods and their covers from
the LFS API, using configured OAuth credentials.

## Locations and track artwork

Each location document owns its shared metadata and nested track configurations.
Definitions and generated outlines live together in `assets/tracks/`:

```yaml
kind: Location
id: BL
name: Blackwood
sequence: 0
# Optional shared location metadata:
# description: ...
# image: ./images/blackwood.webp
tracks:
  - id: BL1
    outline: ./BL1.svg
    # Optional thumbnail, displayed ahead of the outline:
    # image: ./images/blackwood-gp.webp
  - id: BL2
    outline: ./BL2.svg
```

```sh
lfsplanet migrate
lfsplanet seed assets/tracks/*.yaml --dry-run
lfsplanet seed assets/tracks/*.yaml
```

Artwork paths, including nested track paths, are relative to the location YAML
file. SVG, PNG, JPEG, WebP, and AVIF are supported. Track IDs use
`insim_core::Track`. YAML nesting determines each track's location; the track code
does not restrict which location can contain it. Move a track entry to another
location document and reapply to update its stored location. Duplicate locations
or track IDs are rejected across the input batch, so remove the entry from its
previous document when moving it. Standalone `kind: Track`
documents are no longer accepted.

Location rows store their name, optional description and image, and display
sequence. Omitted locations, nested tracks, and optional metadata preserve stored
data. A supplied location name replaces the stored name. Each supplied track
persists its own canonical metadata without changing existing attachment references.
Tracks without artwork still need an explicit nested `id` entry; seeding does not
populate omitted tracks or vehicles. Identical content preserves the current
attachment ID and public URL. Dry runs read and validate assets without uploading.

List stored locations with their track counts, or delete an empty location:

```sh
lfsplanet track-location list
lfsplanet track-location delete OTHER --yes
```

Deletion refuses locations that still contain tracks. Move those tracks in YAML
and seed the updated definitions first. Remove the old location definition from
the seed inputs too; seeding it again would recreate the location. Deleting a
location leaves its image attachment for the normal storage garbage collector.

Track API summaries include stored location metadata (`code`, `name`,
`description`, `sequence`, and `image`), plus track `image` and `outline` attachments.
The track picker derives its location filters and their order from these responses.

The frontend uses `image` when present, then `outline`, and otherwise shows a placeholder. Track
artwork is served through `/api/v1/attachments/{uuid}` and is no longer bundled
in the frontend. Deploy the schema migration and seed artwork before relying on
these URLs. The existing SVG files include light/dark colours for standalone
image rendering.

## Browsing locations and tracks

`GET /api/v1/track-locations` returns all stored locations, ordered by `sequence`
and then code. Each item includes `code`, `name`, `description`, `image`, and
`sequence`; locations do not need an era or track configuration to appear.

`GET /api/v1/track-locations/{code}` returns one location and answers 404 when
the code is unknown.

`GET /api/v1/tracks` remains paginated and accepts `location=BL` alongside the
existing `q`, `page`, and `per_page` parameters. Location filtering applies before
pagination and combines with track name/code search. Unknown location codes
return an empty track list.

The `/tracks` directory shows one full-width card per location, ordered by its
seeded sequence. Each card displays its image, name, and description and links to
`/tracks/{code}`. Cards stack their image above the text on small screens; missing
images use the same camera-off placeholder as track artwork.

Each location page shows its metadata and configuration grid, with track name/code
search and pagination reflected in its URL. The standard breadcrumb navigation leads back to
the location directory. Unknown locations show a 404 page.
The era/vehicle track picker continues to derive its location filters from the
eligible tracks returned by the era endpoint.

## Standard vehicle artwork

Standard vehicle definitions sit alongside their artwork in `assets/vehicles/`:

```yaml
kind: Vehicle
id: XFG
image: ./XFG.png
```

```sh
lfsplanet seed assets/vehicles/*.yaml --dry-run
lfsplanet seed assets/vehicles/*.yaml
```

IDs delegate to `insim_core::Vehicle` and must name a standard vehicle. Unknown
IDs and Vehicle Mods are rejected; mod metadata and covers remain owned by the
LFS API sync. File names are explicit YAML paths rather than an importer convention.
Omitted definitions and omitted `image` fields leave stored images untouched.
Each supplied vehicle creates or updates its canonical row, even when `image` is
omitted. Matching bytes retain the existing attachment identity, URL, and fetch timestamp.
Dry runs validate files without uploading or changing vehicle rows.

Standard vehicle images are served through the same attachment endpoint as mod
covers. No track or vehicle artwork is bundled into the frontend. Seed the
artwork before deploying a frontend that depends on these URLs. Local artwork
is seeded exclusively through YAML definitions.

## Deployment

`just deploy` deploys the application and then runs `deploy/catalogue.py`.
`just deploy-catalogue` runs that catalogue deployment alone, copying definitions
and artwork, synchronizing Vehicle Mods, seeding local artwork, and applying eras
when their files change. Run the application deployment first when schema or
command changes are required.

Existing era replacements require `replace_existing_eras = True` in the deployment
inventory. Review the changes before enabling it and reset it afterwards. If era
seeding fails after the files have been copied, retry `seed` manually on the host;
an unchanged file sync does not retry era seeding.

See [deployment](../deploy/README.md) for setup and release instructions.
