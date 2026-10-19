//! D11 native ownership. Blocks retain numeric identifiers, never editor/native references.
use crate::lifecycle::{Callback, End, Event, Lease, Progress};
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue, NSRunLoop, NSRunLoopCommonModes, NSString, NSTimer,
};
use objc2_ui_kit::{
    UIApplication, UIApplicationDidBecomeActiveNotification, UIApplicationDidEnterBackgroundNotification, UIApplicationWillResignActiveNotification,
    UIApplicationWillTerminateNotification, UIBackgroundTaskInvalid,
};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static NEXT: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static OWNERS: RefCell<BTreeMap<u64,Rc<State>>> = const {RefCell::new(BTreeMap::new())};
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Message {
    Suspend,
    Resume,
    Terminate,
    Poll(u64),
    Expire(u64),
    Finished(u64, End),
    Drain,
}
struct State {
    id: u64,
    mtm: MainThreadMarker,
    started: Instant,
    lease: RefCell<Lease>,
    timer: RefCell<Option<Retained<NSTimer>>>,
    observers: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
    queue: RefCell<VecDeque<Message>>,
    busy: Cell<bool>,
    closed: Cell<bool>,
    callback: Callback,
}
/// Main-thread owner of notification observers, a timer and a bounded background task.
pub struct LifecycleHost {
    id: u64,
    _mtm: MainThreadMarker,
}
// The native block APIs require sendable captures even when their queue is main.
fn sendable<F: Send + Sync>(f: F) -> F {
    f
}
fn owner(id: u64) -> Option<Rc<State>> {
    OWNERS.with(|owners| owners.try_borrow().ok()?.get(&id).cloned())
}
fn on_main(id: u64, message: Message) {
    let block = block2::RcBlock::new(sendable(move || deliver(id, message.clone())));
    // SAFETY: The block owns only Send+Sync value messages and an integer ID.
    // Foundation copies it and runs it on its main operation queue.
    unsafe { NSOperationQueue::mainQueue().addOperationWithBlock(&block) };
}
fn deliver(id: u64, message: Message) {
    if MainThreadMarker::new().is_none() {
        on_main(id, message);
        return;
    }
    // No Rust unwind may return through the Objective-C block invocation.
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(id, message))).is_err()
        && let Some(state) = owner(id)
    {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(cycle) = state.cycle() {
                state.stop(cycle, End::Failed("native lifecycle callback panicked".into()));
            }
            state.busy.set(false);
        }));
    }
}
fn dispatch(id: u64, message: Message) {
    let Some(state) = owner(id).filter(|s| !s.closed.get()) else { return };
    // Expiration must end the OS lease before returning, even if another app
    // callback is active. Its notice is queued for delivery after that callback.
    if let Message::Expire(cycle) = message {
        state.stop(cycle, End::Expired);
    } else if message != Message::Drain
        && !state.enqueue(message)
        && let Some(cycle) = state.cycle()
    {
        state.stop(cycle, End::Failed("lifecycle event queue overflow".into()));
    }
    if state.busy.replace(true) {
        return;
    }
    for _ in 0..32 {
        if state.closed.get() {
            break;
        }
        let message = state.queue.try_borrow_mut().ok().and_then(|mut q| q.pop_front());
        let Some(message) = message else { break };
        match message {
            Message::Suspend => state.begin(),
            Message::Resume => {
                if let Some(cycle) = state.cycle() {
                    state.stop(cycle, End::Resumed);
                }
                state.call(Event::Resumed);
            }
            Message::Terminate => {
                if let Some(cycle) = state.cycle() {
                    state.stop(cycle, End::Closed);
                }
                state.call(Event::Terminating);
            }
            Message::Poll(cycle) => state.poll(cycle),
            Message::Finished(cycle, outcome) => {
                state.call(Event::Finished { cycle, outcome });
            }
            Message::Expire(_) | Message::Drain => {}
        }
    }
    state.busy.set(false);
    if !state.closed.get() && state.queue.try_borrow().is_ok_and(|q| !q.is_empty()) {
        on_main(id, Message::Drain);
    }
}
impl State {
    fn cycle(&self) -> Option<u64> {
        self.lease.try_borrow().ok()?.cycle()
    }
    fn enqueue(&self, message: Message) -> bool {
        let Ok(mut queue) = self.queue.try_borrow_mut() else { return false };
        if matches!(message, Message::Poll(_)) && queue.contains(&message) {
            return true;
        }
        if queue.len() >= 64 {
            return false;
        }
        queue.push_back(message);
        true
    }
    fn call(&self, event: Event) -> Progress {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.callback)(event)))
            .unwrap_or_else(|_| Progress::Failed("application lifecycle callback panicked".into()))
    }
    fn end_task(&self, task: usize) {
        // SAFETY: UIKit exports this immutable invalid-identifier constant.
        if task != unsafe { UIBackgroundTaskInvalid } {
            UIApplication::sharedApplication(self.mtm).endBackgroundTask(task);
        }
    }
    fn stop(&self, cycle: u64, outcome: End) {
        let attempt = self.lease.try_borrow_mut().ok().and_then(|mut lease| lease.finish(cycle));
        let Some(attempt) = attempt else { return };
        // Take every owner before calling native code, which can reenter us.
        let timer = self.timer.try_borrow_mut().ok().and_then(|mut timer| timer.take());
        if let Ok(mut queue) = self.queue.try_borrow_mut() {
            if queue.len() >= 64 {
                queue.clear();
            }
            queue.push_back(Message::Finished(cycle, outcome));
        }
        if let Some(timer) = timer {
            timer.invalidate();
        }
        if let Some(task) = attempt.task {
            self.end_task(task);
        }
    }
    fn begin(&self) {
        let result = self
            .lease
            .try_borrow_mut()
            .map_err(|_| "lifecycle lease is already borrowed".to_string())
            .and_then(|mut lease| lease.begin(self.started.elapsed()));
        let cycle = match result {
            Ok(Some(cycle)) => cycle,
            Ok(None) => return,
            Err(error) => {
                self.call(Event::Finished { cycle: 0, outcome: End::Failed(error) });
                return;
            }
        };
        let (id, task_cycle) = (self.id, cycle);
        let expiration = block2::RcBlock::new(sendable(move || deliver(id, Message::Expire(task_cycle))));
        let task = UIApplication::sharedApplication(self.mtm)
            .beginBackgroundTaskWithName_expirationHandler(Some(&NSString::from_str("FigCraft recovery")), Some(&expiration));
        // SAFETY: UIKit exports this immutable invalid-identifier constant.
        if task == unsafe { UIBackgroundTaskInvalid } {
            if self.cycle() == Some(cycle) {
                self.call(Event::Checkpoint { cycle });
                self.stop(cycle, End::Failed("background checkpoint execution was denied".into()));
            }
            return;
        }
        let orphan = self.lease.try_borrow_mut().map_or(Some(task), |mut lease| lease.attach(cycle, task));
        if let Some(task) = orphan {
            self.end_task(task);
            return;
        }
        if self.closed.get() || self.cycle() != Some(cycle) {
            return;
        }
        let id = self.id;
        let tick = block2::RcBlock::new(sendable(move |_timer: NonNull<NSTimer>| deliver(id, Message::Poll(cycle))));
        // SAFETY: The block's captures are Send+Sync integer identifiers. The
        // timer is retained below and is scheduled only on the main run loop.
        let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(0.05, true, &tick) };
        if self.closed.get() || self.cycle() != Some(cycle) {
            timer.invalidate();
            return;
        }
        if let Ok(mut slot) = self.timer.try_borrow_mut() {
            *slot = Some(timer.clone());
        } else {
            self.stop(cycle, End::Failed("lifecycle timer is already borrowed".into()));
            return;
        }
        // SAFETY: Both run loop and timer are used on the checked main thread;
        // the immutable Foundation common-mode name is valid for this process.
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
        if self.closed.get() || self.cycle() != Some(cycle) {
            timer.invalidate();
            return;
        }
        self.poll(cycle);
    }
    fn poll(&self, cycle: u64) {
        if self.cycle() != Some(cycle) {
            return;
        }
        if self.lease.try_borrow().is_ok_and(|l| l.expired(self.started.elapsed())) {
            self.stop(cycle, End::Deadline);
            return;
        }
        let result = self.call(Event::Checkpoint { cycle });
        if self.closed.get() || self.cycle() != Some(cycle) {
            return;
        }
        if self.lease.try_borrow().is_ok_and(|l| l.expired(self.started.elapsed())) {
            self.stop(cycle, End::Deadline);
            return;
        }
        match result {
            Progress::Pending => {}
            Progress::Complete => self.stop(cycle, End::Complete),
            Progress::Failed(error) => self.stop(cycle, End::Failed(error)),
        }
    }
    fn close(&self) {
        self.closed.set(true);
        if let Some(cycle) = self.cycle() {
            self.stop(cycle, End::Closed);
        }
        let timer = self.timer.try_borrow_mut().ok().and_then(|mut t| t.take());
        if let Some(timer) = timer {
            timer.invalidate();
        }
        let tokens = self.observers.try_borrow_mut().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
        let center = NSNotificationCenter::defaultCenter();
        for token in tokens {
            // SAFETY: This is the exact retained observer token from this center.
            // No registry owner remains, so queued callbacks cannot access it.
            let observer: &ProtocolObject<dyn NSObjectProtocol> = &token;
            unsafe { center.removeObserver(observer.as_ref()) };
        }
    }
}
impl LifecycleHost {
    pub fn new(callback: Callback) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("lifecycle host requires the main thread")?;
        let id = NEXT.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map_err(|_| "lifecycle owner IDs exhausted")?;
        let state = Rc::new(State {
            id,
            mtm,
            started: Instant::now(),
            lease: RefCell::new(Lease::default()),
            timer: RefCell::new(None),
            observers: RefCell::new(Vec::new()),
            queue: RefCell::new(VecDeque::new()),
            busy: Cell::new(false),
            closed: Cell::new(false),
            callback,
        });
        OWNERS.with(|owners| {
            owners.try_borrow_mut().map(|mut owners| owners.insert(id, state.clone())).map_err(|_| "lifecycle registry is already borrowed")
        })?;
        let host = Self { id, _mtm: mtm };
        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        // SAFETY: These are immutable UIKit notification-name constants.
        let notifications = unsafe {
            [
                (UIApplicationWillResignActiveNotification, Message::Suspend),
                (UIApplicationDidEnterBackgroundNotification, Message::Suspend),
                (UIApplicationDidBecomeActiveNotification, Message::Resume),
                (UIApplicationWillTerminateNotification, Message::Terminate),
            ]
        };
        for (name, message) in notifications {
            let block = block2::RcBlock::new(sendable(move |_notification: NonNull<NSNotification>| deliver(id, message.clone())));
            // SAFETY: No object filter or payload cast is used. The exact name
            // and main queue are retained by Foundation. Captures are Send+Sync.
            let token = unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, Some(&queue), &block) };
            state.observers.try_borrow_mut().map_err(|_| "lifecycle observers are already borrowed")?.push(token);
        }
        Ok(host)
    }
}
impl Drop for LifecycleHost {
    fn drop(&mut self) {
        let state = OWNERS.with(|owners| owners.try_borrow_mut().ok().and_then(|mut owners| owners.remove(&self.id)));
        if let Some(state) = state {
            state.close();
        }
    }
}
