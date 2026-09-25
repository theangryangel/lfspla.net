# Non-secret defaults. Per-host inventory data may override these values.

app_user = "lfsplanet"
app_group = "lfsplanet"
app_root = "/opt/lfsplanet"
config_dir = "/etc/lfsplanet"
state_dir = "/var/lib/lfsplanet"
eras_source = "../assets/eras"
builtin_vehicle_images_source = "../assets/builtin-vehicles"
max_spr_upload_bytes = "2 MiB"
webhook_workers = 3

# Frontend releases live in content-addressed directories under
# `frontend_dir/releases`, with `frontend_dir/current` symlinked to the live one.
# Only the live release is retained; see README.md.
frontend_dir = "/opt/lfsplanet/frontend"

# Response security policy for the whole site. Caddy applies this to the static
# frontend and to proxied API responses alike; the application sets none of it.
# `style-src` and `font-src` admit Google Fonts, `img-src` admits the country
# flag CDN, and `script-src` needs 'unsafe-inline' for SvelteKit's bootstrap.
content_security_policy = (
    "default-src 'self'; "
    "script-src 'self' 'unsafe-inline'; "
    "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; "
    "img-src 'self' data: https://flagcdn.com; "
    "font-src 'self' data: https://fonts.gstatic.com; "
    "connect-src 'self'; "
    "object-src 'none'; "
    "base-uri 'self'; "
    "frame-ancestors 'none'; "
    "form-action 'self'"
)

# Pyinfra runs remote state-changing operations through sudo by default.
_sudo = True
