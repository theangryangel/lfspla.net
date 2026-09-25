session := "lfs-planet"

# List available tasks.
default:
    @just --list

# Remove build outputs and frontend caches.
clean:
    cargo clean
    rm -rf frontend2/dist frontend2/.svelte-kit frontend2/node_modules/.vite

# Generate track SVGs from one or more .pth files.
[positional-arguments]
track-gen +paths:
    cargo run --locked -p lfsplanet_track_gen -- --output assets/tracks/ "$@"

# Start or attach to the tmux session for services, frontend, and backend.
dev:
    #!/usr/bin/env bash
    if ! tmux has-session -t {{session}} 2>/dev/null; then
        # Create the session (this creates the first pane automatically)
        tmux new-session -d -s {{session}} -n "services"
        tmux send-keys -t {{session}} "docker compose up" C-m
        tmux split-window -h -t {{session}}
        tmux send-keys -t {{session}} "npm run dev --prefix=frontend2" C-m
        tmux split-window -v -t {{session}}
        tmux send-keys -t {{session}} "cargo run -- -c planet.yaml web" C-m
    fi
    tmux attach-session -t {{session}}

# Run migrations, sync the catalogue, and apply era definitions.
seed:
    cargo run --locked -- migrate
    cargo run --locked -- maintenance catalogue-sync --standard-vehicle-images-dir assets/builtin-vehicles
    cargo run --locked -- era apply assets/eras/*.yaml --yes

# Build the backend and frontend, then deploy the site and eras with pyinfra.
deploy:
    cargo build --locked --release
    npm --prefix frontend2 ci
    npm --prefix frontend2 run build
    uv run --directory deploy --with-requirements requirements.txt pyinfra inventory.py deploy.py
    uv run --directory deploy --with-requirements requirements.txt pyinfra inventory.py eras.py

# Deploy updated eras.
deploy-eras:
    uv run --directory deploy --with-requirements requirements.txt pyinfra inventory.py eras.py

# Run migrations against the development database.
migrate:
    cargo run --locked -- migrate
