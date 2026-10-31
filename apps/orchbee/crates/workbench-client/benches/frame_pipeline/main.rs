//! Stage benchmarks for the native TUI Client's frame pipeline, reached only through stable
//! public entries. See the crate README's benchmark section for commands and measurement scope.

mod decode_apply;
mod harness;
mod obtf;

fn main() {
    let mut harness = match harness::Harness::from_args() {
        Ok(harness) => harness,
        Err(message) => {
            eprintln!("frame_pipeline: {message}");
            std::process::exit(2);
        }
    };
    harness.start();
    decode_apply::register(&mut harness);
    harness.finish();
}
