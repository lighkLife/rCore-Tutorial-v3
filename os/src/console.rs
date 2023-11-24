use core::fmt::{self, Write};
use embassy_futures::block_on;

use crate::drivers::chardev::{ASYNC_UART, CharDevice, UART};

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            #[cfg(feature = "async")]
            block_on(write(c));
            #[cfg(not(feature = "async"))]
            UART.write(c as u8);
        }
        Ok(())
    }
}

async fn write(ch: char) {
    ASYNC_UART.clone().write(ch as u8).await;
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
