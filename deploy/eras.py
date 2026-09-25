"""Synchronize lfsplanet catalogue data and apply version-controlled eras.

Run separately from deploy.py because era changes alter ranking policy:
    pyinfra inventory.py eras.py

Set replace_existing_eras in inventory only when reviewed era replacements are
intended. New eras never require that flag.
"""

from pyinfra import host
from pyinfra.operations import files, server


app_user = host.data.app_user
app_group = host.data.app_group
app_root = host.data.app_root
config_dir = host.data.config_dir
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

builtin_vehicle_images = files.sync(
    name="Deploy built-in vehicle images",
    src=host.data.builtin_vehicle_images_source,
    dest=f"{config_dir}/builtin-vehicles",
    user="root",
    group=app_group,
    mode="0640",
    dir_mode="0750",
    # Images may be removed from a future release only through an explicit
    # object-storage cleanup policy.
    delete=False,
)

server.shell(
    name="Synchronize the lfsplanet catalogue",
    commands=[
        f"{app_root}/lfsplanet --config {config_dir}/config.yaml maintenance catalogue-sync "
        f"--standard-vehicle-images-dir {config_dir}/builtin-vehicles"
    ],
    _sudo_user=app_user,
)

server.shell(
    name="Apply lfsplanet era definitions",
    commands=[
        f"{app_root}/lfsplanet --config {config_dir}/config.yaml era apply "
        f"{apply_confirmation}{config_dir}/eras/*.yaml --yes"
    ],
    _if=era_sync.did_change,
    _sudo_user=app_user,
)
