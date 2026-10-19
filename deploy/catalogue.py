"""Synchronize lfsplanet catalogue data and apply version-controlled eras.

Run separately from deploy.py because era changes alter ranking policy:
    pyinfra inventory.py catalogue.py

Set replace_existing_eras in inventory only when reviewed era replacements are
intended. New eras never require that flag.
"""

from pathlib import Path
from shlex import quote

from object_storage import storage_configuration

from pyinfra import host
from pyinfra.operations import files, server


app_user = host.data.app_user
app_group = host.data.app_group
app_root = host.data.app_root
config_dir = host.data.config_dir
_, storage_environment = storage_configuration(host.data)
replace_existing_eras = host.data.get("replace_existing_eras", False)
apply_confirmation = "--yes " if replace_existing_eras else ""

era_sync = files.sync(
    name="Deploy version-controlled era definitions",
    src=host.data.eras_source,
    dest=f"{config_dir}/eras",
    user="root",
    group=app_group,
    mode="0640",
    dir_mode="0750",
    # Do not delete operator-created era files from the server.
    delete=False,
)

# Transfer each tree in one rsync session instead of checking files individually
# over SSH. Keep the small era sync above for its did_change condition.
for directory, source in (
    ("tracks", host.data.tracks_source),
    ("vehicles", host.data.vehicles_source),
    ("community", host.data.community_source),
):
    files.directory(
        name=f"Ensure the {directory} catalogue directory exists",
        path=f"{config_dir}/{directory}",
        user="root",
        group=app_group,
        mode="0750",
    )
    files.rsync(
        name=f"Deploy {directory} definitions and artwork",
        src=quote(str((Path(__file__).resolve().parent / source).resolve()) + "/"),
        dest=quote(f"{config_dir}/{directory}/"),
        # Explicit flags avoid pyinfra's default --delete. Preserve operator
        # files and use checksums so content changes never depend on timestamps.
        flags=[
            "-ac",
            quote(f"--chown=root:{app_group}"),
            "--chmod=D750,F640",
        ],
    )

# Previous releases copied one kind: Track document per SVG. Remove only those
# managed definition paths so the seed glob cannot include the obsolete format.
# Database records and artwork are retained; operator-created filenames are untouched.
track_source = Path(__file__).resolve().parent / host.data.tracks_source
obsolete_tracks = [
    quote(f"{config_dir}/tracks/{outline.stem}.yaml")
    for outline in sorted(track_source.glob("*.svg"))
]
if obsolete_tracks:
    server.shell(
        name="Remove obsolete standalone track definitions",
        commands=["rm -f -- " + " ".join(obsolete_tracks)],
    )

# The bookmark template was retired. Keep its old managed file out of the seed glob.
files.file(
    name="Remove obsolete bookmarks definition",
    path=f"{config_dir}/community/bookmarks.yaml",
    present=False,
)

server.shell(
    name="Synchronize Vehicle Mods",
    commands=[
        f"{app_root}/lfsplanet --config {config_dir}/config.yaml maintenance sync-mods"
    ],
    _sudo_user=app_user,
    _env=storage_environment,
)

server.shell(
    name="Seed locations, tracks, standard vehicles and community cards",
    commands=[
        f"{app_root}/lfsplanet --config {config_dir}/config.yaml seed "
        f"{config_dir}/tracks/*.yaml {config_dir}/vehicles/*.yaml {config_dir}/community/*.yaml"
    ],
    _sudo_user=app_user,
    _env=storage_environment,
)

server.shell(
    name="Apply lfsplanet era definitions",
    commands=[
        f"{app_root}/lfsplanet --config {config_dir}/config.yaml seed "
        f"{apply_confirmation}{config_dir}/eras/*.yaml"
    ],
    _if=era_sync.did_change,
    _sudo_user=app_user,
    _env=storage_environment,
)
