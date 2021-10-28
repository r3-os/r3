#![no_std]
use eg::{
    image::ImageRaw,
    pixelcolor::{raw::LittleEndian, Rgb565},
};
use embedded_graphics as eg;
include!(concat!(env!("OUT_DIR"), "/gen.rs"));
