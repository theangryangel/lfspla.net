"""Environment-backed inventory for the lfsplanet production host."""

import os


lfsplanet = [
    (
        os.environ["LFSPLANET_HOST"],
        {
            # Pyinfra elevates state-changing operations through sudo.
            "_sudo": True,
            "site_domain": "lfspla.net",
            # Redirect aliases to the canonical site domain.
            "alternate_domains": ["www.lfspla.net"],
            # Paths relative to this deploy directory.
            "release_binary": "../target/release/lfsplanet",
            "frontend_dist": "../frontend2/dist",
            # Exactly 128 hexadecimal characters (64 random bytes).
            "session_key": os.environ["LFSPLANET_SESSION_KEY"],
            # Set both values to enable LFS OAuth, or omit both to disable it.
            "oauth_client_id": os.environ["LFSPLANET_OAUTH_CLIENT_ID"],
            "oauth_client_secret": os.environ["LFSPLANET_OAUTH_CLIENT_SECRET"],
            # Set all four to use S3-compatible storage; leave all unset for local files.
            "bucket_name": os.environ.get("LFSPLANET_BUCKET_NAME"),
            "bucket_host": os.environ.get("LFSPLANET_BUCKET_HOST"),
            "bucket_access_key_id": os.environ.get("LFSPLANET_BUCKET_ACCESS_KEY_ID"),
            "bucket_secret_access_key": os.environ.get("LFSPLANET_BUCKET_SECRET_ACCESS_KEY"),
            # Override when the provider requires a different signing region.
            "bucket_region": os.environ.get("LFSPLANET_BUCKET_REGION", "us-east-1"),
            # Provision prepares the client; pgbackrest.py activates backups.
            "backup_bucket_name": os.environ.get("LFSPLANET_BACKUP_BUCKET_NAME"),
            "backup_bucket_host": os.environ.get("LFSPLANET_BACKUP_BUCKET_HOST"),
            "backup_bucket_access_key_id": os.environ.get("LFSPLANET_BACKUP_BUCKET_ACCESS_KEY_ID"),
            "backup_bucket_secret_access_key": os.environ.get("LFSPLANET_BACKUP_BUCKET_SECRET_ACCESS_KEY"),
            "backup_cipher_pass": os.environ.get("LFSPLANET_BACKUP_CIPHER_PASS"),
            "backup_bucket_region": os.environ.get("LFSPLANET_BACKUP_BUCKET_REGION", "us-east-1"),
            "backup_retention_days": os.environ.get("LFSPLANET_BACKUP_RETENTION_DAYS", "30"),
            # Set true only after reviewing replacements to existing eras.
            "replace_existing_eras": False,
        },
    ),
]
