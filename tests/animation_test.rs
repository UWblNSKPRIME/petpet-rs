use petpet_rs::petpet::{export_webp, make_animation, DEFAULT_DELAY, DEFAULT_IMAGE_SIZE};

#[test]
fn test_128x128_animated_webp_generation() {
    let assets = [
        (
            "user_784329198310916096.png",
            include_bytes!("assets/user_784329198310916096.png").as_slice(),
        ),
        ("wavycat.png", include_bytes!("assets/wavycat.png").as_slice()),
        ("tasica.png", include_bytes!("assets/tasica.png").as_slice()),
    ];

    for (name, bytes) in assets {
        let img = image::load_from_memory(bytes).expect("Failed to decode image");
        let frames = make_animation(&img);
        assert_eq!(frames.len(), 10);
        assert_eq!(frames[0].width(), DEFAULT_IMAGE_SIZE);
        assert_eq!(frames[0].height(), DEFAULT_IMAGE_SIZE);

        let mut webp_buf = Vec::new();
        export_webp(&mut webp_buf, &frames, DEFAULT_DELAY).expect("WebP export failed");

        println!(
            "{:<30} | 128x128 Animated WebP (24-bit + Alpha) | {} bytes ({:.1} KB)",
            name,
            webp_buf.len(),
            (webp_buf.len() as f64) / 1024.0
        );

        assert!(!webp_buf.is_empty());
        assert_eq!(&webp_buf[0..4], b"RIFF");
        assert_eq!(&webp_buf[8..12], b"WEBP");
        assert!(webp_buf.len() < 5 * 1024 * 1024);
    }
}
