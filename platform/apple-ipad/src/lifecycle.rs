//! Safe lifecycle messages and checked ownership of a bounded checkpoint attempt.
use std::time::Duration;

pub const FLUSH_BUDGET: Duration = Duration::from_secs(5);
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Pending,
    Complete,
    Failed(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    Complete,
    Failed(String),
    Deadline,
    Expired,
    Resumed,
    Closed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Repeated with the same cycle while capture or publication is pending.
    Checkpoint {
        cycle: u64,
    },
    Finished {
        cycle: u64,
        outcome: End,
    },
    Resumed,
    Terminating,
}
pub type Callback = std::sync::Arc<dyn Fn(Event) -> Progress + Send + Sync>;

#[cfg(any(target_os = "ios", test))]
#[derive(Debug)]
pub(crate) struct Attempt {
    pub cycle: u64,
    pub deadline: Duration,
    pub task: Option<usize>,
}
#[cfg(any(target_os = "ios", test))]
#[derive(Default)]
pub(crate) struct Lease {
    next: u64,
    attempt: Option<Attempt>,
}
#[cfg(any(target_os = "ios", test))]
impl Lease {
    pub fn begin(&mut self, now: Duration) -> Result<Option<u64>, String> {
        if self.attempt.is_some() {
            return Ok(None);
        }
        let cycle = self.next.checked_add(1).ok_or("lifecycle cycle IDs exhausted")?;
        let deadline = now.checked_add(FLUSH_BUDGET).ok_or("lifecycle deadline overflow")?;
        self.next = cycle;
        self.attempt = Some(Attempt { cycle, deadline, task: None });
        Ok(Some(cycle))
    }
    pub fn cycle(&self) -> Option<u64> {
        self.attempt.as_ref().map(|a| a.cycle)
    }
    pub fn expired(&self, now: Duration) -> bool {
        self.attempt.as_ref().is_some_and(|a| now >= a.deadline)
    }
    /// Return an orphan identifier to end if its attempt expired during creation.
    pub fn attach(&mut self, cycle: u64, task: usize) -> Option<usize> {
        if let Some(a) = &mut self.attempt
            && a.cycle == cycle
            && a.task.is_none()
        {
            a.task = Some(task);
            None
        } else {
            Some(task)
        }
    }
    pub fn finish(&mut self, cycle: u64) -> Option<Attempt> {
        if self.cycle() == Some(cycle) { self.attempt.take() } else { None }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipad_deadline_is_not_extended_by_a_second_notification() {
        let mut lease = Lease::default();
        assert_eq!(lease.begin(Duration::ZERO).unwrap(), Some(1));
        assert_eq!(lease.begin(Duration::from_secs(4)).unwrap(), None);
        assert!(!lease.expired(Duration::from_millis(4999)));
        assert!(lease.expired(FLUSH_BUDGET));
    }
    #[test]
    fn ipad_expiration_before_task_attachment_returns_the_orphan() {
        let mut lease = Lease::default();
        let cycle = lease.begin(Duration::ZERO).unwrap().unwrap();
        assert!(lease.finish(cycle).unwrap().task.is_none());
        assert_eq!(lease.attach(cycle, 41), Some(41));
        assert!(lease.finish(cycle).is_none());
    }
    #[test]
    fn ipad_duplicate_and_stale_terminal_events_cannot_end_a_new_task() {
        let mut lease = Lease::default();
        let first = lease.begin(Duration::ZERO).unwrap().unwrap();
        assert_eq!(lease.attach(first, 11), None);
        assert_eq!(lease.finish(first).unwrap().task, Some(11));
        let second = lease.begin(Duration::from_secs(1)).unwrap().unwrap();
        assert_ne!(first, second);
        assert_eq!(lease.attach(second, 22), None);
        assert!(lease.finish(first).is_none());
        assert_eq!(lease.finish(second).unwrap().task, Some(22));
        assert!(lease.finish(second).is_none());
    }
    #[test]
    fn ipad_duplicate_attachment_keeps_the_original_owner() {
        let mut lease = Lease::default();
        let cycle = lease.begin(Duration::ZERO).unwrap().unwrap();
        assert_eq!(lease.attach(cycle, 11), None);
        assert_eq!(lease.attach(cycle, 22), Some(22));
        assert_eq!(lease.finish(cycle).unwrap().task, Some(11));
    }
    #[test]
    fn ipad_cycle_ids_and_deadlines_fail_without_wrapping() {
        let mut lease = Lease { next: u64::MAX, attempt: None };
        assert!(lease.begin(Duration::ZERO).is_err());
        assert!(lease.cycle().is_none());
        let mut lease = Lease::default();
        assert!(lease.begin(Duration::MAX).is_err());
        assert!(lease.cycle().is_none());
    }
}
