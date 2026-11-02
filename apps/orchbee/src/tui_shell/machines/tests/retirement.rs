//! Clients the picker stops presenting stay owned until process exit observes their threads.
use super::super::*;
use super::{dimensions, profile, setup};
use crate::{
    machines::{Draft, ProfileId},
    terminal_input::UnavailableTerminalInputPort,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

/// A surface whose background thread works until asked to stop, then needs `linger` to end.
/// Dropping it, like dropping a Client, only requests stop.
struct Busy {
    stop: Arc<AtomicBool>,
    task: BackgroundThread,
}

impl Busy {
    /// The surface, and whether its thread has ended.
    fn start(linger: Duration) -> (Box<dyn TerminalSurface>, Arc<AtomicBool>) {
        let stop = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(AtomicBool::new(false));
        let (worker_stop, worker_ended) = (stop.clone(), ended.clone());
        let task = BackgroundThread::spawn("orchbee-test-busy-surface", move || {
            while !worker_stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
            thread::sleep(linger);
            worker_ended.store(true, Ordering::SeqCst);
        })
        .unwrap();
        (Box::new(Self { stop, task }), ended)
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl crate::terminal_input::TerminalInputPort for Busy {
    fn forward(
        &mut self,
        _bytes: &[u8],
    ) -> Result<(), orchbee_workbench_client::test_support::terminal_input::TerminalInputUnavailable>
    {
        Ok(())
    }
}

impl TerminalSurface for Busy {
    fn request_stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
    fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.request_stop();
        self.task.join_before(deadline)
    }
}

fn remote(n: u8) -> Profile {
    Profile {
        id: ProfileId::parse(&format!("{n:032x}")).unwrap(),
        draft: Draft::parse("remote", &format!("example{n}.invalid"), "default").unwrap(),
        enabled: true,
    }
}

fn cached(profile: Option<Profile>, surface: Box<dyn TerminalSurface>) -> Cached {
    Cached {
        profile,
        surface,
        view: ClientView::new(100),
    }
}

fn idle() -> Box<dyn TerminalSurface> {
    Box::new(UnavailableTerminalInputPort)
}

/// How the picker stops presenting a Client.
#[derive(Clone, Copy, Debug)]
enum Removal {
    /// Its profile left the catalog.
    ProfileRemoved,
    /// A fresh connection to its machine superseded the cached surface.
    Superseded,
    /// A fresh connection to the active machine replaced it.
    Replaced,
    /// A fifth inactive remote surface evicted it from the cache.
    Evicted,
}

/// Puts `busy` in a Manager while it works, then removes it as `removal` says.
fn remove(
    removal: Removal,
    busy: Box<dyn TerminalSurface>,
) -> (Manager, ClientView, SurfaceSlot<'static>) {
    let (mut manager, mut view, mut surface) = setup();
    let (next, keep_old) = match removal {
        Removal::ProfileRemoved => {
            manager.cached.push(cached(Some(profile()), busy));
            manager.refresh_catalog(
                Ok(crate::machines::Catalog {
                    revision: 2,
                    profiles: Vec::new(),
                }),
                &mut surface,
                &mut view,
                dimensions(),
                &mut |_| {},
            );
            return (manager, view, surface);
        }
        Removal::Superseded => {
            manager.cached.push(cached(Some(profile()), busy));
            (cached(Some(profile()), idle()), true)
        }
        Removal::Replaced => {
            manager.current = Some(profile());
            drop(surface.replace(busy));
            (cached(Some(profile()), idle()), false)
        }
        Removal::Evicted => {
            manager.cached.push(cached(Some(remote(1)), busy));
            for n in 2..=4 {
                manager.cached.push(cached(Some(remote(n)), idle()));
            }
            // Returning to Local caches the active remote as the fifth.
            manager.current = Some(remote(5));
            (cached(None, idle()), true)
        }
    };
    manager.install(
        next,
        &mut surface,
        &mut view,
        dimensions(),
        keep_old,
        &mut |_| {},
    );
    (manager, view, surface)
}

#[test]
fn clients_replaced_or_evicted_while_working_finish_inside_exit_cleanup() {
    for removal in [
        Removal::ProfileRemoved,
        Removal::Superseded,
        Removal::Replaced,
        Removal::Evicted,
    ] {
        let (busy, ended) = Busy::start(Duration::from_millis(300));
        let (mut manager, _view, _surface) = remove(removal, busy);
        assert!(!ended.load(Ordering::SeqCst), "{removal:?}");
        // The program exits right away; cleanup observes the removed Client's thread to its end.
        manager
            .stop_before(Instant::now() + Duration::from_secs(5))
            .unwrap_or_else(|error| panic!("{removal:?}: {error}"));
        assert!(ended.load(Ordering::SeqCst), "{removal:?}");
    }
}

#[test]
fn a_removed_client_still_running_at_the_exit_deadline_is_reported_and_stays_owned() {
    let (busy, ended) = Busy::start(Duration::from_millis(500));
    let (mut manager, mut view, mut surface) = remove(Removal::Evicted, busy);
    let error = manager
        .stop_before(Instant::now() + Duration::from_millis(20))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(!ended.load(Ordering::SeqCst));
    // Not detached: a later observation still sees the same thread end, and the next tick
    // releases the Client.
    manager
        .stop_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert!(ended.load(Ordering::SeqCst));
    manager.tick(
        &mut surface,
        &mut view,
        dimensions(),
        Instant::now(),
        &mut |_| {},
    );
    assert!(manager.retired.is_empty());
}
