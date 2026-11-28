# lfsplanet_country_ext

Flag choices for `celes::Country` through `CountryFlagsExt`.
`FlagCode::parse` trims whitespace and accepts either case.

## Catalogue

`codes.json` is a snapshot of https://flagcdn.com/en/codes.json from 2026-09-29.
Update it in the repository; builds need no network access.

`build.rs` generates the catalogue and country lists. It rejects missing
national or policy flags, unknown countries, and invalid codes.
Runtime lookups use static tables.

## Choices

Each country gets its national flag first, its subdivisions, and `un`.
The `EUROPE` list in `build.rs` also grants `eu` to European countries and
territories, including transcontinental countries. GB is excluded.

## API

`GET /api/v1/countries/{code}/flags` lists the choices.

A player's `country_code` determines nation rankings. `flag_code` selects their
display flag; null uses their country flag.

Profile PATCH requests leave omitted fields unchanged; null clears them.
Changing country clears an incompatible flag unless a replacement is supplied.
Invalid country or flag choices are rejected.
