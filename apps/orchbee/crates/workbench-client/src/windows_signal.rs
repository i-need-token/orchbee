//! One process-wide console interrupt dispatcher and one scoped owner. The
//! final arbitration CAS transfers the owner into the output finalizer when
//! cancellation wins; late signals otherwise keep the default exit behavior.
use std::{
    io,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};
const ARMED: u8 = 0;
const SIGNALLED: u8 = 1;
const INACTIVE: u8 = 2;
static STATE: AtomicU8 = AtomicU8::new(INACTIVE);
static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();

pub struct ControlInterrupt;
impl ControlInterrupt {
    pub fn arm() -> io::Result<Self> {
        let installed = INSTALLED.get_or_init(|| {
            ctrlc::set_handler(|| {
                loop {
                    match STATE.load(Ordering::Acquire) {
                        INACTIVE => std::process::exit(130),
                        SIGNALLED => break,
                        _ => {
                            if STATE
                                .compare_exchange(
                                    ARMED,
                                    SIGNALLED,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                )
                                .is_ok()
                            {
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(|error| error.to_string())
        });
        if let Err(error) = installed {
            return Err(io::Error::other(error.clone()));
        }
        STATE
            .compare_exchange(INACTIVE, ARMED, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| io::Error::other("console interrupt already owned"))?;
        Ok(Self)
    }
    pub fn requested(&self) -> bool {
        STATE.load(Ordering::Acquire) == SIGNALLED
    }
    pub fn wake(&self) -> orchbee_local_ipc::LocalIo<'_> {
        orchbee_local_ipc::LocalIo::Wake(&STATE, SIGNALLED)
    }
    pub fn finish_clear(&self) -> bool {
        STATE
            .compare_exchange(ARMED, INACTIVE, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}
impl Drop for ControlInterrupt {
    fn drop(&mut self) {
        STATE.store(INACTIVE, Ordering::Release);
    }
}
