# lfsplanet_lfs

Install, update, validate and trim local LFS copies.

Execution requires Linux, Wine and Bubblewrap. Other platforms accept runtime
settings but return unsupported errors. Trimming works on all platforms.

Supply a downloaded installer to `install`. Call `result()` on validation
diagnostics for the HLVC verdict; LFS exit codes are read from a separate result
file. Build a `TrimPlan` before confirming deletion, then pass it to `trim`.

Linux tests also compile and exercise the unsupported runtime.
