#![no_std]
#![no_main]

extern crate alloc;
#[macro_use]
extern crate user_lib;

use alloc::vec;
use user_lib::{exit, get_time, thread_create, uart_test, waittid};

#[no_mangle]
pub fn main() -> i32 {
    let start = get_time();
    let v = vec![
        thread_create(uart as usize, 0),
        thread_create(pi as usize, 0),
    ];
    for tid in v.iter() {
        let _ = waittid(*tid as usize);
    }
    println!("============================");
    println!("     total {}ms ", get_time() - start);
    println!("============================");
    0
}

pub fn uart() -> ! {
    let time = uart_test();
    println!("============================");
    println!("uart: {}ms", time);
    println!("============================");
    exit(0)
}

pub fn pi() {
    let start = get_time();
    let mut sum = 0.0;
    let mut i = 1;
    let mut j = 0;
    while i < 1_000_000_00 {
        let it = 1f64 / i as f64;
        if j % 2 == 0 {
            sum += it;
        } else {
            sum -= it;
        }
        i += 2;
        j += 1;
    }

    let result = 4.0 * sum;
    println!("{}", result);
    println!("============================");
    println!("pi:   {}ms", get_time() - start);
    println!("============================");
    exit(0)
}

