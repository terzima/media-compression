use crate::{settings, Diagnostics, ImageRequest, ImageResult, Properties};
use anyhow::{bail, Context, Result};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use lcms2::{
    CIExyY, CIExyYTRIPLE, ColorSpaceSignature, Intent, PixelFormat, Profile, ToneCurve, Transform,
};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
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
        ImageFormat::Jpeg => {
            validate_jpeg_header(path)?;
        }
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
    let mut icc = decoder.icc_profile()?;
    if format == ImageFormat::Png {
        let fallback = png_profile(path)?;
        if icc.is_none() {
            icc = fallback;
        }
    }
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
        color_profile: icc.as_deref().map(Profile::new_icc).transpose()?.map(|p| {
            if p.color_space() == ColorSpaceSignature::GrayData {
                "gray".into()
            } else {
                "rgb".into()
            }
        }),
        bit_depth: Some(color.bits_per_pixel() / color.channel_count() as u16).map(|x| x as u8),
        ..Default::default()
    };
    Ok((image, props, icc))
}

fn validate_jpeg_header(path: &Path) -> Result<()> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut start = [0; 2];
    reader.read_exact(&mut start)?;
    if start != [0xff, 0xd8] {
        bail!("Invalid JPEG header");
    }
    loop {
        if reader.stream_position()? > 64 * 1024 * 1024 {
            bail!("JPEG metadata exceeds the supported header limit");
        }
        let mut byte = [0];
        reader.read_exact(&mut byte)?;
        if byte[0] != 0xff {
            bail!("Invalid JPEG marker");
        }
        loop {
            reader.read_exact(&mut byte)?;
            if byte[0] != 0xff {
                break;
            }
        }
        let marker = byte[0];
        if marker == 0xd9 || marker == 0xda {
            bail!("JPEG frame header missing");
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let mut length = [0; 2];
        reader.read_exact(&mut length)?;
        let length = u16::from_be_bytes(length) as i64;
        if length < 2 {
            bail!("Invalid JPEG segment");
        }
        if [
            0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
        ]
        .contains(&marker)
        {
            let mut frame = [0; 6];
            reader.read_exact(&mut frame)?;
            if frame[0] != 8 {
                bail!("Only 8-bit JPEG precision is supported");
            }
            if ![1, 3].contains(&frame[5]) {
                bail!(
                    "CMYK and other non-RGB JPEGs require an explicit RGB conversion before import"
                );
            }
            return Ok(());
        }
        reader.seek(SeekFrom::Current(length - 2))?;
    }
}

// PNG gAMA/cHRM describe rendering even when there is no embedded ICC profile.
fn png_profile(path: &Path) -> Result<Option<Vec<u8>>> {
    let reader = png::Decoder::new(BufReader::new(File::open(path)?)).read_info()?;
    let info = reader.info();
    if let Some(c) = info.coding_independent_code_points {
        if c.color_primaries != 1
            || c.transfer_function != 13
            || c.matrix_coefficients != 0
            || !c.is_video_full_range_image
        {
            bail!("HDR and non-sRGB cICP images are not supported without an RGB conversion");
        }
    }
    if info.srgb.is_some() || (info.gama_chunk.is_none() && info.chrm_chunk.is_none()) {
        return Ok(None);
    }
    let gamma = info
        .gama_chunk
        .map(|g| g.into_value() as f64)
        .unwrap_or(1. / 2.2);
    if !(0.01..=10.).contains(&gamma) {
        bail!("PNG gamma is outside the supported range");
    }
    let c = info.chrm_chunk.unwrap_or(png::SourceChromaticities::new(
        (0.3127, 0.3290),
        (0.64, 0.33),
        (0.30, 0.60),
        (0.15, 0.06),
    ));
    let xy = |v: (png::ScaledFloat, png::ScaledFloat)| CIExyY {
        x: v.0.into_value() as f64,
        y: v.1.into_value() as f64,
        Y: 1.,
    };
    let curve = ToneCurve::new(1. / gamma);
    let profile = Profile::new_rgb(
        &xy(c.white),
        &CIExyYTRIPLE {
            Red: xy(c.red),
            Green: xy(c.green),
            Blue: xy(c.blue),
        },
        &[&curve, &curve, &curve],
    )?;
    let mut bytes = profile.icc()?;
    // Synthetic profiles must not introduce wall-clock-dependent candidate/cache bytes.
    for (field, value) in bytes[24..36]
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .zip([2000u16, 1, 1, 0, 0, 0])
    {
        field.copy_from_slice(&value.to_be_bytes());
    }
    Ok(Some(bytes))
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
            for chunk in rgba.as_mut().chunks_mut(4 * 4096) {
                let source: Vec<[u8; 3]> = chunk
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| [p[0], p[1], p[2]])
                    .collect();
                let mut target = vec![[0; 3]; source.len()];
                t.transform_pixels(&source, &mut target);
                for (pixel, color) in chunk.as_chunks_mut::<4>().0.iter_mut().zip(target) {
                    pixel[..3].copy_from_slice(&color);
                }
            }
        } else if profile.color_space() == ColorSpaceSignature::GrayData {
            let t: Transform<u8, [u8; 3]> = Transform::new(
                &profile,
                PixelFormat::GRAY_8,
                &srgb,
                PixelFormat::RGB_8,
                Intent::Perceptual,
            )?;
            for chunk in rgba.as_mut().chunks_mut(4 * 4096) {
                let source: Vec<u8> = chunk.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
                let mut target = vec![[0; 3]; source.len()];
                t.transform_pixels(&source, &mut target);
                for (pixel, color) in chunk.as_chunks_mut::<4>().0.iter_mut().zip(target) {
                    pixel[..3].copy_from_slice(&color);
                }
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
fn write_png_profile(path: &Path, rgba: &RgbaImage, icc: Option<&[u8]>) -> Result<()> {
    let mut info = png::Info::with_size(rgba.width(), rgba.height());
    info.color_type = png::ColorType::Rgba;
    info.bit_depth = png::BitDepth::Eight;
    info.icc_profile = icc.map(|bytes| std::borrow::Cow::Owned(bytes.to_vec()));
    let mut writer = png::Encoder::with_info(File::create(path)?, info)?.write_header()?;
    writer.write_image_data(rgba.as_raw())?;
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
    let mut reference = if options.lossless {
        original.to_rgba8()
    } else {
        normalized(&original, icc.as_deref())?
    };
    if icc.is_some() {
        notices.push(if options.lossless {
            "Rendering profile preserved".into()
        } else {
            "Embedded color profile converted to sRGB".into()
        });
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
    write_png_profile(
        &prepared,
        &reference,
        if options.lossless {
            icc.as_deref()
        } else {
            None
        },
    )?;
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
                "-metadata".into(),
                if options.lossless { "icc" } else { "none" }.into(),
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
    if options.lossless {
        if original.to_rgba8() != after.to_rgba8() {
            bail!("Lossless source pixel verification failed");
        }
        reference = normalized(&original, icc.as_deref())?;
    }
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
        (Some(1.), Some(1.))
    } else if reference.width() < 8 || reference.height() < 8 {
        notices.push("SSIM unavailable for images smaller than 8 pixels on either axis; candidate remains exportable".into());
        (None, None)
    } else {
        let measure = |background, label| {
            ssim(
                &request.tools.ffmpeg,
                &composite(&reference, background),
                &composite(&candidate, background),
                temp.path(),
                label,
            )
        };
        match (measure([255; 3], "light"), measure([18, 22, 30], "dark")) {
            (Ok(a), Ok(b)) => (Some(a), Some(b)),
            (a, b) => {
                notices.push(format!(
                    "SSIM unavailable: {:?}; {:?}; candidate remains exportable",
                    a.err(),
                    b.err()
                ));
                (None, None)
            }
        }
    };
    notices.push(
        if options.lossless {
            "Orientation applied; private image metadata removed; rendering profile retained"
        } else {
            "Orientation applied; private image metadata removed; output uses sRGB interpretation"
        }
        .into(),
    );
    Ok(ImageResult {
        properties: after_props,
        diagnostics: Diagnostics {
            ssim_light: light,
            ssim_dark: dark,
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

#[cfg(test)]
mod profile_tests {
    use super::*;
    #[test]
    fn png_gamma_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gamma.png");
        let mut encoder = png::Encoder::new(File::create(&path).unwrap(), 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_source_gamma(png::ScaledFloat::new(1.));
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[64, 120, 180, 180]).unwrap();
        drop(writer);
        let (image, _, icc) = open(&path).unwrap();
        let converted = normalized(&image, icc.as_deref()).unwrap();
        assert!(
            converted.get_pixel(0, 0)[0] > 100,
            "Raw {:?}, normalized {:?}",
            image.to_rgba8().get_pixel(0, 0),
            converted.get_pixel(0, 0)
        );
    }
}
