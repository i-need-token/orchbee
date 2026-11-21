//! Boundary evidence for the reviewed host statistics adapter: each number agrees with what the
//! system's own tools print, and the tick counters move.
use super::{cpu_load, memory_size, page_size, reclaimable_pages};
use std::{process::Command, time::Instant};

fn sysctl(name: &str) -> u64 {
    let output = Command::new("/usr/sbin/sysctl")
        .args(["-n", name])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn the_sysctl_numbers_are_what_sysctl_prints() {
    assert_eq!(memory_size(), Some(sysctl("hw.memsize")));
    assert_eq!(page_size(), Some(sysctl("vm.pagesize")));
}

#[test]
fn the_reclaimable_pages_fit_the_physical_memory() {
    let pages = reclaimable_pages().unwrap();
    let bytes = pages.checked_mul(page_size().unwrap()).unwrap();
    assert!(bytes <= memory_size().unwrap(), "{bytes} reclaimable bytes");
}

#[test]
fn the_tick_counters_move_while_a_processor_works() {
    let earlier = cpu_load().unwrap();
    let start = Instant::now();
    let mut spin = 0_u64;
    while start.elapsed().as_millis() < 200 {
        spin = std::hint::black_box(spin.wrapping_add(1));
    }
    let later = cpu_load().unwrap();
    let total = |load: super::CpuLoad| {
        load.user
            .wrapping_add(load.system)
            .wrapping_add(load.idle)
            .wrapping_add(load.nice)
    };
    assert!(total(later).wrapping_sub(total(earlier)) > 0);
}
