use crate::{settings, Diagnostics, ImageRequest, ImageResult, Properties};
use anyhow::{bail, Context, Result};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use lcms2::{ColorSpaceSignature, Intent, PixelFormat, Profile, Transform};
use std::{
    fs::File,
    io::{BufReader, Write},
    path::Path,
    process::Command,
};

fn open(path: &Path) -> Result<(DynamicImage, Properties, Option<Vec<u8>>)> {
    let format = ImageReader::open(path)?
        .with_guessed_format()?
        .format()
        .context("Unknown image format")?;
    match format {
        ImageFormat::Png => {
            if image::codecs::png::PngDecoder::new(BufReader::new(File::open(path)?))?.is_apng()? {
                bail!("Animated PNG is outside this release");
            }
        }
        ImageFormat::WebP => {
            if image::codecs::webp::WebPDecoder::new(BufReader::new(File::open(path)?))?
                .has_animation()
            {
                bail!("Animated WebP is outside this release");
            }
        }
        ImageFormat::Jpeg => {}
        _ => bail!("Use static PNG, JPEG or WebP"),
    }
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(20000);
    limits.max_image_height = Some(20000);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if width as u64 * height as u64 > 50_000_000 {
        bail!("Image exceeds the 50-megapixel processing limit");
    }
    let color = decoder.color_type();
    let icc = decoder.icc_profile()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    let props = Properties {
        kind: "image".into(),
        format: match format {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpeg",
            _ => "webp",
        }
        .into(),
        width: Some(image.width()),
        height: Some(image.height()),
        alpha: color.has_alpha(),
        bit_depth: Some(color.bits_per_pixel() / color.channel_count() as u16).map(|x| x as u8),
        ..Default::default()
    };
    Ok((image, props, icc))
}

fn normalized(image: &DynamicImage, icc: Option<&[u8]>) -> Result<RgbaImage> {
    let mut rgba = image.to_rgba8();
    if let Some(bytes) = icc {
        let profile = Profile::new_icc(bytes).context("Invalid embedded color profile")?;
        let srgb = Profile::new_srgb();
        if profile.color_space() == ColorSpaceSignature::RgbData {
            let t: Transform<[u8; 3], [u8; 3]> = Transform::new(
                &profile,
                PixelFormat::RGB_8,
                &srgb,
                PixelFormat::RGB_8,
                Intent::Perceptual,
            )?;
            let source: Vec<[u8; 3]> = rgba.pixels().map(|p| [p[0], p[1], p[2]]).collect();
            let mut target = vec![[0; 3]; source.len()];
            t.transform_pixels(&source, &mut target);
            for (p, c) in rgba.pixels_mut().zip(target) {
                p[0] = c[0];
                p[1] = c[1];
                p[2] = c[2];
            }
        } else if profile.color_space() == ColorSpaceSignature::GrayData {
            let t: Transform<u8, [u8; 3]> = Transform::new(
                &profile,
                PixelFormat::GRAY_8,
                &srgb,
                PixelFormat::RGB_8,
                Intent::Perceptual,
            )?;
            let source: Vec<u8> = rgba.pixels().map(|p| p[0]).collect();
            let mut target = vec![[0; 3]; source.len()];
            t.transform_pixels(&source, &mut target);
            for (p, c) in rgba.pixels_mut().zip(target) {
                p[0] = c[0];
                p[1] = c[1];
                p[2] = c[2];
            }
        } else {
            bail!("This color profile is not safely supported; convert the source to RGB first");
        }
    }
    Ok(rgba)
}
fn composite(image: &RgbaImage, bg: [u8; 3]) -> image::RgbImage {
    image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y);
        let a = p[3] as u32;
        image::Rgb(
            [0, 1, 2].map(|i| ((p[i] as u32 * a + bg[i] as u32 * (255 - a) + 127) / 255) as u8),
        )
    })
}
fn helper(program: &Path, args: &[String]) -> Result<()> {
    let output = Command::new(program).args(args).output().with_context(|| {
        format!(
            "Cannot start {}",
            program.file_name().unwrap_or_default().to_string_lossy()
        )
    })?;
    if !output.status.success() {
        bail!(
            "Encoder failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
fn write_ppm(path: &Path, image: &image::RgbImage) -> Result<()> {
    let mut file = File::create(path)?;
    write!(file, "P6\n{} {}\n255\n", image.width(), image.height())?;
    file.write_all(image.as_raw())?;
    Ok(())
}
fn ssim(
    ffmpeg: &Path,
    original: &image::RgbImage,
    candidate: &image::RgbImage,
    temp: &Path,
    label: &str,
) -> Result<f64> {
    let a = temp.join(format!("{label}-original.ppm"));
    let b = temp.join(format!("{label}-candidate.ppm"));
    write_ppm(&a, original)?;
    write_ppm(&b, candidate)?;
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-threads", "1", "-i"])
        .arg(a)
        .args(["-threads", "1", "-i"])
        .arg(b)
        .args([
            "-filter_complex_threads",
            "1",
            "-lavfi",
            "ssim",
            "-f",
            "null",
            "-",
        ])
        .output()?;
    if !output.status.success() {
        bail!(
            "Comparison decode failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let logs = String::from_utf8_lossy(&output.stderr);
    let value = logs
        .rsplit("All:")
        .next()
        .context("SSIM result missing")?
        .split_whitespace()
        .next()
        .context("SSIM result missing")?
        .parse::<f64>()?;
    if !value.is_finite() {
        bail!("SSIM result is not finite");
    }
    Ok(value)
}

pub fn execute(request: ImageRequest) -> Result<ImageResult> {
    let (original, props, icc) = open(&request.input)?;
    if request.action == "inspect" {
        normalized(&original, icc.as_deref())?;
        return Ok(ImageResult {
            properties: props,
            diagnostics: Diagnostics::default(),
        });
    }
    if request.action == "probe" {
        let rgba = normalized(&original, icc.as_deref())?;
        // Preview bounds are independent from full-resolution validation and metrics.
        DynamicImage::ImageRgba8(rgba).save_with_format(&request.output, ImageFormat::Png)?;
        return Ok(ImageResult {
            properties: props,
            diagnostics: Diagnostics::default(),
        });
    }
    if request.action != "compress" {
        bail!("Unknown image operation");
    }
    let options = request
        .settings
        .as_ref()
        .context("Compression settings missing")?;
    settings::validate(options, &props)?;
    let temp = tempfile::tempdir_in(
        request
            .output
            .parent()
            .context("Output directory missing")?,
    )?;
    let mut notices = Vec::new();
    if options.lossless && options.format == "png" && props.format == "png" {
        let mut oxi = oxipng::Options::from_preset(options.effort.unwrap_or(2));
        oxi.strip = oxipng::StripChunks::Safe;
        oxi.optimize_alpha = false;
        oxipng::optimize(
            &oxipng::InFile::Path(request.input.clone()),
            &oxipng::OutFile::from_path(request.output.clone()),
            &oxi,
        )?;
        let (after, after_props, _) = open(&request.output)?;
        if original.color() != after.color() {
            // Palette/bit-depth reductions are legal only when sample values match.
            if props.bit_depth == Some(16) {
                if original.to_rgba16() != after.to_rgba16() {
                    bail!("Lossless pixel check failed");
                }
            } else if original.to_rgba8() != after.to_rgba8() {
                bail!("Lossless pixel check failed");
            }
        } else if original.as_bytes() != after.as_bytes() {
            bail!("Lossless pixel check failed");
        }
        return Ok(ImageResult{properties:after_props,diagnostics:Diagnostics{ssim_light:Some(1.),ssim_dark:Some(1.),alpha_max_error:Some(0),alpha_mean_error:Some(0.),pixel_identical:Some(true),notices:vec!["Lossless pixels verified; rendering metadata retained and safe ancillary metadata removed".into()],..Default::default()}});
    }
    let mut reference = normalized(&original, icc.as_deref())?;
    if icc.is_some() {
        notices.push("Embedded color profile converted to sRGB".into());
    }
    if options.format == "jpeg" && props.alpha {
        let bg = options
            .background
            .as_deref()
            .context("JPEG background missing")?;
        let n = u32::from_str_radix(&bg[1..], 16)?;
        reference = DynamicImage::ImageRgb8(composite(
            &reference,
            [(n >> 16) as u8, (n >> 8) as u8, n as u8],
        ))
        .to_rgba8();
        notices.push(format!("Transparency flattened onto {bg}"));
    }
    let prepared = temp.path().join("normalized.png");
    reference.save(&prepared)?;
    let quality = options.quality.unwrap_or(100.).to_string();
    match options.format.as_str() {
        "png" => {
            if options.lossless {
                std::fs::copy(&prepared, &request.output)?;
            } else {
                helper(
                    &request.tools.pngquant,
                    &[
                        format!("--quality=0-{quality}"),
                        "--speed".into(),
                        options.effort.unwrap_or(3).max(1).to_string(),
                        "--strip".into(),
                        "--output".into(),
                        request.output.to_string_lossy().into(),
                        prepared.to_string_lossy().into(),
                    ],
                )?;
            }
            let mut oxi = oxipng::Options::from_preset(2);
            oxi.strip = oxipng::StripChunks::Safe;
            oxi.optimize_alpha = false;
            oxipng::optimize(
                &oxipng::InFile::Path(request.output.clone()),
                &oxipng::OutFile::from_path(request.output.clone()),
                &oxi,
            )?;
        }
        "webp" => {
            let mut args = vec![
                "-quiet".into(),
                "-m".into(),
                options.effort.unwrap_or(4).to_string(),
                "-exact".into(),
                "-alpha_q".into(),
                "100".into(),
            ];
            if options.lossless {
                args.push("-lossless".into());
            } else {
                args.extend(["-q".into(), quality]);
            }
            args.extend([
                prepared.to_string_lossy().into(),
                "-o".into(),
                request.output.to_string_lossy().into(),
            ]);
            helper(&request.tools.cwebp, &args)?;
        }
        "jpeg" => {
            let ppm = temp.path().join("normalized.ppm");
            let rgb = DynamicImage::ImageRgba8(reference.clone()).to_rgb8();
            let mut file = File::create(&ppm)?;
            write!(file, "P6\n{} {}\n255\n", rgb.width(), rgb.height())?;
            file.write_all(rgb.as_raw())?;
            drop(file);
            helper(
                &request.tools.cjpeg,
                &[
                    "-quality".into(),
                    quality,
                    "-optimize".into(),
                    "-progressive".into(),
                    "-outfile".into(),
                    request.output.to_string_lossy().into(),
                    ppm.to_string_lossy().into(),
                ],
            )?;
        }
        _ => bail!("Unknown output format"),
    }
    let (after, after_props, after_icc) = open(&request.output)?;
    let candidate = normalized(&after, after_icc.as_deref())?;
    if candidate.dimensions() != reference.dimensions() {
        bail!("Image dimensions changed");
    }
    let identical = reference == candidate;
    if options.lossless && !identical {
        bail!("Lossless normalized pixel check failed");
    }
    let mut max = 0;
    let mut sum = 0u64;
    for (a, b) in reference.pixels().zip(candidate.pixels()) {
        let error = a[3].abs_diff(b[3]);
        max = max.max(error);
        sum += error as u64;
    }
    let (light, dark) = if identical {
        (1., 1.)
    } else {
        (
            ssim(
                &request.tools.ffmpeg,
                &composite(&reference, [255; 3]),
                &composite(&candidate, [255; 3]),
                temp.path(),
                "light",
            )?,
            ssim(
                &request.tools.ffmpeg,
                &composite(&reference, [18, 22, 30]),
                &composite(&candidate, [18, 22, 30]),
                temp.path(),
                "dark",
            )?,
        )
    };
    notices.push(
        "Orientation applied; private image metadata removed; output uses sRGB interpretation"
            .into(),
    );
    Ok(ImageResult {
        properties: after_props,
        diagnostics: Diagnostics {
            ssim_light: Some(light),
            ssim_dark: Some(dark),
            alpha_max_error: Some(max),
            alpha_mean_error: Some(
                sum as f64 / (reference.width() as f64 * reference.height() as f64),
            ),
            pixel_identical: Some(identical),
            notices,
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_composite_is_background_aware() {
        let image = RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 0]));
        assert_eq!(composite(&image, [255; 3]).get_pixel(0, 0).0, [255; 3]);
        assert_eq!(composite(&image, [0; 3]).get_pixel(0, 0).0, [0; 3]);
    }
}
