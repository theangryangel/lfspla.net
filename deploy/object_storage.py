"""Translate deployment bucket settings into object_store's AWS configuration."""

import re
from urllib.parse import urlsplit


def storage_configuration(data):
    values = {
        name: data.get(name)
        for name in (
            "bucket_name",
            "bucket_host",
            "bucket_access_key_id",
            "bucket_secret_access_key",
        )
    }
    if not any(values.values()):
        return f"{data.state_dir}/spr", {}
    if not all(values.values()):
        raise ValueError("set all four LFSPLANET_BUCKET_* variables, or leave all unset")

    bucket = values["bucket_name"]
    if not re.fullmatch(r"[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]", bucket):
        raise ValueError("LFSPLANET_BUCKET_NAME must be an S3 bucket name")
    endpoint = values["bucket_host"]
    if "://" not in endpoint:
        endpoint = f"https://{endpoint}"
    parsed = urlsplit(endpoint)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or parsed.path not in ("", "/")
    ):
        raise ValueError("LFSPLANET_BUCKET_HOST must be an HTTPS S3 API endpoint")

    environment = {
        "AWS_ACCESS_KEY_ID": values["bucket_access_key_id"],
        "AWS_SECRET_ACCESS_KEY": values["bucket_secret_access_key"],
        "AWS_ENDPOINT": endpoint.rstrip("/"),
        "AWS_REGION": data.get("bucket_region", "us-east-1"),
    }
    # These values also appear in a systemd EnvironmentFile. Reject line breaks
    # rather than allowing a value to introduce another environment assignment.
    for value in environment.values():
        if not isinstance(value, str) or not value or any(c in value for c in "\0\r\n"):
            raise ValueError("bucket settings must be non-empty, single-line strings")
    return f"s3://{bucket}", environment


def quote_environment(value):
    """Quote a literal value for a systemd EnvironmentFile (no shell expansion)."""
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def storage_image_policy(policy, endpoint):
    """Allow redirected images from the configured S3 origin."""
    if not endpoint:
        return policy
    parsed = urlsplit(endpoint)
    hostname = parsed.hostname
    if not hostname or any(c.isspace() or c in "'\";\\" for c in hostname):
        raise ValueError("invalid S3 hostname for Content-Security-Policy")
    hostname = f"[{hostname}]" if ":" in hostname else hostname
    port = f":{parsed.port}" if parsed.port and parsed.port != 443 else ""
    origin = f"https://{hostname}{port}"
    directives = [part.split() for part in policy.split(";") if part.strip()]
    if not any(parts[0] == "img-src" for parts in directives):
        fallback = next((parts[1:] for parts in directives if parts[0] == "default-src"), None)
        if fallback is not None:
            directives.append(["img-src", *fallback])
    for parts in directives:
        if parts[0] in ("img-src", "img-src-elem") and origin not in parts:
            parts[:] = [part for part in parts if part != "'none'"]
            parts.append(origin)
    return "; ".join(" ".join(parts) for parts in directives)


def backup_configuration(data):
    names = (
        "backup_bucket_name",
        "backup_bucket_host",
        "backup_bucket_access_key_id",
        "backup_bucket_secret_access_key",
        "backup_cipher_pass",
    )
    values = {name: data.get(name) for name in names}
    if all(value is None for value in values.values()):
        return None
    for name, value in values.items():
        if (
            not isinstance(value, str)
            or not value
            or value != value.strip()
            or any(c in value for c in "\0\r\n")
        ):
            raise ValueError(f"set all five LFSPLANET_BACKUP_* settings; invalid {name}")

    bucket = values["backup_bucket_name"]
    if not re.fullmatch(r"[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]", bucket):
        raise ValueError("LFSPLANET_BACKUP_BUCKET_NAME must be an S3 bucket name")
    # GC scans the entire attachment bucket, so never put a repository there.
    if bucket == data.get("bucket_name"):
        raise ValueError("the backup bucket must be separate from the attachment bucket")

    endpoint = values["backup_bucket_host"]
    if "://" not in endpoint:
        endpoint = f"https://{endpoint}"
    parsed = urlsplit(endpoint)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or parsed.path not in ("", "/")
        or not re.fullmatch(r"[a-zA-Z0-9.-]+", parsed.hostname)
        or parsed.port == 0
    ):
        raise ValueError("LFSPLANET_BACKUP_BUCKET_HOST must be an HTTPS S3 API endpoint")

    region = data.get("backup_bucket_region", "us-east-1")
    if not isinstance(region, str) or not re.fullmatch(r"[a-zA-Z0-9-]+", region):
        raise ValueError("LFSPLANET_BACKUP_BUCKET_REGION must be a signing region")
    retention = str(data.get("backup_retention_days", "30"))
    if not retention.isascii() or not retention.isdigit() or not 1 <= int(retention) <= 9999999:
        raise ValueError("LFSPLANET_BACKUP_RETENTION_DAYS must be between 1 and 9999999")

    return {
        "bucket": bucket,
        "endpoint": parsed.hostname,
        "port": parsed.port or 443,
        "region": region,
        "access_key": values["backup_bucket_access_key_id"],
        "secret_key": values["backup_bucket_secret_access_key"],
        "cipher_pass": values["backup_cipher_pass"],
        "retention_days": int(retention),
    }
