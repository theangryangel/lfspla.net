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
