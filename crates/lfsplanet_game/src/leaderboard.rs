use std::{error::Error, future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use parking_lot::RwLock;

/// Error returned by a leaderboard store.
pub type LeaderboardError = Box<dyn Error + Send + Sync>;

/// Result returned by a leaderboard store.
pub type LeaderboardResult<T> = Result<T, LeaderboardError>;

/// Storage for classified standings and the submissions which produce them.
pub trait LeaderboardStore<Standing, Submission>: Send + Sync {
    /// Returns the currently classified standings.
    fn standings(
        &self,
    ) -> Pin<Box<dyn Future<Output = LeaderboardResult<Vec<Standing>>> + Send + '_>>;

    /// Records a submission.
    fn record(
        &self,
        submission: Submission,
    ) -> Pin<Box<dyn Future<Output = LeaderboardResult<()>> + Send + '_>>;
}

/// Shared handle to a leaderboard store.
pub type Leaderboard<Standing, Submission> = Arc<dyn LeaderboardStore<Standing, Submission>>;

/// In-memory leaderboard updated by a domain-specific reducer.
pub struct InMemoryLeaderboardStore<Standing, Submission, Reducer> {
    standings: RwLock<Vec<Standing>>,
    reducer: Reducer,
    submission: PhantomData<fn(Submission)>,
}

impl<Standing, Submission, Reducer> InMemoryLeaderboardStore<Standing, Submission, Reducer> {
    /// Creates an empty in-memory leaderboard.
    pub fn new(reducer: Reducer) -> Self {
        Self {
            standings: RwLock::new(Vec::new()),
            reducer,
            submission: PhantomData,
        }
    }
}

impl<Standing, Submission, Reducer> std::fmt::Debug
    for InMemoryLeaderboardStore<Standing, Submission, Reducer>
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InMemoryLeaderboardStore")
            .finish_non_exhaustive()
    }
}

impl<Standing, Submission, Reducer> LeaderboardStore<Standing, Submission>
    for InMemoryLeaderboardStore<Standing, Submission, Reducer>
where
    Standing: Clone + Send + Sync + 'static,
    Submission: Send + 'static,
    Reducer: Fn(&mut Vec<Standing>, Submission) + Send + Sync + 'static,
{
    fn standings(
        &self,
    ) -> Pin<Box<dyn Future<Output = LeaderboardResult<Vec<Standing>>> + Send + '_>> {
        let standings = self.standings.read().clone();
        Box::pin(async move { Ok(standings) })
    }

    fn record(
        &self,
        submission: Submission,
    ) -> Pin<Box<dyn Future<Output = LeaderboardResult<()>> + Send + '_>> {
        (self.reducer)(&mut self.standings.write(), submission);
        Box::pin(async { Ok(()) })
    }
}
