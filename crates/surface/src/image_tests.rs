//! Image trust-boundary contracts are table-heavy.
#![allow(clippy::too_many_lines)]

use std::io::Cursor;

use image::{DynamicImage, ImageFormat};

use crate::FileRef;
use crate::image_validate::decode;

fn file(mime: &str, size: usize) -> FileRef {
    FileRef {
        id: "F1".to_owned(),
        name: None,
        media_type: Some(mime.to_owned()),
        size: Some(u64::try_from(size).unwrap()),
        url: Some("unused".to_owned()),
    }
}

fn png_size(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_luma8(width, height)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

// A full-page screenshot (2880x9240 in the field) must reach the model, not
// bounce: anything past the model's effective resolution is downscaled to it.
#[test]
fn oversized_images_are_downscaled_to_the_model_resolution() {
    let oversized = png_size(2_000, 3_200);
    let image = decode(&file("image/png", oversized.len()), oversized).unwrap();
    assert_eq!((image.width(), image.height()), (980, 1_568));
    let decoded = image::load_from_memory(image.bytes()).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (980, 1_568));
}

#[test]
fn downscaled_images_reencode_as_png_regardless_of_source_format() {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_luma8(1_600, 3_200)
        .write_to(&mut bytes, ImageFormat::Jpeg)
        .unwrap();
    let bytes = bytes.into_inner();
    let image = decode(&file("image/jpeg", bytes.len()), bytes).unwrap();
    assert_eq!(image.media_type().as_str(), "image/png");
    assert_eq!((image.width(), image.height()), (784, 1_568));
}

#[test]
fn images_within_the_model_resolution_keep_their_original_bytes() {
    let original = png_size(2, 3);
    let image = decode(&file("image/png", original.len()), original.clone()).unwrap();
    assert_eq!(image.bytes(), original);
}

// The allocation limit is the decompression-bomb guard that survives the
// resize change: a decode too large to buffer still fails closed.
#[test]
fn decompression_bombs_still_fail_closed() {
    let bomb = {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(6_000, 6_000)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    };
    let error = decode(&file("image/png", bomb.len()), bomb).unwrap_err();
    assert_eq!(error.code(), "attachment_too_large");
}

#[test]
fn corrupt_or_mismatched_images_fail_closed() {
    let png = png_size(2, 3);
    let mismatched = decode(&file("image/jpeg", png.len()), png).unwrap_err();
    assert_eq!(mismatched.code(), "invalid_attachment");
    let corrupt = decode(&file("image/png", 3), vec![1, 2, 3]).unwrap_err();
    assert_eq!(corrupt.code(), "invalid_attachment");
}
