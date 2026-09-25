# Configuration

Every `lfsplanet` process accepts `--config PATH` (or `-c`); the default is
`planet.yaml`. Generate a complete development configuration with:

```sh
cargo run --locked -- generate-config > planet.yaml
```

`worker.hlvc.poll_interval_seconds` and `worker.hlvc.timeout_seconds` control
hotlap validation. `worker.webhooks.workers` sets concurrent deliveries per
worker process and defaults to 3. Deliveries to the same destination are
serialized. Keep the count within the worker's database connection limit. The
deployment inventory sets it with `webhook_workers` in
`deploy/group_data/all.py`.
