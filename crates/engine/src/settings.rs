use crate::{Properties, Settings};
use anyhow::{bail, Result};

pub fn validate(s: &Settings, p: &Properties) -> Result<()> {
    if p.kind == "image" {
        if !["png", "jpeg", "webp"].contains(&s.format.as_str()) {
            bail!("Choose PNG, JPEG or WebP");
        }
        if s.lossless
            && p.color_profile.as_deref() == Some("gray")
            && !(s.format == "png" && p.format == "png")
        {
            bail!("Lossless conversion of gray ICC profiles is unsupported; use lossless PNG optimization or a lossy RGB conversion");
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
        if s.format == "mp3" && s.effort.is_some_and(|v| v > 9) {
            bail!("MP3 encoding effort must be 0–9 (lower is slower)");
        }
        if s.format == "opus" && s.effort.is_some_and(|v| v > 10) {
            bail!("Opus encoding effort must be 0–10");
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
                "opus" => (0.5, 256. * p.channels.unwrap_or(2) as f64),
                "mp3" => (8., 320.),
                _ => (
                    0.001,
                    output_rate(s, p) as f64 * 6. * p.channels.unwrap_or(2) as f64 / 1000.,
                ),
            };
            if s.format == "mp3" {
                let rate = output_rate(s, p);
                let allowed: &[u32] = if rate >= 32000 {
                    &[
                        32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
                    ]
                } else if rate >= 16000 {
                    &[8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]
                } else {
                    &[8, 16, 24, 32, 40, 48, 56, 64]
                };
                if b.fract() != 0. || !allowed.contains(&(b as u32)) {
                    bail!("At {rate} Hz, MP3 constant bitrate must be one of {allowed:?} kbps; VBR provides a continuous quality control");
                }
            }
            if !b.is_finite()
                || (b * 1000. - (b * 1000.).round()).abs() > 0.000001
                || b < min
                || b > max
            {
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
    #[test]
    fn audio_ranges_follow_codec_and_channel_limits() {
        let mut p = Properties {
            kind: "audio".into(),
            channels: Some(1),
            sample_rate: Some(44100),
            ..Default::default()
        };
        let mut s = setting();
        s.format = "opus".into();
        s.bitrate = Some(0.5);
        assert!(validate(&s, &p).is_ok());
        s.bitrate = Some(256.);
        assert!(validate(&s, &p).is_ok());
        s.bitrate = Some(256.001);
        assert!(validate(&s, &p).is_err());
        p.channels = Some(2);
        s.bitrate = Some(512.);
        assert!(validate(&s, &p).is_ok());
        s.bitrate = Some(f64::NAN);
        assert!(validate(&s, &p).is_err());
        s.format = "aac".into();
        s.bitrate = Some(529.2);
        assert!(validate(&s, &p).is_ok());
        s.bitrate = Some(529.201);
        assert!(validate(&s, &p).is_err());
        s.format = "mp3".into();
        p.sample_rate = Some(8000);
        s.bitrate = Some(64.);
        assert!(validate(&s, &p).is_ok());
        s.bitrate = Some(80.);
        assert!(validate(&s, &p).is_err());
        s.bitrate = Some(8.5);
        assert!(validate(&s, &p).is_err());
    }
}
