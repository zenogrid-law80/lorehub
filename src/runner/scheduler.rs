//! Stage barriers and explicit job dependencies, with bounded parallelism.
use std::{collections::HashSet, future::Future};

use anyhow::{Result, anyhow, ensure};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::ci::config::PipelineConfig;

pub(super) async fn run_jobs<F, Fut>(
    config: &PipelineConfig,
    cancel: &CancellationToken,
    mut run: F,
) -> Result<()>
where
    F: FnMut(usize, CancellationToken) -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    ensure!(
        (1..=16).contains(&config.max_parallel_jobs),
        "invalid job concurrency"
    );
    let running_cancel = cancel.child_token();
    let mut running = JoinSet::new();
    let mut started = vec![false; config.jobs.len()];
    let mut completed = HashSet::new();
    let result = async {
        for stage in &config.stages {
            loop {
                ensure!(!cancel.is_cancelled(), "pipeline canceled");
                for (index, job) in config.jobs.iter().enumerate() {
                    if running.len() >= config.max_parallel_jobs {
                        break;
                    }
                    if &job.stage != stage
                        || started[index]
                        || !job.needs.iter().all(|name| completed.contains(name))
                    {
                        continue;
                    }
                    started[index] = true;
                    let future = run(index, running_cancel.clone());
                    running.spawn(async move { (index, future.await) });
                }
                if running.is_empty() {
                    ensure!(
                        config
                            .jobs
                            .iter()
                            .enumerate()
                            .all(|(index, job)| &job.stage != stage || started[index]),
                        "job dependencies cannot make progress"
                    );
                    break;
                }
                let joined = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Err(anyhow!("pipeline canceled")),
                    joined = running.join_next() => joined.expect("running set is not empty"),
                };
                let (index, outcome) = joined?;
                outcome?;
                completed.insert(config.jobs[index].name.clone());
            }
        }
        Ok(())
    }
    .await;
    if result.is_err() {
        // Let every executor kill and reap its process tree before its shared
        // checkout is deleted or the pipeline completion is reported.
        running_cancel.cancel();
        while running.join_next().await.is_some() {}
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };
    use tokio::sync::{Semaphore, mpsc};

    fn config(limit: usize) -> PipelineConfig {
        PipelineConfig::parse(&format!(
            r#"
max_parallel_jobs = {limit}
stages = ["build", "test"]
[[jobs]]
name = "a"
stage = "build"
script = ["a"]
[[jobs]]
name = "b"
stage = "build"
script = ["b"]
[[jobs]]
name = "c"
stage = "build"
needs = ["a"]
script = ["c"]
[[jobs]]
name = "d"
stage = "build"
script = ["d"]
[[jobs]]
name = "e"
stage = "test"
script = ["e"]
"#
        ))
        .unwrap()
    }

    struct Controlled {
        task: tokio::task::JoinHandle<Result<()>>,
        started: mpsc::UnboundedReceiver<usize>,
        gates: Vec<Arc<Semaphore>>,
        cleaned: Arc<AtomicUsize>,
    }

    fn controlled(limit: usize, cancel: CancellationToken, fail: Option<usize>) -> Controlled {
        let (started, receiver) = mpsc::unbounded_channel();
        let gates: Vec<_> = (0..5).map(|_| Arc::new(Semaphore::new(0))).collect();
        let gates_for_tasks = gates.clone();
        let cleaned = Arc::new(AtomicUsize::new(0));
        let cleanup = cleaned.clone();
        let task = tokio::spawn(async move {
            run_jobs(&config(limit), &cancel, |index, token| {
                let gate = gates_for_tasks[index].clone();
                let started = started.clone();
                let cleanup = cleanup.clone();
                async move {
                    started.send(index).unwrap();
                    tokio::select! {
                        _ = token.cancelled() => {
                            tokio::task::yield_now().await;
                            cleanup.fetch_add(1, Ordering::SeqCst);
                            Err(anyhow!("canceled"))
                        }
                        permit = gate.acquire() => {
                            permit.unwrap().forget();
                            ensure!(fail != Some(index), "job {index} failed");
                            Ok(())
                        }
                    }
                }
            })
            .await
        });
        Controlled {
            task,
            started: receiver,
            gates,
            cleaned,
        }
    }

    async fn next(receiver: &mut mpsc::UnboundedReceiver<usize>) -> usize {
        tokio::time::timeout(Duration::from_secs(3), receiver.recv())
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn parallel_jobs_obey_capacity_dependencies_and_stage_barriers() {
        let mut run = controlled(2, CancellationToken::new(), None);
        let mut first = vec![next(&mut run.started).await, next(&mut run.started).await];
        first.sort();
        assert_eq!(first, [0, 1]);
        run.gates[0].add_permits(1);
        assert_eq!(next(&mut run.started).await, 2); // a is done; b need not finish for c.
        run.gates[2].add_permits(1);
        assert_eq!(next(&mut run.started).await, 3);
        run.gates[3].add_permits(1);
        tokio::task::yield_now().await;
        assert!(run.started.try_recv().is_err()); // test stage still waits for b.
        run.gates[1].add_permits(1);
        assert_eq!(next(&mut run.started).await, 4);
        run.gates[4].add_permits(1);
        tokio::time::timeout(Duration::from_secs(3), run.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn serial_configuration_keeps_existing_order() {
        let mut run = controlled(1, CancellationToken::new(), None);
        for index in 0..5 {
            assert_eq!(next(&mut run.started).await, index);
            assert!(run.started.try_recv().is_err());
            run.gates[index].add_permits(1);
        }
        tokio::time::timeout(Duration::from_secs(3), run.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn failure_cancels_and_drains_siblings_before_returning() {
        let parent = CancellationToken::new();
        let mut run = controlled(2, parent.clone(), Some(0));
        next(&mut run.started).await;
        next(&mut run.started).await;
        run.gates[0].add_permits(1);
        let error = tokio::time::timeout(Duration::from_secs(3), run.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains("job 0 failed"));
        assert_eq!(run.cleaned.load(Ordering::SeqCst), 1);
        assert!(run.started.try_recv().is_err());
        assert!(!parent.is_cancelled()); // Pipeline heartbeat stays alive for finish().
    }

    #[tokio::test]
    async fn cancellation_drains_all_running_jobs_and_never_starts_dependents() {
        let parent = CancellationToken::new();
        let mut run = controlled(2, parent.clone(), None);
        next(&mut run.started).await;
        next(&mut run.started).await;
        parent.cancel();
        assert!(
            tokio::time::timeout(Duration::from_secs(3), run.task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(run.cleaned.load(Ordering::SeqCst), 2);
        assert!(run.started.try_recv().is_err());
    }
}
