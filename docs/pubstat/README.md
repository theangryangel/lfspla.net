# Pubstat endpoint

LFSplanet provides a subset of LFSWorld Pubstat at:

```text
GET /api/pubstat/{era}/get_stat2.php?action=...
```

A short public reference is served at `/api/pubstat`, linked from `/api/docs`
and the frontend footer. Its source is [reference.txt](reference.txt).

The era is an LFSplanet era slug. Protocol version is selected independently by
`version=1.0` through `version=1.5`; omission defaults to `1.5`.

## Supported queries

| Action | Required                                          | Optional                                     |
| ------ | ------------------------------------------------- | -------------------------------------------- |
| `hl`   | `racer`: LFS username                             | —                                            |
| `ch`   | `track`: legacy numeric code; `car`: vehicle code | `control=w/m/ks/kn`; `country=GB` (1.5 only) |
| `wr`   | —                                                 | `track`, `car` (1.4 and later)               |

Results contain the current best valid lap per player/chart, including imported
laps. WR results contain the current leader of each populated chart. All reads
are scoped to the requested era and its current charts. Chart rows follow
LFSplanet's global chart order, including its tie breaks; controller and country
filters select from those best laps without recomputing personal bests.
Collections are complete, without pagination.

```text
/api/pubstat/2007-12-21/get_stat2.php?action=hl&racer=example&version=1.2
/api/pubstat/2007-12-21/get_stat2.php?action=ch&track=000&car=XFG&control=w
/api/pubstat/2007-12-21/get_stat2.php?action=wr&car=XFG&s=1
```

## Unsupported options and errors

`pb`, `fuel`, `pst`, `hosts`, `teams`, `hl_log`, and `counters` return HTTP 501.
PHP serialization (`s=2`) also returns 501.

Malformed queries, unsupported versions, unknown actions/formats, and filters
unavailable in the selected version return 400. Missing eras, players, or charts
return 404. Supported queries without matching laps return a successful empty
collection. Errors are plain text in every output mode: `code: message\n`.
Internal errors do not expose database details.

Read access is public. Legacy `idk`, `user`, `pass`, and `ps` fields are unused;
do not send LFSWorld credentials. Tarpitting and premium billing are not
implemented.
