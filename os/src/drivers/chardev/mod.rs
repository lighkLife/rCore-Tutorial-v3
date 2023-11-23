use alloc::sync::Arc;

use lazy_static::*;

pub use async_ns16550a::AsyncNS16550a;
pub use executor::thread::{Executor, WorkMarker};

use crate::board::AsyncCharDeviceImpl;

mod async_ns16550a;
mod ns16550a;
mod executor;

pub trait CharDevice {
    fn init(&self);
    fn read(&self) -> u8;
    fn write(&self, ch: u8);
    fn handle_irq(&self);
}

lazy_static! {
    pub static ref ASYNC_UART: Arc<AsyncCharDeviceImpl> = Arc::new(AsyncCharDeviceImpl::new());
}
