"""Synchronize lfsplanet catalogue data and apply version-controlled eras.

Run separately from deploy.py because era changes alter ranking policy:
    pyinfra inventory.py catalogue.py

Set replace_existing_eras in inventory only when reviewed era replacements are
intended. New eras never require that flag.
"""

from pathlib import Path

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

track_sync = files.sync(
    name="Deploy location definitions and track artwork",
    src=host.data.tracks_source,
    dest=f"{config_dir}/tracks",
    user="root",
    group=app_group,
    mode="0640",
    dir_mode="0750",
    delete=False,
)

# Previous releases copied one kind: Track document per SVG. Remove only those
# managed definition paths so the seed glob cannot include the obsolete format.
# Database records and artwork are retained; operator-created filenames are untouched.
track_source = Path(__file__).resolve().parent / host.data.tracks_source
for outline in sorted(track_source.glob("*.svg")):
    files.file(
        name=f"Remove obsolete standalone track definition {outline.stem}",
        path=f"{config_dir}/tracks/{outline.stem}.yaml",
        present=False,
    )

vehicles = files.sync(
    name="Deploy standard vehicle definitions and artwork",
    src=host.data.vehicles_source,
    dest=f"{config_dir}/vehicles",
    user="root",
    group=app_group,
    mode="0640",
    dir_mode="0750",
    # Images may be removed from a future release only through an explicit
    # object-storage cleanup policy.
    delete=False,
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
    name="Seed locations, tracks and standard vehicles",
    commands=[
        f"{app_root}/lfsplanet --config {config_dir}/config.yaml seed "
        f"{config_dir}/tracks/*.yaml {config_dir}/vehicles/*.yaml"
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
