# lfsplanet_spr

Strongly typed parsing for the documented 192-byte header of Live for Speed
single-player replay (`.spr`) files. `SprHeader::read` and `from_path` also scan
the remaining replay stream for the exact bytes `Test Drive\0` and `TEST DRV`,
returning `Error::TestDrive` only when both are found. This heuristic detects test-drive player names
without depending on an undocumented offset, but can match other string fields
and does not protect against edited files. Replay data is read into memory with a 64 MiB safety cap; larger streams
are rejected. Accepted files are consumed to EOF. `SprHeader::decode` parses only
the header.

An SPR can have any filename. `ConventionalFilename` separately parses the
metadata-bearing `<username>_<track>_<vehicle>_<lap-time>.spr` convention when
an application expects it.

```rust
use lfsplanet_spr::{ConventionalFilename, SprHeader};

let header = SprHeader::from_path("data/spr/example.spr")?;
println!("{} on {}", header.local_driver_name, header.track);
let filename = "racer_BL2R_XRG_132450.spr".parse::<ConventionalFilename>()?;
assert_eq!(filename.lap_time.as_millis(), 92_450);
# Ok::<(), Box<dyn std::error::Error>>(())
```
