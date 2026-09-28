//! Real Windows script and native argument boundaries, with an owned Job.

use super::*;
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Child(Box<dyn ChildWrapper>);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
        let _ = self.0.wait();
    }
}

#[test]
fn actual_windows_programs_receive_literal_arguments() {
    let root = std::env::temp_dir().join(format!(
        "orchbee-start-argv-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    let output = fixture.0.join("argv.json");
    let script = fixture.0.join("dump.cjs");
    fs::write(&script, "require('fs').writeFileSync(process.env.ORCHBEE_START_ARGV_FILE,JSON.stringify(process.argv.slice(2)))").unwrap();
    let ps = fixture.0.join("probe.ps1");
    fs::write(&ps, "[IO.File]::WriteAllText($env:ORCHBEE_START_ARGV_FILE,(ConvertTo-Json -InputObject @($args) -Compress),[Text.UTF8Encoding]::new($false))").unwrap();
    let batch = fixture.0.join("probe.cmd");
    fs::write(
        &batch,
        format!("@echo off\r\nnode.exe \"{}\" %*\r\n", script.display()),
    )
    .unwrap();
    for name in ["agent.cmd", "qoder.cmd"] {
        fs::copy(&batch, fixture.0.join(name)).unwrap();
    }
    // A regression must fail locally, never launch a real historical install.
    for name in ["cursor-agent.cmd", "qodercli.cmd"] {
        fs::write(fixture.0.join(name), b"@exit /b 23\r\n").unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(fixture.0.clone())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let values: Vec<String> = [
        "",
        "a b",
        "中文😀",
        "a'b",
        "a\"b",
        "trail\\",
        "%PATH%",
        "!literal!",
        "a&b",
        "$x;()`x",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for (program, args) in [
        (
            "node.exe".to_owned(),
            [vec![script.to_str().unwrap().to_owned()], values.clone()].concat(),
        ),
        (ps.to_str().unwrap().to_owned(), values.clone()),
        (batch.to_str().unwrap().to_owned(), values.clone()),
        ("cursor".to_owned(), values.clone()),
        ("qodercli".to_owned(), values.clone()),
    ] {
        let text = if matches!(program.as_str(), "cursor" | "qodercli") {
            launch(&program, &args).unwrap()
        } else {
            launch_program(&program, &args).unwrap()
        };
        // Server retains PowerShell after provider exit. Helpers and error policy
        // must not leak into that interactive shell's parent scope.
        let encoded = text.split_whitespace().last().unwrap();
        let script = format!(
            "$before=$ErrorActionPreference;& ([ScriptBlock]::Create([Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('{encoded}'))));if($ErrorActionPreference -ne $before -or (Test-Path Function:q)){{throw 'launch scope leaked'}}"
        );
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoLogo", "-NoProfile", "-Command", &script])
            .env("ORCHBEE_START_ARGV_FILE", &output)
            .env("PATH", &path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut wrapped = CommandWrap::from(command);
        wrapped
            .wrap(CreationFlags(
                windows::Win32::System::Threading::CREATE_NO_WINDOW,
            ))
            .wrap(JobObject);
        let mut child = Child(wrapped.spawn().unwrap());
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success(), "{program}: {status}");
                break;
            }
            assert!(Instant::now() < end, "{program}: timeout");
            thread::sleep(Duration::from_millis(10));
        }
        drop(child);
        let observed: Vec<String> =
            serde_json::from_slice(&fs::read(&output).expect("argument witness")).unwrap();
        assert_eq!(observed, values, "{program}");
        fs::remove_file(&output).unwrap();
    }
}
