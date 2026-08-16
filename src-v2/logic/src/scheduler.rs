/// # Persistent Scheduler — Time-Based Task Rotation
///
/// The scheduler runs alongside the behaviour tree and manages:
/// - Field rotation (20–30 min intervals)
/// - AFK / blink simulation (random pauses every 3–7 min)
/// - Session time tracking (auto-stop after configurable play sessions)

use std::time::{Duration, Instant};
use rand::Rng;
use log::info;

/// How often the scheduler evaluates its tasks.
pub const SCHEDULER_TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Minimum / maximum interval between forced AFK pauses (3–7 min).
pub const AFK_PAUSE_MIN: Duration = Duration::from_secs(3 * 60);
pub const AFK_PAUSE_MAX: Duration = Duration::from_secs(7 * 60);

/// Duration of a simulated "blink" / pause (2–5 seconds).
pub const BLINK_DURATION_MIN: Duration = Duration::from_secs(2);
pub const BLINK_DURATION_MAX: Duration = Duration::from_secs(5);

/// Maximum session length before forced stop (4 hours).
pub const MAX_SESSION_DURATION: Duration = Duration::from_secs(4 * 3600);

/// A task tracked by the scheduler.
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    pub name: &'static str,
    pub interval_min: Duration,
    pub interval_max: Duration,
    pub last_run: Instant,
    /// The action to perform. Returns true if the action was completed.
    pub action: fn() -> bool,
}

impl ScheduledTask {
    pub fn due(&self) -> bool {
        let elapsed = self.last_run.elapsed();
        let next_in = rand::thread_rng().gen_range(
            self.interval_min.as_secs()..=self.interval_max.as_secs()
        );
        elapsed >= Duration::from_secs(next_in)
    }

    pub fn execute(&mut self) {
        info!("[SCHEDULER] Running task: {}", self.name);
        (self.action)();
        self.last_run = Instant::now();
    }
}

/// Anti-detection noise engine.
pub struct HumanizationLayer {
    /// Timestamp of the last AFK pause.
    last_afk: Instant,
    /// Timestamp of the last micro-adjustment.
    last_micro: Instant,
}

impl HumanizationLayer {
    pub fn new() -> Self {
        Self {
            last_afk: Instant::now(),
            last_micro: Instant::now(),
        }
    }

    /// Returns true if the engine should insert a brief idle pause.
    pub fn should_afk_pause(&self) -> bool {
        let elapsed = self.last_afk.elapsed();
        let threshold = rand::thread_rng().gen_range(
            AFK_PAUSE_MIN.as_secs()..=AFK_PAUSE_MAX.as_secs()
        );
        elapsed >= Duration::from_secs(threshold)
    }

    /// Record that an AFK pause just happened.
    pub fn did_afk_pause(&mut self) {
        self.last_afk = Instant::now();
    }

    /// Duration of the next "blink" (human-like idle).
    pub fn blink_duration(&self) -> Duration {
        Duration::from_millis(
            rand::thread_rng().gen_range(
                BLINK_DURATION_MIN.as_millis() as u64..=BLINK_DURATION_MAX.as_millis() as u64
            )
        )
    }

    /// Should we insert a micro-adjustment (tiny cursor jitter)?
    pub fn should_micro_adjust(&self) -> bool {
        self.last_micro.elapsed() > Duration::from_secs(
            rand::thread_rng().gen_range(8..25)
        )
    }

    pub fn did_micro_adjust(&mut self) {
        self.last_micro = Instant::now();
    }
}

/// The master scheduler.
pub struct Scheduler {
    pub tasks: Vec<ScheduledTask>,
    pub humanization: HumanizationLayer,
    pub session_start: Instant,
    pub running: bool,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            humanization: HumanizationLayer::new(),
            session_start: Instant::now(),
            running: true,
        }
    }

    pub fn add_task(&mut self, task: ScheduledTask) {
        info!("[SCHEDULER] Registered task: {}", task.name);
        self.tasks.push(task);
    }

    /// Called every tick. Checks all tasks and executes due ones.
    pub fn tick(&mut self) {
        // Check session time
        if self.session_start.elapsed() >= MAX_SESSION_DURATION {
            info!("[SCHEDULER] Max session duration reached — stopping");
            self.running = false;
            return;
        }

        // Run due tasks
        for task in &mut self.tasks {
            if task.due() {
                task.execute();
            }
        }

        // Humanization
        if self.humanization.should_afk_pause() {
            info!("[SCHEDULER] AFK pause ({}s)",
                  self.humanization.blink_duration().as_secs());
            self.humanization.did_afk_pause();
        }
    }
}
