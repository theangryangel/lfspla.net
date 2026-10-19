# Community cards

These catalogue documents supply the homepage's Community & partners carousel.
See [catalogue authoring](../../docs/catalogue.md#community-cards) for supported
templates and the seed workflow.

The `partner` template describes a community resource; it does not assert a
commercial partnership. These resource cards are not marked as sponsored. All
templates use text on the left and an optional rectangular image panel on the
right. Cards with a destination are clickable across their whole surface.

## Publication controls

Set `enabled: false` to hide a card, and use `sequence` to order it (lowest first).
Optional `starts_at` and `ends_at` timestamps gate its availability:

```yaml
enabled: true
sequence: 20
starts_at: "2026-11-01T00:00:00Z"
ends_at: "2026-12-01T00:00:00Z"
```

Omit either timestamp for an open-ended window. Timestamps require an explicit
timezone; the start is inclusive and the end exclusive. Seed the updated YAML to
apply changes.

## Temporary template previews

`preview/` contains one labelled example of each template. Load them explicitly
for local review; the normal `just seed` and deployment seed globs select only
the community YAML files at the top level.

`just demo` generates demo data and imports these preview cards automatically.

```sh
lfsplanet seed assets/community/preview/*.yaml
```

To hide previews afterwards, set `enabled: false` in the three preview documents
and seed them again. Removing a document alone does not remove its database row.

## Image sources

Branding images were downloaded from the linked sites on 19 October 2026 and are
retained without alteration. Branding belongs to the respective projects.

| File                        | Source                                                                                                            |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `images/sim-broadcasts.jpg` | [Sim Broadcasts social preview image](https://simbroadcasts.tv/assets/meta/og-image.jpg)                          |
| `images/lfs-track-hub.png`  | [LFS Track Hub site logo](https://lfstrackhub.com/wp-content/themes/lfs-track-hub-theme/assets/logo-approved.png) |
| `images/lfs-manual.png`     | [LFS Manual wiki logo](https://en.lfsmanual.net/skins/common/images/wikilogo.png)                                 |

Descriptions were written for this catalogue using the projects' own sites:
[Sim Broadcasts](https://simbroadcasts.tv/join),
[LFS Track Hub](https://lfstrackhub.com/), and
[LFS Manual](https://en.lfsmanual.net/wiki/Main_Page).
