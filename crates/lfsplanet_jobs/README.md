# lfsplanet_jobs

Concurrent workers for application-owned database records. The crate has no
schema, serialized job payloads, or database dependency.

Implement `Processor::process_next` to select, execute and persist one record.
Return `Step::Processed` to immediately try another, or `Step::Idle` to poll
later. Unexpected errors are logged and followed by the polling delay.

Each processor owns eligibility, transactions, locking, retries and deadlines.
For PostgreSQL, a processor can hold a transaction around `SELECT ... FOR UPDATE
SKIP LOCKED`, execution and outcome persistence. Crashes roll back that attempt,
including its attempt counter. External effects can still repeat after a crash.

Register processors with `WorkerConfig`; their Rust type names identify them in logs.
Each processor type can be registered once per runner. `Runner::run` starts the
configured number of Tokio tasks for each processor. Counts are per process.
Type names are log labels, not database queues. Processors must support concurrent
calls, including calls from other processes.

Shutdown stops new iterations and waits for active calls. Processors must bound
external work so shutdown can finish. Dropping the runner aborts workers;
processor implementations must handle cancellation safely.

See `src/jobs/hlvc.rs` in the application for a record-backed implementation.

Run `cargo test -p lfsplanet_jobs` for the runner tests.
