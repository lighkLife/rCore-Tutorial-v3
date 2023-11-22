use alloc::sync::Arc;

use lazy_static::*;

pub use executor::thread::{Executor, WorkMarker};
pub use async_ns16550a::AsyncNS16550a;
pub use ns16550a::NS16550a;


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

#[cfg(not(feature = "async"))]
lazy_static! {
    pub static ref UART: Arc<CharDeviceImpl> = Arc::new(CharDeviceImpl::new());
}

lazy_static! {
    pub static ref ASYNC_UART: Arc<AsyncCharDeviceImpl> = Arc::new(AsyncCharDeviceImpl::new());
}
