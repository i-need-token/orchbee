use super::*;
use rustix::process::test_kill_process;
use std::{
    io::{BufRead, BufReader, PipeWriter, Write},
    process::Stdio,
};

/// A root shell that starts a descendant in its group, announces the descendant and exits 7 after
/// one input line. With `ignore_term` both ignore `SIGTERM`.
fn tree(ignore_term: bool) -> (CommandChild, PipeWriter, Pid) {
    let (input, input_writer) = std::io::pipe().unwrap();
    let (output, output_writer) = std::io::pipe().unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(format!(
            "{} sleep 30 & echo $!; read line; exit 7",
            if ignore_term { "trap '' TERM;" } else { "" }
        ))
        .stdin(input)
        .stdout(output_writer)
        .stderr(Stdio::null());
    let child = CommandChild::spawn(command).unwrap();
    let mut line = String::new();
    BufReader::new(output).read_line(&mut line).unwrap();
    let descendant = Pid::from_raw(line.trim().parse().unwrap()).unwrap();
    test_kill_process(descendant).expect("descendant runs before settlement");
    (child, input_writer, descendant)
}

fn gone(pid: Pid) -> bool {
    test_kill_process(pid) == Err(Errno::SRCH)
}

#[test]
fn root_exit_and_cancellation_end_every_group_member_before_the_result() {
    for (root_exits, ignore_term) in [(true, false), (false, false), (false, true)] {
        let (mut child, mut input, descendant) = tree(ignore_term);
        assert!(child.try_wait().unwrap().is_none());
        let started = Instant::now();
        let status = if root_exits {
            input.write_all(b"exit\n").unwrap();
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                assert!(started.elapsed() < SETTLE_BUDGET, "root failed to exit");
                thread::sleep(POLL_SLICE);
            }
        } else {
            child.settle().unwrap()
        };
        if root_exits {
            assert_eq!(status.code(), Some(7));
        }
        if ignore_term {
            assert!(
                started.elapsed() >= TERM_GRACE,
                "an ignored TERM ends the grace early"
            );
        }
        assert!(started.elapsed() < SETTLE_BUDGET);
        // The descendant shares the root's group; settlement waits until it has left it.
        assert!(gone(descendant));
        // A retained result is stable and never signals the released group again.
        assert_eq!(child.try_wait().unwrap(), Some(status));
        assert_eq!(child.settle().unwrap(), status);
    }
}

#[test]
fn dropped_owner_kills_its_group_and_leaves_no_zombie() {
    let (child, _input, descendant) = tree(false);
    // A zombie still answers a null signal, so absence proves the root was reaped.
    let root = Pid::from_raw(child.id() as i32).unwrap();
    drop(child);
    let deadline = Instant::now() + SETTLE_BUDGET;
    while !(gone(root) && gone(descendant)) {
        assert!(Instant::now() < deadline, "dropped group survived");
        thread::sleep(POLL_SLICE);
    }
}

#[test]
fn missing_program_reports_that_no_process_was_created() {
    let missing = std::env::temp_dir().join(format!("orchbee-absent-git-{}", std::process::id()));
    assert!(!missing.exists());
    let Err(failure) = CommandChild::spawn(Command::new(missing)) else {
        panic!("missing program spawned")
    };
    assert!(!failure.created);
    assert_eq!(failure.error.kind(), io::ErrorKind::NotFound);
}

/// A command started for a thread that blocks `SIGINT` and `SIGTERM`, as the CLI's signal owner
/// does, still receives them: a shell that signals itself ends by `SIGTERM` instead of running on.
#[test]
fn an_unmasked_command_receives_the_interrupts_its_spawning_thread_blocks() {
    use nix::sys::signal::{SigSet, SigmaskHow, Signal};
    let mut interrupts = SigSet::empty();
    interrupts.add(Signal::SIGINT);
    interrupts.add(Signal::SIGTERM);
    let previous = interrupts.thread_swap_mask(SigmaskHow::SIG_BLOCK).unwrap();
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "kill -TERM $$; echo survived"]);
    let result =
        run_unmasked_unix_command(command, SpawnEnvironment::Inherited, 1024, None, || {
            Ok(false)
        });
    previous.thread_set_mask().unwrap();
    assert!(
        result.stdout.is_empty(),
        "{:?}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert_eq!(result.exit_code, None);
}

/// A cleared launch passes exactly the variables the command sets; an inherited one passes this
/// process's environment with the command's changes.
#[test]
fn an_unmasked_command_receives_exactly_the_environment_it_states() {
    let mut command = Command::new("/usr/bin/env");
    command.env_clear().env("ORCHBEE_SPAWN_PROBE", "1");
    let cleared =
        run_unmasked_unix_command(command, SpawnEnvironment::Cleared, 64 * 1024, None, || {
            Ok(false)
        });
    assert_eq!(cleared.stdout, b"ORCHBEE_SPAWN_PROBE=1\n");

    let mut command = Command::new("/usr/bin/env");
    command.env("ORCHBEE_SPAWN_PROBE", "2").env_remove("PATH");
    let inherited = run_unmasked_unix_command(
        command,
        SpawnEnvironment::Inherited,
        64 * 1024,
        None,
        || Ok(false),
    );
    let text = String::from_utf8(inherited.stdout).unwrap();
    assert!(
        text.lines().any(|line| line == "ORCHBEE_SPAWN_PROBE=2"),
        "{text}"
    );
    assert!(
        !text.lines().any(|line| line.starts_with("PATH=")),
        "{text}"
    );
    assert_eq!(
        text.lines().any(|line| line.starts_with("HOME=")),
        std::env::var_os("HOME").is_some(),
        "{text}"
    );
}
