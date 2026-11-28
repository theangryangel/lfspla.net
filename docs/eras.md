# Eras and rankings

An era defines a range of LFS versions, an LFS installation, and rankings.
A track and vehicle pair is eligible only when at least one ranking in that
era selects it.

Era definitions are versioned YAML documents under `assets/eras/`, but PostgreSQL
is the source of truth at runtime.

Think of the eras files as desired state, or seeding/fixture data.

## Applying definitions

```sh
lfsplanet seed assets/eras/*.yaml --dry-run
lfsplanet seed assets/eras/*.yaml
lfsplanet hotlaps era list
lfsplanet hotlaps era export 2007-12-21 > era.yaml
lfsplanet seed era.yaml --yes
lfsplanet hotlaps era open 2007-12-21
lfsplanet hotlaps era close 2007-12-21
lfsplanet hotlaps era delete 2007-12-21 --yes
```

Era documents declare `kind: Era`. See [catalogue seeding](catalogue.md) for
input formats, validation, and dry runs. `hotlaps era export` emits tagged YAML suitable
for `seed`.

Each ranking has a `discipline`: `circuit` (the default for existing definitions)
or `drag`. For example:

```yaml
- id: drag-stock
  discipline: drag
  title: Stock Drag Rank
  # description, rules and combinations follow as usual
```

Discipline groups rankings for navigation and filtering; it does not change
chart identity, scoring, replay admission or upload validation. An era can have
several rankings of either discipline, and a chart can belong to several
rankings. Discipline-scoped chart suggestions include combinations belonging
to any matching ranking, without duplicates.

The API includes `discipline` in ranking summaries and details. The era ranking,
track and vehicle collections accept an optional `discipline=circuit|drag`
query parameter. Omitting it preserves the complete era catalogue.

Eras omitted from the input are left untouched. Rankings and combinations inside
a supplied era retain their replacement rules. Changing an existing era requires
`--yes` and may delete laps outside its new combinations, as reported in the
change summary. Referenced LFS installations and canonical tracks and vehicles
must already exist; location definitions with nested tracks and vehicle definitions can be seeded
in the same batch.

## Era statistics

`GET /api/v1/eras/{era}/stats` returns `validated_hotlaps`, `rankings` and
`charts`. Hotlaps counts all validated laps, including imports and superseded
personal bests. Charts counts distinct offered track/vehicle combinations,
including those without laps; membership in multiple rankings counts once.

The response uses the same one-hour in-memory cache as homepage statistics,
keyed by era path and shared across visitors. Query strings do not create extra
cache entries. Uploads and catalogue changes may take up to an hour to appear;
a backend restart also clears the cache.

## Completion badges

A ranking can award a completion badge in addition to its usual ranking badge:

```yaml
badge:
  id: nutter
  qualification:
    type: ranked
    limit: 3
  completion:
    id: gabor
```

The top three keep their Nutter badges. Every player with a published result on
all charts selected by that era's Nutter ranking also receives Gabor, regardless
of position or benchmark handicap. Players can hold both badges. The `id` fields reference [catalogue badges](catalogue.md#badges); artwork and names
come from those definitions. Seed the badge definitions before applying an era.

Apply the updated era definitions to activate this configuration; applying an
updated definition also rebuilds its stored player badges.
