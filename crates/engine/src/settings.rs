use crate::{Properties, Settings};
use anyhow::{bail, Result};

pub fn validate(s: &Settings, p: &Properties) -> Result<()> {
    if p.kind == "image" {
        if !["png", "jpeg", "webp"].contains(&s.format.as_str()) {
            bail!("Choose PNG, JPEG or WebP");
        }
        if s.lossless && s.format == "jpeg" {
            bail!("JPEG lossless encoding is not available");
        }
        if p.bit_depth.unwrap_or(8) > 8 && !(s.lossless && s.format == "png" && p.format == "png") {
            bail!("This high-bit-depth image supports lossless PNG optimization only; precision will not be silently reduced");
        }
        if s.format == "jpeg" && p.alpha && s.background.is_none() {
            bail!("Choose a background before converting transparency to JPEG");
        }
        if let Some(bg) = &s.background {
            if bg.len() != 7 || !bg.starts_with('#') || u32::from_str_radix(&bg[1..], 16).is_err() {
                bail!("Background must be #RRGGBB");
            }
        }
        if !s.lossless {
            let q = s
                .quality
                .ok_or_else(|| anyhow::anyhow!("Choose image quality"))?;
            let min = 0.0;
            if !q.is_finite() || q < min || q > 100.0 || (s.format != "webp" && q.fract() != 0.0) {
                bail!("Quality must be {min}–100 (JPEG and PNG require integers)");
            }
        }
        let max = if s.format == "png" && !s.lossless {
            11
        } else {
            6
        };
        if (s.format == "png" && !s.lossless && s.effort == Some(0)) || s.effort.unwrap_or(2) > max
        {
            bail!("Effort is outside this codec's supported range");
        }
    } else if p.kind == "audio" {
        if !["aac", "mp3", "opus", "flac"].contains(&s.format.as_str()) {
            bail!("Choose AAC, MP3, Opus or FLAC");
        }
        if s.format == "flac" {
            if ![Some(16), Some(24)].contains(&p.bit_depth)
                || p.sample_format.as_deref().unwrap_or("").contains('f')
            {
                bail!("Lossless FLAC requires a 16- or 24-bit integer source");
            }
            if s.effort.unwrap_or(5) > 12 {
                bail!("FLAC effort must be 0–12");
            }
        } else if s.lossless {
            bail!("This audio codec is lossy");
        } else if let ("mp3", Some(q)) = (s.format.as_str(), s.vbr_quality) {
            if !q.is_finite() || !(0.0..=9.999).contains(&q) {
                bail!("MP3 VBR quality must be 0–9.999 (lower is higher quality)");
            }
        } else {
            let b = s
                .bitrate
                .ok_or_else(|| anyhow::anyhow!("Choose a bitrate in kbps"))?;
            let (min, max) = match s.format.as_str() {
                "opus" => (6, 510),
                "mp3" => (8, 320),
                _ => (1, 1024),
            };
            if s.format == "mp3"
                && ![
                    8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 192, 224, 256, 320,
                ]
                .contains(&b)
            {
                bail!("MP3 constant bitrate must be a supported MPEG bitrate (8,16,24,32,40,48,56,64,80,96,112,128,144,160,192,224,256,320 kbps); use VBR for a continuous quality range");
            }
            if b < min || b > max {
                bail!("Bitrate must be {min}–{max} kbps; the encoder may reject incompatible rate/channel combinations");
            }
        }
    } else {
        bail!("Unsupported media");
    }
    Ok(())
}

pub fn output_rate(s: &Settings, p: &Properties) -> u32 {
    let rate = p.sample_rate.unwrap_or(48000);
    let supported: &[u32] = match s.format.as_str() {
        "aac" => &[
            7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
        ],
        "mp3" => &[8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000],
        "opus" => &[48000],
        _ => return rate,
    };
    if supported.contains(&rate) {
        rate
    } else {
        48000
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> Properties {
        Properties {
            kind: "image".into(),
            format: "png".into(),
            alpha: true,
            ..Default::default()
        }
    }
    fn setting() -> Settings {
        Settings {
            format: "webp".into(),
            lossless: false,
            quality: Some(0.0),
            bitrate: None,
            vbr_quality: None,
            effort: Some(2),
            background: None,
        }
    }
    #[test]
    fn aggressive_quality_is_allowed() {
        assert!(validate(&setting(), &image()).is_ok());
    }
    #[test]
    fn transparency_requires_explicit_background() {
        let mut s = setting();
        s.format = "jpeg".into();
        s.quality = Some(1.);
        assert!(validate(&s, &image()).is_err());
        s.background = Some("#ffffff".into());
        assert!(validate(&s, &image()).is_ok());
    }
    #[test]
    fn precision_is_not_silently_reduced() {
        let mut p = image();
        p.bit_depth = Some(16);
        assert!(validate(&setting(), &p).is_err());
    }
    #[test]
    fn nan_quality_is_rejected() {
        let mut s = setting();
        s.quality = Some(f64::NAN);
        assert!(validate(&s, &image()).is_err());
    }
    #[test]
    fn sample_rate_changes_are_explicit() {
        let p = Properties {
            sample_rate: Some(44100),
            ..Default::default()
        };
        let mut s = setting();
        s.format = "opus".into();
        assert_eq!(output_rate(&s, &p), 48000);
        s.format = "aac".into();
        assert_eq!(output_rate(&s, &p), 44100);
    }
}
