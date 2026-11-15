//! An image's reference is how a reply names a tool-returned image so a
//! surface can attach it. It is stored only as reply text, so it must be the
//! same in every build that will ever read that text back.

use agent::{Image, ImageMediaType};
use serde_json::json;

fn image(bytes: &[u8]) -> Image {
    Image::new(ImageMediaType::Png, bytes.to_vec(), 2, 3).expect("valid image")
}

#[test]
fn a_reference_is_a_pinned_digest_of_the_bytes() {
    // SHA-256 of [1, 2, 3] is 039058c6f2c0cb49...; pinning it is what keeps a
    // reply written before a deploy resolvable after it.
    assert_eq!(
        brainstone_media::reference(&image(&[1, 2, 3])),
        "image:039058c6f2c0cb49"
    );
}

#[test]
fn equal_bytes_share_a_reference_and_different_bytes_do_not() {
    let first = image(&[1, 2, 3]);
    let same = Image::new(ImageMediaType::Webp, vec![1, 2, 3], 9, 9).expect("valid image");

    assert_eq!(
        brainstone_media::reference(&first),
        brainstone_media::reference(&same)
    );
    assert_ne!(
        brainstone_media::reference(&first),
        brainstone_media::reference(&image(&[1, 2, 4]))
    );
    assert!(brainstone_media::reference(&first).starts_with(brainstone_media::REFERENCE_PREFIX));
}

#[test]
fn an_attachment_tells_the_model_how_to_show_the_image() {
    assert_eq!(
        brainstone_media::attachment(&image(&[1, 2, 3])),
        json!({
            "reference": "image:039058c6f2c0cb49",
            "media_type": "image/png",
            "width": 2,
            "height": 3,
            "show_in_reply": "![short description](image:039058c6f2c0cb49)",
        })
    );
}
