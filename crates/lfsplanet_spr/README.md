# lfsplanet_spr

Strongly typed parsing for the documented 192-byte header of Live for Speed
single-player replay (`.spr`) files. The replay stream after the header is not
publicly specified and is not read.

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
