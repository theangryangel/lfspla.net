# Built-in vehicle images

Name files after the vehicle code: `XFG.png`, `FBM.png`.

`maintenance catalogue-sync --standard-vehicle-images-dir DIR` uploads every
required PNG to object storage and records it in the matching `vehicle` row.
The API then serves it through the vehicle image endpoint.
