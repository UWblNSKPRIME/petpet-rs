use anyhow::{anyhow, Result};
use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbaImage};
use once_cell::sync::Lazy;
use std::io::Write;
use zenwebp::decoder::LoopCount;
use zenwebp::mux::{AnimationConfig, AnimationEncoder, BlendMethod, DisposeMethod};
use zenwebp::{EncoderConfig, PixelLayout};

pub const DEFAULT_IMAGE_SIZE: u32 = 128;
pub const DEFAULT_DELAY: u16 = 3; // in 10ms ticks (30ms per frame)

const HORIZONTAL_IMAGE_OFFSET: f64 = 0.1;
const VERTICAL_IMAGE_OFFSET: f64 = 0.08;
const ANIMATION_FRAME_COUNT: usize = 10;
const BASE_IMAGE_SCALE: f64 = 0.8;
const HORIZONTAL_SCALE_STEP: f64 = 0.02;
const VERTICAL_SCALE_STEP: f64 = 0.05;

const WEBP_MILLISECONDS_PER_GIF_TICK: u32 = 10;
const WEBP_QUALITY: f32 = 85.0;

static HANDS: Lazy<[DynamicImage; ANIMATION_FRAME_COUNT]> = Lazy::new(|| {
    let raw_files: [&[u8]; ANIMATION_FRAME_COUNT] = [
        include_bytes!("img/pet0.gif"),
        include_bytes!("img/pet1.gif"),
        include_bytes!("img/pet2.gif"),
        include_bytes!("img/pet3.gif"),
        include_bytes!("img/pet4.gif"),
        include_bytes!("img/pet5.gif"),
        include_bytes!("img/pet6.gif"),
        include_bytes!("img/pet7.gif"),
        include_bytes!("img/pet8.gif"),
        include_bytes!("img/pet9.gif"),
    ];

    raw_files.map(|bytes| {
        image::load_from_memory_with_format(bytes, image::ImageFormat::Gif)
            .expect("Failed to decode embedded hand GIF")
    })
});

fn animation_frame_scale(frame_index: usize) -> (f64, f64) {
    let progress = if frame_index >= ANIMATION_FRAME_COUNT / 2 {
        ANIMATION_FRAME_COUNT - frame_index
    } else {
        frame_index
    };

    (
        BASE_IMAGE_SCALE + (progress as f64) * HORIZONTAL_SCALE_STEP,
        BASE_IMAGE_SCALE - (progress as f64) * VERTICAL_SCALE_STEP,
    )
}

fn create_animation_frame(
    base_img: &DynamicImage,
    width: u32,
    height: u32,
    frame_index: usize,
) -> RgbaImage {
    let (scale_x, scale_y) = animation_frame_scale(frame_index);
    let offset_x = (((1.0 - scale_x) * 0.5 + HORIZONTAL_IMAGE_OFFSET) * (width as f64)) as i64;
    let offset_y = (((1.0 - scale_y) - VERTICAL_IMAGE_OFFSET) * (height as f64)) as i64;

    let target_w = ((width as f64) * scale_x).max(1.0) as u32;
    let target_h = ((height as f64) * scale_y).max(1.0) as u32;

    let mut canvas = RgbaImage::new(width, height);
    // Use Triangle (Bilinear) on WASM for ultra-low CPU usage (<10ms on Cloudflare Free Tier),
    // and Lanczos3 on native targets.
    #[cfg(target_arch = "wasm32")]
    let filter = FilterType::Triangle;
    #[cfg(not(target_arch = "wasm32"))]
    let filter = FilterType::Lanczos3;

    let resized_base = base_img.resize_exact(target_w, target_h, filter);
    imageops::overlay(&mut canvas, &resized_base, offset_x, offset_y);

    let hand = &HANDS[frame_index];
    let resized_hand = if hand.width() != width || hand.height() != height {
        hand.resize_exact(width, height, filter)
    } else {
        hand.clone()
    };
    imageops::overlay(&mut canvas, &resized_hand, 0, 0);

    canvas
}

/// Generates 128x128 Pet-Pet animation frames in 24-bit RGB + 8-bit Alpha (RGBA).
pub fn make_animation(base_img: &DynamicImage) -> Vec<RgbaImage> {
    // Pre-downsample huge images (e.g. 1024x1024) to 256x256 once so per-frame resizing is super fast
    // while still preserving 2x supersampling sharpness for the 128x128 output!
    let prepared = if base_img.width() > 256 || base_img.height() > 256 {
        std::borrow::Cow::Owned(base_img.resize_exact(256, 256, FilterType::Triangle))
    } else {
        std::borrow::Cow::Borrowed(base_img)
    };

    (0..ANIMATION_FRAME_COUNT)
        .map(|idx| create_animation_frame(&prepared, DEFAULT_IMAGE_SIZE, DEFAULT_IMAGE_SIZE, idx))
        .collect()
}

/// Exports 128x128 animation frames in Animated WebP format (24-bit RGB + 8-bit Alpha).
/// Pure Rust implementation (works in both Cloudflare Workers WASM and Native servers).
pub fn export_webp<W: Write>(mut writer: W, frames: &[RgbaImage], delay: u16) -> Result<()> {
    if frames.is_empty() {
        anyhow::bail!("must provide at least one image");
    }

    let width = frames[0].width();
    let height = frames[0].height();

    let anim_cfg = AnimationConfig {
        background_color: [0, 0, 0, 0],
        loop_count: LoopCount::Forever,
        minimize_size: false,
    };

    let mut anim = AnimationEncoder::new(width, height, anim_cfg)
        .map_err(|e| anyhow!("failed to create WebP animation encoder: {:?}", e))?;

    // Method 0 or 1 is fastest and ideal for Cloudflare Workers Free Tier CPU limits
    #[cfg(target_arch = "wasm32")]
    let method = 0;
    #[cfg(not(target_arch = "wasm32"))]
    let method = 2;

    let frame_cfg = EncoderConfig::new_lossy()
        .with_quality(WEBP_QUALITY)
        .with_method(method);

    let frame_duration_ms = (delay as u32) * WEBP_MILLISECONDS_PER_GIF_TICK;

    for (i, frame) in frames.iter().enumerate() {
        let timestamp_ms = (i as u32) * frame_duration_ms;
        anim.add_frame_advanced(
            frame.as_raw(),
            PixelLayout::Rgba8,
            width,
            height,
            0,
            0,
            timestamp_ms,
            &frame_cfg,
            DisposeMethod::Background,
            BlendMethod::Overwrite,
        )
        .map_err(|e| anyhow!("failed to encode WebP frame {}: {:?}", i, e))?;
    }

    let webp_data = anim
        .finalize(frame_duration_ms)
        .map_err(|e| anyhow!("failed to finalize WebP animation: {:?}", e))?;

    writer.write_all(&webp_data)?;
    Ok(())
}
