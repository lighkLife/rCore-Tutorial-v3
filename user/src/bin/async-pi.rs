#![no_main]
#![no_std]
extern crate alloc;
#[macro_use]
extern crate user_lib;

use user_lib::{get_time, uart_test};

#[no_mangle]
pub fn main() -> i32 {
    let start = get_time();
    uart_test();
    println!("============================");
    println!("     total {}ms ", get_time() - start);
    println!("============================");
    0
}

