//! Process-owned text agent. One transport serializes presence and observations.
//! Window focus/visibility never participates in this lifecycle.

use super::{Observation, ObservationQueue, agent_id, observer_loop, post_json};
use log::{debug, warn};
use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::async_runtime::JoinHandle as Task;

const PRESENCE_INTERVAL: Duration = Duration::from_secs(20);
const RETRY_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObserverPhase {
    Idle,
    Active { input_pane: bool, uia: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Capability {
    generation: u64,
    enabled: bool,
    phase: ObserverPhase,
}

impl Capability {
    fn body(self) -> serde_json::Value {
        let (input_pane, uia) = match self.phase {
            ObserverPhase::Active { input_pane, uia } => (input_pane, uia),
            _ => (false, false),
        };
        serde_json::json!({
            "version": 1, "agent": self.enabled, "agent_id": agent_id(),
            "supports_text_context": true, "generation": self.generation,
            "observer_active": matches!(self.phase, ObserverPhase::Active { .. }),
            "input_pane": input_pane, "uia": uia,
        })
    }
}

pub(super) struct Shared {
    capability: Mutex<Capability>,
    pub(super) queue: ObservationQueue,
}

impl Shared {
    fn new() -> Self {
        Self {
            capability: Mutex::new(Capability {
                generation: 1,
                enabled: true,
                phase: ObserverPhase::Idle,
            }),
            queue: ObservationQueue::default(),
        }
    }

    fn snapshot(&self) -> Capability {
        *self.capability.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn take_observations(&self, expected: Capability) -> Option<Vec<Observation>> {
        // Match publish's lock order: replacing/readying an observer must not
        // slip between capability validation and draining its first result.
        let capability = self.capability.lock().unwrap_or_else(|e| e.into_inner());
        (*capability == expected).then(|| self.queue.take())
    }

    fn set_enabled(&self, enabled: bool) {
        let mut capability = self.capability.lock().unwrap_or_else(|e| e.into_inner());
        if capability.enabled != enabled {
            capability.enabled = enabled;
            capability.generation += 1;
            capability.phase = ObserverPhase::Idle;
            self.queue.clear();
            self.queue.notify.notify_one();
        }
    }

    fn begin(&self) -> u64 {
        let mut capability = self.capability.lock().unwrap_or_else(|e| e.into_inner());
        // Only begin grants a worker this generation. Stop advances it even
        // when initialization has not reached ready, so a Starting flag is redundant.
        capability.generation += 1;
        capability.phase = ObserverPhase::Idle;
        self.queue.clear();
        self.queue.notify.notify_one();
        capability.generation
    }

    pub(super) fn ready(&self, generation: u64, input_pane: bool, uia: bool) -> bool {
        let mut capability = self.capability.lock().unwrap_or_else(|e| e.into_inner());
        if capability.generation != generation {
            return false;
        }
        capability.phase = ObserverPhase::Active { input_pane, uia };
        self.queue.notify.notify_one();
        true
    }

    pub(super) fn finished(&self, generation: u64) {
        let mut capability = self.capability.lock().unwrap_or_else(|e| e.into_inner());
        if capability.generation == generation {
            // Invalidate both queued observations and delayed COM completion.
            capability.generation += 1;
            capability.phase = ObserverPhase::Idle;
            self.queue.clear();
            self.queue.notify.notify_one();
        }
    }

    pub(super) fn publish(&self, observation: Observation) {
        let capability = self.capability.lock().unwrap_or_else(|e| e.into_inner());
        if capability.generation == observation.generation
            && matches!(capability.phase, ObserverPhase::Active { .. })
        {
            self.queue.publish(observation);
        }
    }
}

struct Observer {
    generation: u64,
    stop: Arc<AtomicBool>,
}

struct ObserverLifetime(Arc<Shared>, u64);

impl Drop for ObserverLifetime {
    fn drop(&mut self) {
        self.0.finished(self.1);
    }
}

impl Observer {
    fn start(shared: &Arc<Shared>) -> Option<Self> {
        let generation = shared.begin();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_shared = shared.clone();
        match std::thread::Builder::new()
            .name("text-context-observer".into())
            .spawn(move || {
                let _lifetime = ObserverLifetime(worker_shared.clone(), generation);
                observer_loop(generation, worker_stop, worker_shared);
            }) {
            Ok(_) => Some(Self { generation, stop }),
            Err(error) => {
                shared.finished(generation);
                warn!("Failed to start text context observer: {error}");
                None
            }
        }
    }

    fn stop(self, shared: &Shared) {
        shared.finished(self.generation);
        self.stop.store(true, Ordering::Release);
        // COM providers can block indefinitely. The detached worker releases
        // its own resources when it returns; generation checks reject late results.
    }
}

struct Agent {
    shared: Arc<Shared>,
    transport: Task<()>,
    observer: Option<Observer>,
    stream_active: bool,
}

static AGENT: Mutex<Option<Agent>> = Mutex::new(None);

pub fn start(enabled: bool) {
    let mut owner = AGENT.lock().unwrap_or_else(|e| e.into_inner());
    if owner.is_some() {
        return;
    }
    let shared = Arc::new(Shared::new());
    shared.set_enabled(enabled);
    let worker_shared = shared.clone();
    let transport = tauri::async_runtime::spawn(async move {
        network_loop(worker_shared, |path, body| async move {
            post_json(path, &body).await
        })
        .await;
    });
    *owner = Some(Agent {
        shared,
        transport,
        observer: None,
        stream_active: false,
    });
}

fn should_observe(state: Option<&crate::sunshine::TrayState>) -> bool {
    state.is_some_and(|state| {
        state.status == "streaming"
            && (!state
                .capabilities
                .iter()
                .any(|capability| capability == "sessions-v1")
                || !state.sessions.is_empty())
    })
}

pub fn reconcile_streaming_state(state: Option<&crate::sunshine::TrayState>) {
    let mut owner = AGENT.lock().unwrap_or_else(|e| e.into_inner());
    // Late tray events must not restart services after process shutdown.
    let Some(agent) = owner.as_mut() else {
        return;
    };
    agent.stream_active = should_observe(state);
    agent.reconcile(Observer::start);
}

impl Agent {
    fn reconcile(&mut self, start_observer: impl FnOnce(&Arc<Shared>) -> Option<Observer>) {
        let capability = self.shared.snapshot();
        let active = self.stream_active && capability.enabled;
        // Worker exit already revokes its generation, including during COM
        // initialization. Retire that owner before deciding whether to restart.
        if let Some(observer) = self
            .observer
            .take_if(|observer| !active || observer.generation != capability.generation)
        {
            observer.stop(&self.shared);
        }
        if active && self.observer.is_none() {
            self.observer = start_observer(&self.shared);
        }
    }
}

pub fn set_enabled(enabled: bool) {
    let mut owner = AGENT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(agent) = owner.as_mut() {
        agent.shared.set_enabled(enabled);
        agent.reconcile(Observer::start);
    }
}

pub fn stop() {
    let Some(agent) = AGENT.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    if let Some(observer) = agent.observer {
        observer.stop(&agent.shared);
    }
    agent.transport.abort();
}

async fn network_loop<F, Fut>(shared: Arc<Shared>, mut post: F)
where
    F: FnMut(&'static str, serde_json::Value) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let mut heartbeat = tokio::time::interval(PRESENCE_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    shared.queue.notify.notify_one(); // initial presence, then one heartbeat per interval
    let mut sent = None;
    let mut backoff = RETRY_INTERVAL;
    loop {
        let presence_due = tokio::select! {
            _ = heartbeat.tick() => true,
            _ = shared.queue.notify.notified() => false,
        };
        let snapshot = shared.snapshot();
        if presence_due || sent != Some(snapshot) {
            if let Err(error) = post("/api/v1/text-context/capability", snapshot.body()).await {
                debug!("Text context capability failed: {error}");
                sent = None;
                let retry_at = tokio::time::Instant::now() + backoff;
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep_until(retry_at) => {
                            backoff = (backoff * 2).min(PRESENCE_INTERVAL);
                            break;
                        }
                        _ = shared.queue.notify.notified() => {
                            // New lifecycle state should be announced promptly;
                            // ordinary observations cannot bypass offline backoff.
                            if shared.snapshot() != snapshot {
                                backoff = RETRY_INTERVAL;
                                break;
                            }
                        }
                    }
                }
                shared.queue.notify.notify_one();
                continue;
            }
            backoff = RETRY_INTERVAL;
            sent = Some(snapshot);
        }
        let Some(observations) = shared.take_observations(snapshot) else {
            shared.queue.notify.notify_one();
            continue;
        };
        // Presence and observations share this task, so the active generation
        // is always acknowledged before its first observation is submitted.
        for observation in observations {
            if shared.snapshot() != snapshot
                || observation.generation != snapshot.generation
                || !matches!(snapshot.phase, ObserverPhase::Active { .. })
            {
                continue;
            }
            if sent.is_none() {
                shared.queue.requeue_if_empty(observation);
                continue;
            }
            if let Err(error) = post(
                "/api/v1/text-context/observation",
                serde_json::to_value(&observation).unwrap(),
            )
            .await
            {
                debug!("Text context observation failed: {error}");
                sent = None;
                if shared.snapshot() == snapshot {
                    shared.queue.requeue_if_empty(observation);
                }
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn capability_retries_back_off_to_heartbeat_interval_and_reset_after_success() {
        let shared = Arc::new(Shared::new());
        let worker_shared = shared.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = tokio::spawn(async move {
            network_loop(worker_shared, move |path, _| {
                assert!(path.ends_with("/capability"));
                let (reply, result) = tokio::sync::oneshot::channel();
                tx.send((tokio::time::Instant::now(), reply)).unwrap();
                async move { result.await.unwrap() }
            })
            .await;
        });
        let (mut previous, mut reply) = rx.recv().await.unwrap();
        for delay in [
            RETRY_INTERVAL,
            RETRY_INTERVAL * 2,
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8),
            Duration::from_secs(16),
            PRESENCE_INTERVAL,
            PRESENCE_INTERVAL,
        ] {
            reply.send(Err("offline".into())).unwrap();
            let (now, next) = rx.recv().await.unwrap();
            assert_eq!(now - previous, delay);
            (previous, reply) = (now, next);
        }
        // A preference/lifecycle update interrupts a long offline wait.
        reply.send(Err("still offline".into())).unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        shared.set_enabled(false);
        let (now, reply) = rx.recv().await.unwrap();
        assert_eq!(now - previous, Duration::from_secs(1));
        reply.send(Ok(())).unwrap();
        shared.set_enabled(true); // A new capability after a successful POST.
        let (previous, reply) = rx.recv().await.unwrap();
        reply.send(Err("offline again".into())).unwrap();
        let (now, _reply) = rx.recv().await.unwrap();
        assert_eq!(now - previous, RETRY_INTERVAL);
        worker.abort();
    }

    #[test]
    fn stale_capability_cannot_drain_the_new_observer_baseline() {
        let shared = Shared::new();
        let generation = shared.begin();
        let initializing = shared.snapshot();
        shared.ready(generation, false, true);
        let active = shared.snapshot();
        let mut baseline = super::super::tests::observation("uia", true);
        baseline.generation = generation;
        shared.publish(baseline.clone());
        assert!(shared.take_observations(initializing).is_none());
        assert_eq!(shared.take_observations(active).unwrap().len(), 1);

        shared.finished(generation);
        let next = shared.begin();
        shared.ready(next, false, true);
        baseline.generation = next;
        shared.publish(baseline);
        assert!(shared.take_observations(active).is_none());
        let pending = shared.take_observations(shared.snapshot()).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].generation, next);
    }

    #[test]
    fn observer_requires_video_stream_and_stops_on_pause_or_disconnect() {
        let mut state = crate::sunshine::TrayState {
            status: "streaming".into(),
            ..Default::default()
        };
        assert!(should_observe(Some(&state))); // legacy Core
        state.capabilities.push("sessions-v1".into());
        assert!(!should_observe(Some(&state))); // control-only session
        state.sessions.push(Default::default());
        assert!(should_observe(Some(&state)));
        for status in ["paused", "idle", "stopped"] {
            state.status = status.into();
            assert!(!should_observe(Some(&state)));
        }
        assert!(!should_observe(None));
    }

    #[test]
    fn disabling_invalidates_initialization_and_publishes_absence_until_reenabled() {
        let shared = Shared::new();
        let generation = shared.begin();
        shared.set_enabled(false);
        assert!(!shared.ready(generation, true, true));
        assert_eq!(shared.snapshot().body()["agent"], false);
        assert_eq!(shared.snapshot().body()["observer_active"], false);
        shared.set_enabled(true);
        assert!(!shared.ready(generation, true, true));
        let new = shared.begin();
        assert!(shared.ready(new, true, true));
        assert_eq!(shared.snapshot().body()["agent"], true);
        assert_eq!(shared.snapshot().body()["observer_active"], true);
    }

    #[tokio::test]
    async fn failed_observation_reannounces_capability_before_retrying() {
        let shared = Arc::new(Shared::new());
        let generation = shared.begin();
        shared.ready(generation, false, true);
        let mut observation = super::super::tests::observation("uia", true);
        observation.generation = generation;
        shared.publish(observation);
        let mut pane = super::super::tests::observation("input_pane", true);
        pane.generation = generation;
        shared.publish(pane);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut failed = false;
        let worker = tokio::spawn(async move {
            network_loop(shared, move |path, _| {
                tx.send(path).unwrap();
                let fail = path.ends_with("/observation") && !failed;
                if fail {
                    failed = true;
                }
                async move {
                    if fail {
                        Err("connection reset".into())
                    } else {
                        Ok(())
                    }
                }
            })
            .await;
        });
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            for endpoint in [
                "/capability",
                "/observation",
                "/capability",
                "/observation",
                "/observation",
            ] {
                assert!(rx.recv().await.unwrap().ends_with(endpoint));
            }
        })
        .await;
        worker.abort();
        result.unwrap();
    }

    #[test]
    fn old_provider_result_cannot_overwrite_new_observer_baseline() {
        let shared = Shared::new();
        let old = shared.begin();
        shared.ready(old, false, true);
        let mut old_result = super::super::tests::observation("uia", true);
        old_result.generation = old;
        shared.finished(old);
        let new = shared.begin();
        shared.ready(new, false, true);
        let mut baseline = super::super::tests::observation("uia", false);
        baseline.generation = new;
        shared.publish(baseline);
        shared.publish(old_result);
        let pending = shared.queue.take();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].generation, new);
        assert!(!pending[0].active);
    }

    #[test]
    fn unexpected_observer_exit_clears_advertised_capability() {
        let shared = Arc::new(Shared::new());
        let generation = shared.begin();
        shared.ready(generation, true, true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _lifetime = ObserverLifetime(shared.clone(), generation);
            panic!("provider failed");
        }));
        assert!(result.is_err());
        assert_eq!(shared.snapshot().phase, ObserverPhase::Idle);
        assert!(shared.snapshot().generation > generation);
    }

    #[tokio::test]
    async fn reconciliation_replaces_exited_observers_without_restarting_live_initialization() {
        let shared = Arc::new(Shared::new());
        let mut agent = Agent {
            shared: shared.clone(),
            transport: tauri::async_runtime::spawn(std::future::pending()),
            observer: None,
            stream_active: true,
        };
        // Exercise ownership without calling real COM providers in the test.
        let starts = std::cell::Cell::new(0);
        let start = |shared: &Arc<Shared>| {
            starts.set(starts.get() + 1);
            Some(Observer {
                generation: shared.begin(),
                stop: Arc::new(AtomicBool::new(false)),
            })
        };
        agent.reconcile(start);
        let old = agent.observer.as_ref().unwrap().generation;
        let old_stop = agent.observer.as_ref().unwrap().stop.clone();
        agent.reconcile(start);
        assert_eq!(starts.get(), 1); // Idle also covers initialization in progress.

        drop(ObserverLifetime(shared.clone(), old)); // Failed initialization/early exit.
        agent.reconcile(start);
        assert_eq!(starts.get(), 2);
        assert!(old_stop.load(Ordering::Acquire));
        let new = agent.observer.as_ref().unwrap().generation;
        assert!(new > old);
        shared.ready(new, false, true);
        shared.finished(old); // Late cleanup cannot retire the replacement.
        agent.reconcile(start);
        assert_eq!(starts.get(), 2);

        shared.set_enabled(false);
        let new_stop = agent.observer.as_ref().unwrap().stop.clone();
        agent.reconcile(start);
        assert!(agent.observer.is_none());
        assert!(new_stop.load(Ordering::Acquire));
        assert_eq!(starts.get(), 2);
        shared.set_enabled(true);
        agent.reconcile(start);
        assert_eq!(starts.get(), 3);
        agent.stream_active = false;
        agent.reconcile(start);
        assert!(agent.observer.is_none());
        agent.transport.abort();
    }

    #[tokio::test]
    async fn generation_change_during_capability_post_does_not_send_old_observations() {
        let shared = Arc::new(Shared::new());
        let old = shared.begin();
        shared.ready(old, false, true);
        let mut observation = super::super::tests::observation("uia", true);
        observation.generation = old;
        shared.publish(observation.clone());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (release, gate) = tokio::sync::oneshot::channel();
        let worker_shared = shared.clone();
        let mut gate = Some(gate);
        let worker = tokio::spawn(async move {
            network_loop(worker_shared, move |path, body| {
                tx.send((path, body)).unwrap();
                let gate = gate.take();
                async move {
                    if let Some(gate) = gate {
                        gate.await.unwrap();
                    }
                    Ok(())
                }
            })
            .await;
        });
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            let (path, body) = rx.recv().await.unwrap();
            assert!(path.ends_with("/capability"));
            assert_eq!(body["generation"], old);
            shared.finished(old);
            let new = shared.begin();
            shared.ready(new, false, true);
            observation.generation = new;
            shared.publish(observation);
            release.send(()).unwrap();
            // Capability and queue updates coalesced while the old POST was blocked.
            let (path, body) = rx.recv().await.unwrap();
            assert!(path.ends_with("/capability"));
            assert_eq!(body["generation"], new);
            let (path, body) = rx.recv().await.unwrap();
            assert!(path.ends_with("/observation"));
            assert_eq!(body["generation"], new);
        })
        .await;
        worker.abort();
        result.unwrap();
    }

    #[test]
    fn stopped_generation_cannot_reactivate_or_clear_a_new_observer() {
        let shared = Shared::new();
        let old = shared.begin();
        let stop = Arc::new(AtomicBool::new(false));
        Observer {
            generation: old,
            stop: stop.clone(),
        }
        .stop(&shared);
        assert!(stop.load(Ordering::Acquire));
        let idle = shared.snapshot();
        assert!(!shared.ready(old, true, true)); // stopped before initialization completed
        shared.finished(old);
        assert_eq!(shared.snapshot(), idle); // repeated stop is inert
        let new = shared.begin();
        assert!(!shared.ready(old, true, true));
        assert!(shared.ready(new, false, true));
        shared.finished(old);
        assert_eq!(
            shared.snapshot().phase,
            ObserverPhase::Active {
                input_pane: false,
                uia: true
            }
        );
        let body = shared.snapshot().body();
        assert_eq!(body["generation"], new);
        assert_eq!(body["agent"], true);
        assert_eq!(body["observer_active"], true);
        assert!(!body["agent_id"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn transport_orders_capability_before_observation_and_retries_without_losing_baseline() {
        let shared = Arc::new(Shared::new());
        let generation = shared.begin();
        shared.ready(generation, false, true);
        let mut observation = super::super::tests::observation("uia", true);
        observation.generation = generation;
        shared.publish(observation);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let worker_shared = shared.clone();
        let mut fail_once = true;
        let worker = tokio::spawn(async move {
            network_loop(worker_shared, move |path, body| {
                tx.send((path, body)).unwrap();
                let fail = std::mem::take(&mut fail_once);
                async move { if fail { Err("offline".into()) } else { Ok(()) } }
            })
            .await;
        });
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            assert!(rx.recv().await.unwrap().0.ends_with("/capability"));
            assert!(rx.recv().await.unwrap().0.ends_with("/capability"));
            let (path, body) = rx.recv().await.unwrap();
            assert!(path.ends_with("/observation"));
            assert_eq!(body["generation"], generation);
            shared.finished(generation);
            let (_, idle) = loop {
                let message = rx.recv().await.unwrap();
                if message.0.ends_with("/capability")
                    && message.1["generation"].as_u64().unwrap() > generation
                {
                    break message;
                }
            };
            assert_eq!(idle["observer_active"], false);
            assert!(idle["generation"].as_u64().unwrap() > generation);
        })
        .await;
        worker.abort();
        result.unwrap();
    }
}
