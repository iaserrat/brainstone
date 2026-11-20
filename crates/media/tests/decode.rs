//! Untrusted bytes become model input only as a bounded, decodable raster in
//! a provider-accepted format, no larger than the model will look at.

use std::io::Cursor;

use agent::ImageMediaType;
use brainstone_media::MediaError;
use image::{DynamicImage, ImageFormat};

fn encoded(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_rgb8(width, height)
        .write_to(&mut bytes, format)
        .unwrap();
    bytes.into_inner()
}

#[test]
fn supported_media_types_are_named_and_others_are_not() {
    assert_eq!(
        brainstone_media::media_type("image/png"),
        Ok(ImageMediaType::Png)
    );
    assert_eq!(
        brainstone_media::media_type("image/jpeg"),
        Ok(ImageMediaType::Jpeg)
    );
    assert_eq!(
        brainstone_media::media_type("image/webp"),
        Ok(ImageMediaType::Webp)
    );
    for unsupported in ["image/gif", "image/svg+xml", "", "IMAGE/PNG"] {
        assert_eq!(
            brainstone_media::media_type(unsupported),
            Err(MediaError::Unsupported),
            "{unsupported}"
        );
    }
}

#[test]
fn an_image_within_the_model_resolution_keeps_its_bytes() {
    let original = encoded(2, 3, ImageFormat::Png);

    let image = brainstone_media::decode(original.clone(), ImageMediaType::Png).unwrap();

    assert_eq!(image.bytes(), original);
    assert_eq!((image.width(), image.height()), (2, 3));
}

#[test]
fn an_oversized_image_is_fitted_to_the_model_resolution_as_png() {
    let original = encoded(3_200, 1_600, ImageFormat::Jpeg);

    let image = brainstone_media::decode(original, ImageMediaType::Jpeg).unwrap();

    assert_eq!(image.media_type(), ImageMediaType::Png);
    assert_eq!((image.width(), image.height()), (1_568, 784));
}

#[test]
fn bytes_past_the_per_image_cap_are_too_large_before_decoding() {
    let oversized = vec![0; brainstone_media::MAX_IMAGE_BYTES + 1];

    assert_eq!(
        brainstone_media::decode(oversized, ImageMediaType::Png),
        Err(MediaError::TooLarge)
    );
}

#[test]
fn empty_corrupt_or_mislabelled_bytes_are_invalid() {
    let png = encoded(2, 3, ImageFormat::Png);
    for (bytes, declared) in [
        (Vec::new(), ImageMediaType::Png),
        (vec![1, 2, 3], ImageMediaType::Png),
        (png, ImageMediaType::Jpeg),
    ] {
        assert_eq!(
            brainstone_media::decode(bytes, declared),
            Err(MediaError::Invalid)
        );
    }
}

#[test]
fn a_raster_in_another_format_is_unsupported() {
    // A GIF header: sniffed as a real raster, but not one providers accept.
    let gif = b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".to_vec();

    assert_eq!(
        brainstone_media::decode(gif, ImageMediaType::Png),
        Err(MediaError::Unsupported)
    );
}

#[test]
fn png_encoding_keeps_dimensions_and_transcodes_webp() {
    let webp = encoded(4, 5, ImageFormat::WebP);
    let image = brainstone_media::decode(webp, ImageMediaType::Webp).unwrap();

    let png = brainstone_media::png(&image).unwrap();

    let decoded = image::load_from_memory_with_format(&png, ImageFormat::Png).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (4, 5));
}
