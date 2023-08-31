#![warn(clippy::arithmetic_side_effects)]
#![allow(internal_features, unused)]
#![feature(lang_items, start, libc)]
#![no_std]

use core::num::Wrapping;

struct Foo {
    wrapping: Wrapping<usize>,
}

pub fn foo(n: usize) -> usize {
    let mut val = Foo { wrapping: Wrapping(5) };
    val.wrapping += n;
    val.wrapping.0
}

#[start]
fn main(_argc: isize, _argv: *const *const u8) -> isize {
    let mut x: Wrapping<u32> = Wrapping(0_u32);
    x += 1;

    0
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
