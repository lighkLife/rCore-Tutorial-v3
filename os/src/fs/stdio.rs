use static_cell::StaticCell;
use crate::drivers::chardev::{ASYNC_UART, Executor, WorkMarker};
use crate::mm::UserBuffer;

use super::File;

pub struct Stdin;

pub struct Stdout;

impl File for Stdin {
    fn readable(&self) -> bool {
        true
    }
    fn writable(&self) -> bool {
        false
    }
    fn read(&self, mut user_buf: UserBuffer) -> usize {
        assert_eq!(user_buf.len(), 1);
        let ch = read();
        unsafe {
            user_buf.buffers[0].as_mut_ptr().write_volatile(ch);
        }
        1
    }
    fn write(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot write to stdin!");
    }
}

static EXECUTOR : StaticCell<Executor> = StaticCell::new();
pub fn read() -> u8 {
    println!("1111");
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(send_data()).unwrap();
    });
    'H' as u8
}

#[embassy_executor::task]
pub async fn send_data() {
    println!("read 1");
    let ch = ASYNC_UART.clone().read().await;
    println!("read {}", ch);
    // mark work as finished
    let mark = WorkMarker {};
    mark.mark_finish();
}

impl File for Stdout {
    fn readable(&self) -> bool {
        false
    }
    fn writable(&self) -> bool {
        true
    }
    fn read(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot read from stdout!");
    }
    fn write(&self, user_buf: UserBuffer) -> usize {
        for buffer in user_buf.buffers.iter() {
            print!("{}", core::str::from_utf8(*buffer).unwrap());
        }
        user_buf.len()
    }
}
