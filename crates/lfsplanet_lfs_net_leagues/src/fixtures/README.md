# LFS parser fixtures

Public HTML captured on 22 October 2026 from:

- `leagues.html.txt`: https://www.lfs.net/leagues
- `league.html.txt`: https://www.lfs.net/leagues/432
- `season.html.txt`: https://www.lfs.net/leagues/432/season/1347
- `calendar.html.txt`: https://www.lfs.net/leagues/calendar

The `.html.txt` extension preserves raw upstream markup, including malformed
HTML, without passing it through an HTML formatter. Only the page body containing the relevant records is retained. Markup and
community-authored text belong to their respective authors and are included as
parser regression inputs, not as application content. These fixtures capture
observed structures, not a supported upstream API contract.
