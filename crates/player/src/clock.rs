use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const RESYNC: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    Show,
    Late,
    Stopped,
}

#[derive(Debug, PartialEq, Eq)]
enum Plan {
    Anchor,
    Wait(Duration),
    Show,
    Late,
}

#[derive(Default)]
struct State {
    anchor: Option<(Instant, u64)>,
    paused: bool,
    stopped: bool,
}

pub struct Clock {
    state: Mutex<State>,
    changed: Condvar,
    max_lateness: Duration,
}

impl Clock {
    pub fn new(max_lateness: Duration) -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            max_lateness,
        }
    }

    pub fn pause(&self) {
        let mut state = self.lock();
        state.paused = true;
        state.anchor = None;
    }

    pub fn resume(&self) {
        self.lock().paused = false;
        self.changed.notify_all();
    }

    pub fn stop(&self) {
        self.lock().stopped = true;
        self.changed.notify_all();
    }

    pub fn wait_for(&self, pts_us: u64) -> Due {
        let mut state = self.lock();
        loop {
            if state.stopped {
                return Due::Stopped;
            }
            if state.paused {
                state = self.changed.wait(state).unwrap_or_else(|e| e.into_inner());
                continue;
            }
            let now = Instant::now();
            match plan(state.anchor, now, pts_us, self.max_lateness) {
                Plan::Anchor => {
                    state.anchor = Some((now, pts_us));
                    return Due::Show;
                }
                Plan::Wait(wait) => {
                    state = self
                        .changed
                        .wait_timeout(state, wait)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
                Plan::Show => return Due::Show,
                Plan::Late => return Due::Late,
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn plan(anchor: Option<(Instant, u64)>, now: Instant, pts_us: u64, max_lateness: Duration) -> Plan {
    let Some((wall, base)) = anchor else {
        return Plan::Anchor;
    };
    if pts_us < base {
        return Plan::Anchor;
    }
    let due = wall + Duration::from_micros(pts_us - base);
    if due > now {
        return Plan::Wait(due - now);
    }
    match now - due {
        late if late > RESYNC => Plan::Anchor,
        late if late > max_lateness => Plan::Late,
        _ => Plan::Show,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;

    use super::*;

    const LATENESS: Duration = Duration::from_millis(50);

    fn anchored(now: Instant) -> Option<(Instant, u64)> {
        Some((now, 1_000_000))
    }

    #[test]
    fn the_first_frame_sets_the_timeline() {
        assert_eq!(plan(None, Instant::now(), 5, LATENESS), Plan::Anchor);
    }

    #[test]
    fn a_frame_ahead_of_the_timeline_waits_for_its_turn() {
        let start = Instant::now();
        assert_eq!(
            plan(anchored(start), start, 1_040_000, LATENESS),
            Plan::Wait(Duration::from_millis(40))
        );
    }

    #[test]
    fn a_frame_slightly_behind_is_still_shown() {
        let start = Instant::now();
        let now = start + Duration::from_millis(60);
        assert_eq!(plan(anchored(start), now, 1_020_000, LATENESS), Plan::Show);
    }

    #[test]
    fn a_frame_too_far_behind_is_late() {
        let start = Instant::now();
        let now = start + Duration::from_millis(200);
        assert_eq!(plan(anchored(start), now, 1_020_000, LATENESS), Plan::Late);
    }

    #[test]
    fn a_stall_longer_than_a_second_resynchronises() {
        let start = Instant::now();
        let now = start + Duration::from_secs(3);
        assert_eq!(
            plan(anchored(start), now, 1_020_000, LATENESS),
            Plan::Anchor
        );
    }

    #[test]
    fn a_loop_restart_resynchronises() {
        let start = Instant::now();
        assert_eq!(plan(anchored(start), start, 0, LATENESS), Plan::Anchor);
    }

    #[test]
    fn a_paused_clock_holds_the_frame_until_it_resumes() {
        let clock = Arc::new(Clock::new(LATENESS));
        clock.pause();
        let (sent, received) = mpsc::channel();
        let waiter = Arc::clone(&clock);
        std::thread::spawn(move || sent.send(waiter.wait_for(0)));
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        clock.resume();
        assert_eq!(received.recv_timeout(Duration::from_secs(5)), Ok(Due::Show));
    }

    #[test]
    fn stopping_releases_a_paused_waiter() {
        let clock = Arc::new(Clock::new(LATENESS));
        clock.pause();
        let (sent, received) = mpsc::channel();
        let waiter = Arc::clone(&clock);
        std::thread::spawn(move || sent.send(waiter.wait_for(0)));
        clock.stop();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)),
            Ok(Due::Stopped)
        );
    }
}
