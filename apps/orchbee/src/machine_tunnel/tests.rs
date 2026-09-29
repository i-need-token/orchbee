use super::*;
use std::{
    io::{Read, Write},
    process::Command,
};
fn echo() -> io::Result<Command> {
    let mut command = Command::new(
        PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
            .join("WindowsPowerShell/v1.0/powershell.exe"),
    );
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Console]::OpenStandardInput().CopyTo([Console]::OpenStandardOutput())",
    ]);
    Ok(command)
}
#[test]
fn binary_carrier_handles_backpressure_and_last_owner_retires_lanes() {
    let (endpoint, guard) = start(echo).unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut stream = LocalStream::connect_before(&endpoint, deadline).unwrap();
    stream.set_nonblocking(true).unwrap();
    let expected: Vec<u8> = (0..1024 * 1024).map(|n| (n % 256) as u8).collect();
    let mut sent = 0;
    let mut got = Vec::new();
    let mut read = [0; 8192];
    let mut backpressure = false;
    while got.len() < expected.len() {
        assert!(
            Instant::now() < deadline,
            "binary pump stalled: sent {sent}, received {}",
            got.len()
        );
        if sent < expected.len() {
            match stream.write(&expected[sent..]) {
                Ok(n) => sent += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => backpressure = true,
                result => panic!("binary write: {result:?}"),
            }
        }
        match stream.read(&mut read) {
            Ok(0) => panic!("unexpected EOF"),
            Ok(n) => got.extend_from_slice(&read[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            result => panic!("binary read: {result:?}"),
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(got, expected);
    assert!(backpressure, "exercise writes larger than pipe capacity");
    let retained = guard.clone();
    drop(guard);
    let mut second = LocalStream::connect_before(&endpoint, deadline).unwrap();
    second
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    second
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    second.write_all(b"secondary lane").unwrap();
    let mut secondary = [0; 14];
    second.read_exact(&mut secondary).unwrap();
    assert_eq!(&secondary, b"secondary lane");
    drop(second);
    drop(retained);
    assert!(
        LocalStream::connect_before(&endpoint, Instant::now() + Duration::from_millis(100))
            .is_err()
    );
    assert!(matches!(stream.read(&mut read), Ok(0) | Err(_)));
}
