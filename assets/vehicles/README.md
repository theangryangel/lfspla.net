# Standard vehicle artwork

Each standard vehicle has an explicit tagged definition next to its artwork:

```yaml
kind: Vehicle
id: XFG
image: ./XFG.png
```

Seed the definitions with `lfsplanet seed assets/vehicles/*.yaml`.
Paths resolve relative to the YAML file; image filenames do not need to match
vehicle codes. The existing PNG names are retained for continuity.

Images use the shared attachment store and are delivered through the API.
Each definition creates or updates that vehicle’s canonical row. Omitted vehicle
definitions leave existing rows untouched; omitted image fields preserve artwork. Identical bytes
retain the existing attachment ID and URL. Images are not bundled in the frontend.

`maintenance sync-mods` refreshes Vehicle Mods and their covers only.
