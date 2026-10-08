session := "lfs-planet"

# List available tasks.
default:
    @just --list

# Regenerate checked-in SDKs from the Rust OpenAPI document (requires Docker).
generate-sdks: generate-openapi generate-sdks-from-openapi

# Export the Rust API contract without starting the backend or database.
generate-openapi:
    mkdir -p target
    cargo run --locked -- openapi > target/openapi.json

# Generate SDKs from target/openapi.json (requires Docker).
generate-sdks-from-openapi:
    #!/usr/bin/env bash
    set -euo pipefail
    test -s target/openapi.json
    cd sdks
    npx --yes fern-api@5.148.2 check
    npx --yes fern-api@5.148.2 generate --group typescript --local --no-prompt --force
    cd ..
    # Two passes stabilize nested file types with the pinned Prettier version.
    frontend2/node_modules/.bin/prettier --write sdks/typescript/src
    frontend2/node_modules/.bin/prettier --write sdks/typescript/src

# Remove build outputs and frontend caches.
clean:
    cargo clean
    rm -rf frontend2/dist frontend2/.svelte-kit frontend2/node_modules/.vite

# Generate track SVGs from one or more .pth files.
[positional-arguments]
track-gen +paths:
    cargo run --locked -p lfsplanet_track_gen -- --output assets/tracks/ "$@"

# Check tools needed for local development.
check-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    missing=0
    if [[ -t 1 ]]; then
        green=$'\033[32m'
        red=$'\033[31m'
        reset=$'\033[0m'
    else
        green=''
        red=''
        reset=''
    fi
    check_command() {
        label="$1"
        command_name="$2"
        if command -v "$command_name" >/dev/null 2>&1; then
            version="$("$command_name" --version 2>/dev/null | head -n 1)"
            printf '  %b✓%b %s (%s)\n' "$green" "$reset" "$label" "$version"
        else
            printf '  %b✗%b %s (%s)\n' "$red" "$reset" "$label" "$command_name"
            missing=1
        fi
    }
    printf 'Checking development prerequisites...\n'
    check_command "Rust compiler" rustc
    check_command "Cargo" cargo
    check_command "npm" npm
    if command -v node >/dev/null 2>&1; then
        node_major="$(node -p 'Number(process.versions.node.split(".")[0])')"
        if (( node_major >= 24 )); then
            printf '  %b✓%b Node.js >=24 (%s)\n' "$green" "$reset" "$(node --version)"
        else
            printf '  %b✗%b Node.js >=24 (found %s)\n' "$red" "$reset" "$(node --version)"
            missing=1
        fi
    else
        printf '  %b✗%b Node.js >=24 (node)\n' "$red" "$reset"
        missing=1
    fi
    if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
        printf '  %b✓%b Docker Compose (%s)\n' "$green" "$reset" "$(docker compose version --short)"
    else
        printf '  %b✗%b Docker Compose plugin\n' "$red" "$reset"
        missing=1
    fi
    if command -v tmux >/dev/null 2>&1; then
        printf '  %b✓%b tmux (%s)\n' "$green" "$reset" "$(tmux -V)"
    else
        printf '  %b✗%b tmux\n' "$red" "$reset"
        missing=1
    fi
    if command -v prek >/dev/null 2>&1; then
        printf '  %b✓%b prek (%s)\n' "$green" "$reset" "$(prek --version)"
    else
        printf '  prek is recommended for repository checks and hooks\n'
    fi
    if command -v bacon >/dev/null 2>&1; then
        printf '  %b✓%b Bacon (%s)\n' "$green" "$reset" "$(bacon --version)"
    else
        printf '  Bacon is optional; install it with cargo install --locked bacon for --watch\n'
    fi
    if (( missing )); then
        echo "Install the missing prerequisites, then run 'just init' again." >&2
        exit 1
    fi

# Initialize local development: config, frontend dependencies, PostgreSQL, and migrations.
init: check-deps
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! -e planet.yaml ]]; then
        just generate-config
    fi
    lock_hash="$(sha256sum frontend2/package-lock.json | cut -d ' ' -f 1)"
    if [[ ! -f frontend2/node_modules/.package-lock.sha256 ]] || [[ "$(cat frontend2/node_modules/.package-lock.sha256)" != "$lock_hash" ]]; then
        npm --prefix frontend2 ci
        printf '%s\n' "$lock_hash" > frontend2/node_modules/.package-lock.sha256
    fi
    docker compose up -d --wait postgres
    cargo run --locked -- -c planet.yaml migrate
    printf '%s\n' \
        "Init complete 🎉" \
        "Next steps: " \
        " 1. Register at https://www.lfs.net/account/api with callback http://localhost:5173/auth/lfs/callback, then replace REPLACE_ME in 'planet.yaml'." \
        " 2. Run 'just seed' to sync all vehicles from LFS.net" \
        " 3. Run 'just dev' (or 'just start') and head to http://localhost:5173/"

# Familiar synonym for init.
setup: init

# Familiar synonym for dev; pairs with stop and forwards options like --watch.
alias start := dev

# Start the host API and frontend; use --watch for automatic API restarts.
[arg('watch', long='watch', short='w', value='true', help='Watch Rust files and restart the API with Bacon')]
dev watch='false':
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! -f planet.yaml ]]; then
        echo "planet.yaml is missing; run 'just init' first" >&2
        exit 1
    fi
    lock_hash="$(sha256sum frontend2/package-lock.json | cut -d ' ' -f 1)"
    if [[ ! -f frontend2/node_modules/.package-lock.sha256 ]] || [[ "$(cat frontend2/node_modules/.package-lock.sha256)" != "$lock_hash" ]]; then
        npm --prefix frontend2 ci
        printf '%s\n' "$lock_hash" > frontend2/node_modules/.package-lock.sha256
    fi
    docker compose up -d --wait postgres
    if [[ "{{watch}}" == "true" ]]; then
        if ! command -v bacon >/dev/null 2>&1; then
            echo "Bacon is required for watch mode; install it with 'cargo install --locked bacon'." >&2
            exit 1
        fi
        api_command="bacon --headless --job web"
    else
        api_command="cargo run --locked -- -c planet.yaml web"
    fi
    if ! tmux has-session -t {{session}} 2>/dev/null; then
        tmux new-session -d -s {{session}} -n "services"
        tmux send-keys -t {{session}} "docker compose logs -f postgres" C-m
        tmux split-window -h -t {{session}}
        tmux send-keys -t {{session}} "npm run dev --prefix=frontend2" C-m
        tmux split-window -v -t {{session}}
        tmux send-keys -t {{session}} "$api_command" C-m
    fi
    tmux attach-session -t {{session}}

# Stop the local API, frontend, and PostgreSQL.
stop:
    #!/usr/bin/env bash
    set -euo pipefail
    if tmux has-session -t {{session}} 2>/dev/null; then
        tmux kill-session -t {{session}}
    fi
    docker compose stop postgres
    echo "Development services stopped."

# Generate a development config without overwriting an existing one.
generate-config:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -e planet.yaml ]]; then
        echo "planet.yaml already exists; refusing to overwrite it" >&2
        exit 1
    fi
    config_tmp="$(mktemp)"
    trap 'rm -f "$config_tmp"' EXIT
    cargo run --locked -- generate-config --development > "$config_tmp"
    mv "$config_tmp" planet.yaml

# Run database migrations, sync the catalogue, and apply eras.
seed:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! -f planet.yaml ]]; then
        echo "planet.yaml is missing; run 'just init' first" >&2
        exit 1
    fi
    if grep -qF 'REPLACE_ME' planet.yaml; then
        echo "Replace the LFS OAuth REPLACE_ME values in planet.yaml before running 'just seed'." >&2
        echo "See docs/development.md for how to register an LFS API application." >&2
        exit 1
    fi
    docker compose up -d --wait postgres
    cargo run --locked -- -c planet.yaml migrate
    cargo run --locked -- -c planet.yaml maintenance catalogue-sync --standard-vehicle-images-dir assets/builtin-vehicles
    cargo run --locked -- -c planet.yaml era apply assets/eras/*.yaml --yes

# Deploy demo data
demo:
    cargo run --locked -- -c planet.yaml demo --yes

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

# Run database migrations.
migrate:
    docker compose up -d --wait postgres
    cargo run --locked -- -c planet.yaml migrate
