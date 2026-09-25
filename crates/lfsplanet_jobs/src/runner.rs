use std::{
    any::{TypeId, type_name},
    collections::HashSet,
    future::Future,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, ensure};
use tokio::{sync::watch, task::JoinSet};

/// One processor iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Processed,
    Idle,
}

/// Selects, executes and persists one application record.
///
/// Calls may run concurrently. Implementations own transactions, locking,
/// retries and deadlines. Return `Idle` when no record is eligible.
///
/// Errors are logged and retried after the poll delay. Dropping the runner
/// cancels in-flight calls; external effects may still happen more than once.
pub trait Processor: Send + Sync + 'static {
    fn process_next(&self) -> impl Future<Output = anyhow::Result<Step>> + Send;
}

#[derive(Debug, Clone, Copy)]
pub struct WorkerConfig {
    pub workers: usize,
    /// Delay after an idle iteration or unexpected error.
    pub poll_interval: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            workers: 1,
            poll_interval: Duration::from_secs(1),
        }
    }
}

type Start = Box<dyn FnOnce(&mut JoinSet<()>, watch::Receiver<bool>) + Send>;

/// Runs independently configured processors, without owning their storage.
#[derive(Default)]
pub struct Runner {
    processors: HashSet<TypeId>,
    starters: Vec<Start>,
}

impl Runner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a processor type once, using its Rust type name in logs.
    pub fn register<P: Processor>(
        mut self,
        processor: P,
        config: WorkerConfig,
    ) -> anyhow::Result<Self> {
        let name = type_name::<P>();
        ensure!(config.workers > 0, "worker count must be positive");
        ensure!(
            !config.poll_interval.is_zero(),
            "poll interval must be positive"
        );
        ensure!(
            self.processors.insert(TypeId::of::<P>()),
            "processor already registered: {name}"
        );
        let processor = Arc::new(processor);
        self.starters.push(Box::new(move |workers, stopped| {
            for _ in 0..config.workers {
                let processor = processor.clone();
                let mut stopped = stopped.clone();
                workers.spawn(async move {
                    loop {
                        if *stopped.borrow() { break; }
                        match processor.process_next().await {
                            Ok(Step::Processed) => {
                                // Avoid starving other workers.
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Ok(Step::Idle) => {}
                            Err(error) => tracing::error!(processor = %name, ?error, "processor iteration failed"),
                        }
                        tokio::select! {
                            _ = stopped.changed() => break,
                            () = tokio::time::sleep(config.poll_interval) => {}
                        }
                    }
                });
            }
        }));
        Ok(self)
    }

    /// Stop starting work on shutdown and let current calls finish.
    /// An unexpected worker exit shuts down its siblings and returns an error.
    /// Dropping this future aborts all workers.
    pub async fn run(self, shutdown: impl Future<Output = ()>) -> anyhow::Result<()> {
        ensure!(!self.starters.is_empty(), "no processors registered");
        let (stop, stopped) = watch::channel(false);
        let mut workers = JoinSet::new();
        for start in self.starters {
            start(&mut workers, stopped.clone());
        }
        tokio::pin!(shutdown);
        let failure = tokio::select! {
            () = &mut shutdown => None,
            result = workers.join_next() => Some(anyhow::anyhow!("worker stopped unexpectedly: {result:?}")),
        };
        let _ = stop.send(true);
        let mut failure = failure;
        while let Some(result) = workers.join_next().await {
            if let Err(error) = result.context("worker panicked during shutdown") {
                failure.get_or_insert(error);
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{Semaphore, mpsc, oneshot};

    struct Blocking<const ID: usize> {
        entered: mpsc::UnboundedSender<()>,
        release: Arc<Semaphore>,
        completed: Arc<AtomicUsize>,
    }

    impl<const ID: usize> Processor for Blocking<ID> {
        async fn process_next(&self) -> anyhow::Result<Step> {
            self.entered.send(())?;
            self.release.acquire().await?.forget();
            self.completed.fetch_add(1, Ordering::SeqCst);
            Ok(Step::Idle)
        }
    }

    #[tokio::test]
    async fn starts_each_processors_workers_and_drains_on_shutdown() -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(5), async {
            let (entered, mut starts) = mpsc::unbounded_channel();
            let release = Arc::new(Semaphore::new(0));
            let completed = Arc::new(AtomicUsize::new(0));
            let runner = Runner::new()
                .register(
                    Blocking::<0> {
                        entered: entered.clone(),
                        release: release.clone(),
                        completed: completed.clone(),
                    },
                    WorkerConfig {
                        workers: 1,
                        poll_interval: Duration::from_secs(60),
                    },
                )?
                .register(
                    Blocking::<1> {
                        entered: entered.clone(),
                        release: release.clone(),
                        completed: completed.clone(),
                    },
                    WorkerConfig {
                        workers: 2,
                        poll_interval: Duration::from_secs(60),
                    },
                )?;
            let (stop, stopped) = oneshot::channel();
            let (seen, shutdown_seen) = oneshot::channel();
            let task = tokio::spawn(runner.run(async {
                let _ = stopped.await;
                let _ = seen.send(());
            }));
            for _ in 0..3 {
                assert_eq!(starts.recv().await, Some(()));
            }
            stop.send(()).unwrap();
            shutdown_seen.await?;
            assert!(!task.is_finished(), "shutdown must wait for active calls");
            release.add_permits(3);
            task.await??;
            assert_eq!(completed.load(Ordering::SeqCst), 3);
            assert!(starts.try_recv().is_err());
            anyhow::Ok(())
        })
        .await?
    }

    struct Sequence(AtomicUsize);
    impl Processor for Sequence {
        async fn process_next(&self) -> anyhow::Result<Step> {
            tokio::task::yield_now().await;
            match self.0.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(Step::Processed),
                1 => anyhow::bail!("temporary database failure"),
                _ => Ok(Step::Idle),
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn processed_repeats_immediately_errors_and_idle_wait() -> anyhow::Result<()> {
        struct Shared(Arc<Sequence>);
        impl Processor for Shared {
            async fn process_next(&self) -> anyhow::Result<Step> {
                self.0.process_next().await
            }
        }
        let sequence = Arc::new(Sequence(AtomicUsize::new(0)));
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(
            Runner::new()
                .register(
                    Shared(sequence.clone()),
                    WorkerConfig {
                        workers: 1,
                        poll_interval: Duration::from_secs(10),
                    },
                )?
                .run(async {
                    let _ = stopped.await;
                }),
        );
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(sequence.0.load(Ordering::SeqCst), 2);
        tokio::time::advance(Duration::from_secs(10)).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(sequence.0.load(Ordering::SeqCst), 3);
        tokio::time::advance(Duration::from_secs(10)).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(sequence.0.load(Ordering::SeqCst), 4);
        stop.send(()).unwrap();
        task.await??;
        Ok(())
    }

    struct Panics;
    impl Processor for Panics {
        async fn process_next(&self) -> anyhow::Result<Step> {
            tokio::task::yield_now().await;
            panic!("broken processor")
        }
    }

    #[tokio::test]
    async fn unexpected_worker_exit_is_reported() -> anyhow::Result<()> {
        let result = Runner::new()
            .register(Panics, WorkerConfig::default())?
            .run(std::future::pending())
            .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("worker stopped unexpectedly")
        );
        Ok(())
    }
}
