#![feature(decl_macro)]
use embedded_graphics::{pixelcolor, prelude::RawData};
use image::{AnimationDecoder, RgbaImage};
use std::{env, fmt::Write, fs, io, path::Path};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir);
    let mut generated_code = String::new();

    macro w($($tt:tt)*) {
	    write!(generated_code, $($tt)*).unwrap();
	}

    // Logo
    let logo_path = manifest_dir.join("assets/logo.svg");
    let logo_svg = nsvg::parse_file(&logo_path, nsvg::Units::Pixel, 96.0).unwrap();
    write_image(
        &mut generated_code,
        out_dir,
        "logo_80",
        &raw_to_image(logo_svg.rasterize_to_raw_rgba(1.7).unwrap()),
    );

    // Animation
    static YELLOW_ONE: &[u8] = include_bytes!("assets/yellow_one.zst");
    let yellow_one = zstd::decode_all(io::Cursor::new(YELLOW_ONE)).unwrap();
    let decoder = image::codecs::gif::GifDecoder::new(io::Cursor::new(&yellow_one[..])).unwrap();
    let frames = decoder.into_frames().collect_frames().unwrap();
    for (i, frame) in frames.iter().enumerate() {
        write_image(
            &mut generated_code,
            out_dir,
            &format!("animation_{i}"),
            frame.buffer(),
        );
    }
    w!("pub static ANIMATION_FRAMES_565: &[fn() -> \
    	ImageRaw<'static, Rgb565, LittleEndian>] = &[\n");
    for i in 0..frames.len() {
        w!("    animation_{i}_565,\n");
    }
    w!("];\n");

    let out_generated_code_path = out_dir.join("gen.rs");
    fs::write(out_generated_code_path, &generated_code).unwrap();
}

fn raw_to_image((width, height, raw): (u32, u32, Vec<u8>)) -> RgbaImage {
    image::RgbaImage::from_raw(width, height, raw).unwrap()
}

fn write_image(out: &mut impl Write, dir: &Path, name: &str, image: &RgbaImage) {
    macro w($($tt:tt)*) {
	    write!(out, $($tt)*).unwrap();
	}
    let pixels888 = image
        .pixels()
        .map(|image::Rgba(data)| pixelcolor::Rgb888::new(data[0], data[1], data[2]));

    let pixels565 = pixels888.map(pixelcolor::Rgb565::from);
    let name565 = format!("{name}_565");
    std::fs::write(
        dir.join(&name565),
        pixels565
            .flat_map(|p| pixelcolor::raw::RawU16::from(p).into_inner().to_le_bytes())
            .collect::<Vec<u8>>(),
    )
    .unwrap();
    w!("pub fn {name565}() -> ImageRaw<'static, Rgb565, LittleEndian> {{\n");
    w!("    static IMAGE: &[u8] = include_bytes!(\"{name565}\");\n");
    w!("    ImageRaw::new(IMAGE, {})\n", image.width());
    w!("}}\n");
}
