#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(llvm_asm)]
#![feature(naked_functions)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]

mod arm;
mod startup;

// TODO: Add port

fn main() {
    let channels = rtt_target::rtt_init! {
        up: {
            0: {
                size: 1024
                mode: NoBlockSkip
                name: "Terminal"
            }
        }
    };

    unsafe {
        rtt_target::set_print_channel_cs(
            channels.up.0,
            &((|arg, f| f(arg)) as rtt_target::CriticalSectionFunc),
        )
    };

    rtt_target::rprintln!("hello, world!");
    loop {}
}
