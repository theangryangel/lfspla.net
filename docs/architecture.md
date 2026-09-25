# Architecture

lfspla.net is a Rust API and worker using PostgreSQL, with a Svelte frontend.

```mermaid
flowchart LR
    User[Browser\nor API client] --> Caddy[Caddy]
    Caddy --> Frontend[Svelte frontend]
    Caddy --> Web[web service]

    subgraph App["lfsplanet Rust binary"]
        Web
        Worker[worker]
        subgraph Commands[One-off commands]
            Migrate[migrate]
            Eras[era apply]
            Maintenance[maintenance]
        end
    end

    App --> Database[(PostgreSQL)]
    App --> Storage[Object storage\nReplays and vehicle images]
    Worker --> Game[LFS HLVC\nBubblewrapped sandbox]
    Config[Configuration\nplanet.yaml or config.yaml] --> App
    EraFiles["assets/eras/*.yaml"] --> Eras
```

## Eras

The YAML files in `assets/eras/` are setup files. Apply them explicitly:

```sh
lfsplanet maintenance catalogue-sync
lfsplanet era apply assets/eras/*.yaml
```

`catalogue-sync` adds tracks and vehicles. `era apply` checks and saves eras in
one transaction. New eras need no confirmation; changes need `--yes`. Startup
does not apply these files.

## HLVC worker

Uploads are parsed, queued, and matched to an open era. Run the worker
separately:

```sh
lfsplanet worker
```

The worker validates queued hotlaps with the era's LFS installation, Wine, and
Bubblewrap. It publishes valid laps and records all other results. See [replay
validation](validator.md) for host requirements.
