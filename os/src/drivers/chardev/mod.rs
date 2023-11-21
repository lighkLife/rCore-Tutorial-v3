mod ns16550a;
mod raw_ns16550a;
mod executor;

use crate::board::CharDeviceImpl;
use alloc::sync::Arc;
use lazy_static::*;
pub use ns16550a::NS16550a;

#[cfg(feature = "async")]
pub use executor::thread::Executor;

pub trait CharDevice {
    fn init(&self);
    fn read(&self) -> u8;
    fn write(&self, ch: u8);
    fn handle_irq(&self);
}

lazy_static! {
    pub static ref UART: Arc<CharDeviceImpl> = Arc::new(CharDeviceImpl::new());
}

// #[cfg(feature = "async")]
// lazy_static! {
//     pub static ref ASYNC_UART: Arc<CharDeviceImpl> = Arc::new(CharDeviceImpl::new());
// }
