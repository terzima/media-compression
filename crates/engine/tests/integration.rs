use media_engine::{hash_file, Engine, Settings, Tools};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

fn tools() -> Tools {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let dir = std::env::var_os("MEDIA_CODEC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("src-tauri/resources/codecs"));
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let target = std::env::var("MEDIA_BUILD_TARGET").unwrap_or_else(|_| {
        if cfg!(windows) {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
        .into()
    });
    Tools {
        worker: std::env::var_os("MEDIA_WORKER_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join(format!("src-tauri/binaries/media-worker-{target}{ext}"))),
        ffmpeg: dir.join(format!("ffmpeg{ext}")),
        ffprobe: dir.join(format!("ffprobe{ext}")),
        pngquant: dir.join(format!("pngquant{ext}")),
        cwebp: dir.join(format!("cwebp{ext}")),
        cjpeg: dir.join(format!("cjpeg{ext}")),
    }
}
fn engine(root: &Path) -> Arc<Engine> {
    Engine::new(root.to_path_buf(), tools(), || {}).unwrap()
}
fn fixture(root: &Path) -> PathBuf {
    let path = root.join("original 🐈.png");
    let image = image::RgbaImage::from_fn(64, 64, |x, y| {
        image::Rgba([
            (x * 4) as u8,
            (y * 4) as u8,
            ((x + y) * 2) as u8,
            if x < 10 { 0 } else { 255 },
        ])
    });
    image.save(&path).unwrap();
    path
}
fn image(format: &str, lossless: bool, quality: f64) -> Settings {
    Settings {
        format: format.into(),
        lossless,
        quality: Some(quality),
        effort: Some(2),
        bitrate: None,
        vbr_quality: None,
        background: None,
    }
}
fn wait(engine: &Engine) {
    let start = Instant::now();
    loop {
        let data = engine.snapshot();
        if data
            .jobs
            .iter()
            .all(|j| !["queued", "processing"].contains(&j.state.as_str()))
        {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(180),
            "Jobs timed out: {:?}",
            data.jobs
        );
        std::thread::sleep(Duration::from_millis(40));
    }
}
#[test]
#[ignore = "requires bundled helpers: npm run codecs && npm run worker"]
fn actual_image_study_preserves_source_deduplicates_and_exports_without_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let original = fixture(dir.path());
    let hash = hash_file(&original).unwrap();
    let engine = engine(&dir.path().join("workspace"));
    engine.import(vec![original.clone()]).unwrap();
    let id = engine.snapshot().media[0].id.clone();
    engine
        .start(vec![(
            id.clone(),
            vec![
                image("png", true, 100.),
                image("webp", true, 100.),
                image("webp", false, 0.),
                image("webp", false, 80.),
                image("webp", false, 80.),
            ],
        )])
        .unwrap();
    wait(&engine);
    let data = engine.snapshot();
    assert_eq!(data.jobs[0].state, "ready", "{:?}", data.jobs);
    assert!(data.jobs[0].errors.is_empty(), "{:?}", data.jobs);
    assert_eq!(data.media[0].candidates.len(), 4);
    assert_eq!(hash_file(&original).unwrap(), hash);
    for c in &data.media[0].candidates {
        assert_eq!(c.properties.width, Some(64));
        assert!(c.diagnostics.ssim_light.unwrap().is_finite());
        if c.settings[0].lossless {
            assert_eq!(c.diagnostics.pixel_identical, Some(true));
        }
        let preview = engine.preview(&c.id).unwrap();
        assert!(Path::new(&preview).exists());
    }
    let ids = data.media[0]
        .candidates
        .iter()
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    let export = dir.path().join("输出");
    let first = engine.export(&ids, &export, true).unwrap();
    let again = engine.export(&ids, &export, true).unwrap();
    assert_eq!(first.len(), 4);
    assert_eq!(again.len(), 4);
    for name in &first {
        assert!(!again.contains(name));
    }
    assert_eq!(hash_file(&original).unwrap(), hash);
    // Cached candidate tampering is repaired by recompression rather than treated as verified.
    let c = &data.media[0].candidates[0];
    std::fs::write(&c.path, b"corrupt").unwrap();
    engine
        .start(vec![(id, vec![c.settings[0].clone()])])
        .unwrap();
    wait(&engine);
    assert_eq!(hash_file(&c.path).unwrap(), c.sha256);
}
#[test]
#[ignore = "requires bundled helpers"]
fn actual_audio_formats_timing_and_lossless_samples() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boundary-tone.wav");
    let mut wav = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..44100 {
        let x = if i == 0 || i == 44099 {
            4_000_000
        } else {
            ((i as f64 * 0.062689).sin() * 2_000_000.) as i32
        };
        wav.write_sample(x).unwrap();
        wav.write_sample(-x).unwrap();
    }
    wav.finalize().unwrap();
    let hash = hash_file(&path).unwrap();
    let engine = engine(&dir.path().join("workspace"));
    engine.import(vec![path.clone()]).unwrap();
    let id = engine.snapshot().media[0].id.clone();
    let options = [
        ("aac", Some(128.), None),
        ("mp3", None, Some(4.)),
        ("opus", Some(96.), None),
        ("flac", None, None),
    ]
    .map(|(format, bitrate, vbr_quality)| Settings {
        format: format.into(),
        lossless: format == "flac",
        quality: None,
        effort: Some(5),
        bitrate,
        vbr_quality,
        background: None,
    })
    .to_vec();
    engine.start(vec![(id, options)]).unwrap();
    wait(&engine);
    let data = engine.snapshot();
    assert!(data.jobs[0].errors.is_empty(), "{:?}", data.jobs);
    assert_eq!(data.media[0].candidates.len(), 4);
    for c in &data.media[0].candidates {
        assert_eq!(c.properties.channels, Some(2));
        assert!(c.diagnostics.duration_delta.unwrap().abs() < 0.025);
        let (pcm, _, _) = engine.playback_pcm(&c.id).unwrap();
        assert!(std::fs::metadata(pcm).unwrap().len() > 0);
    }
    assert_eq!(hash_file(&path).unwrap(), hash);
}
#[test]
#[ignore = "requires bundled helpers"]
fn invalid_batch_is_atomic_and_source_changes_block_export() {
    let dir = tempfile::tempdir().unwrap();
    let original = fixture(dir.path());
    let engine = engine(&dir.path().join("workspace"));
    engine.import(vec![original.clone()]).unwrap();
    let id = engine.snapshot().media[0].id.clone();
    assert!(engine
        .start(vec![
            (id.clone(), vec![image("png", true, 100.)]),
            ("unknown".into(), vec![image("png", true, 100.)])
        ])
        .is_err());
    assert!(engine.snapshot().jobs.is_empty());
    engine
        .start(vec![(id, vec![image("png", true, 100.)])])
        .unwrap();
    wait(&engine);
    let candidate = engine.snapshot().media[0].candidates[0].id.clone();
    std::fs::write(&original, b"changed").unwrap();
    assert!(engine
        .export(&[candidate], &dir.path().join("export"), false)
        .is_err());
    assert!(std::fs::read_dir(dir.path().join("export"))
        .unwrap()
        .next()
        .is_none());
    // One failed file must not prevent independent exports or retrying a repaired input.
    let good = dir.path().join("unaffected.png");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([70, 80, 90, 255]))
        .save(&good)
        .unwrap();
    engine.import(vec![good.clone()]).unwrap();
    let good_id = engine.snapshot().media.last().unwrap().id.clone();
    engine
        .start(vec![(good_id, vec![image("png", true, 100.)])])
        .unwrap();
    wait(&engine);
    let data = engine.snapshot();
    let ids = data
        .media
        .iter()
        .flat_map(|m| &m.candidates)
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    assert!(engine
        .export(&ids, &dir.path().join("partial"), true)
        .is_err());
    assert!(dir
        .path()
        .join("partial/unaffected-compressed.png")
        .exists());
    assert!(!engine.snapshot().media[0].candidates[0]
        .export_errors
        .is_empty());
    engine.import(vec![original.clone()]).unwrap();
    assert!(engine.snapshot().media.last().unwrap().error.is_some());
    image::RgbaImage::from_pixel(16, 16, image::Rgba([120, 80, 90, 255]))
        .save(&original)
        .unwrap();
    engine.import(vec![original.clone()]).unwrap();
    assert!(engine.snapshot().media.last().unwrap().error.is_none());
    engine.import(vec![original]).unwrap();
    assert!(engine
        .snapshot()
        .media
        .last()
        .unwrap()
        .error
        .as_deref()
        .unwrap()
        .contains("already imported"));
}

#[test]
#[ignore = "requires bundled helpers; exercises a 1,000-file mixed batch"]
fn thousand_file_batch_and_queued_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = dir.path().join("mixed");
    std::fs::create_dir(&inputs).unwrap();
    let template = fixture(dir.path());
    let audio = dir.path().join("short.wav");
    let mut wav = hound::WavWriter::create(
        &audio,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..480 {
        wav.write_sample(0i16).unwrap();
    }
    wav.finalize().unwrap();
    for n in 0..1000 {
        let (source, ext) = if n % 2 == 0 {
            (&template, "png")
        } else {
            (&audio, "wav")
        };
        std::fs::copy(source, inputs.join(format!("fixture-{n}.{ext}"))).unwrap();
    }
    let engine = engine(&dir.path().join("workspace"));
    engine.import(vec![inputs]).unwrap();
    let data = engine.snapshot();
    assert_eq!(data.media.len(), 1000);
    assert!(data.media.iter().all(|m| m.error.is_none()));
    let items = data
        .media
        .iter()
        .map(|m| {
            (
                m.id.clone(),
                vec![if m.properties.as_ref().unwrap().kind == "image" {
                    image("png", true, 100.)
                } else {
                    Settings {
                        format: "flac".into(),
                        lossless: true,
                        quality: None,
                        bitrate: None,
                        vbr_quality: None,
                        effort: Some(5),
                        background: None,
                    }
                }],
            )
        })
        .collect::<Vec<_>>();
    engine.start(items.clone()).unwrap();
    wait(&engine);
    assert!(engine
        .snapshot()
        .jobs
        .iter()
        .all(|j| j.state == "ready" && j.errors.is_empty()));
    assert!(engine
        .snapshot()
        .media
        .iter()
        .all(|m| m.candidates.len() == 1));
    // Once all results are cached, a fresh 1,000-item sweep can still cancel queued work immediately.
    let start = Instant::now();
    engine.start(items).unwrap();
    engine.cancel(None);
    assert!(engine.snapshot().jobs.iter().all(|j| j.state != "queued"));
    wait(&engine);
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
#[ignore = "requires bundled helpers"]
fn color_metadata_precision_orientation_and_small_images() {
    let dir = tempfile::tempdir().unwrap();
    let colored = dir.path().join("gamma.png");
    let mut encoder = png::Encoder::new(std::fs::File::create(&colored).unwrap(), 16, 16);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_source_gamma(png::ScaledFloat::new(1.));
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&[64, 120, 180, 180].repeat(256))
        .unwrap();
    drop(writer);
    let sixteen = dir.path().join("precision.png");
    let img = image::ImageBuffer::from_fn(16, 16, |x, y| {
        image::Rgba([(x * 997) as u16, (y * 1133) as u16, 2037u16, 50001u16])
    });
    image::DynamicImage::ImageRgba16(img)
        .save(&sixteen)
        .unwrap();
    let tiny = dir.path().join("tiny.png");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([63, 111, 174, 200]))
        .save(&tiny)
        .unwrap();
    let oriented = dir.path().join("orientation.jpg");
    image::RgbImage::from_fn(12, 8, |x, y| image::Rgb([x as u8 * 20, y as u8 * 30, 100]))
        .save(&oriented)
        .unwrap();
    let jpeg = std::fs::read(&oriented).unwrap();
    let exif = b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut tagged = jpeg[..2].to_vec();
    tagged.extend([0xff, 0xe1]);
    tagged.extend(((exif.len() + 2) as u16).to_be_bytes());
    tagged.extend(exif);
    tagged.extend(&jpeg[2..]);
    std::fs::write(&oriented, tagged).unwrap();
    let engine = engine(&dir.path().join("workspace"));
    engine
        .import(vec![
            colored.clone(),
            sixteen.clone(),
            tiny.clone(),
            oriented.clone(),
        ])
        .unwrap();
    let data = engine.snapshot();
    assert!(
        data.media.iter().all(|m| m.error.is_none()),
        "{:?}",
        data.media
    );
    let mut tasks = vec![];
    for (i, m) in data.media.iter().enumerate() {
        let options = match i {
            0 => vec![
                image("webp", true, 100.),
                image("webp", false, 80.),
                image("png", true, 100.),
            ],
            1 => vec![image("png", true, 100.)],
            2 => vec![image("webp", false, 0.)],
            _ => vec![image("png", true, 100.)],
        };
        tasks.push((m.id.clone(), options));
    }
    engine.start(tasks).unwrap();
    wait(&engine);
    let out = engine.snapshot();
    assert!(
        out.jobs.iter().all(|j| j.errors.is_empty()),
        "{:?}",
        out.jobs
    );
    let gamma = &out.media[0].candidates;
    assert_eq!(gamma[0].diagnostics.pixel_identical, Some(true));
    let preview = image::open(engine.preview(&gamma[0].id).unwrap())
        .unwrap()
        .to_rgba8();
    assert!(
        preview.get_pixel(0, 0)[0] > 100,
        "Gamma must be converted for an sRGB preview: {:?}; {:?}; {:?}",
        preview.get_pixel(0, 0),
        out.media[0].properties,
        gamma[0].properties
    );
    assert_eq!(out.media[1].candidates[0].properties.bit_depth, Some(16));
    assert!(out.media[2].candidates[0].diagnostics.ssim_light.is_none());
    assert_eq!(out.media[3].properties.as_ref().unwrap().width, Some(8));
    assert_eq!(out.media[3].candidates[0].properties.height, Some(12));
    for m in &data.media {
        assert_eq!(hash_file(&m.path).unwrap(), m.sha256);
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires bundled helpers"]
fn export_rejects_link_traversal_and_redacts_quoted_paths() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("folder\"quoted");
    std::fs::create_dir(&folder).unwrap();
    let path = fixture(&folder);
    std::fs::write(folder.join("broken.png"), b"broken media").unwrap();
    let blocked = folder.join("unreadable");
    std::fs::create_dir(&blocked).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o0)).unwrap();
    let engine = engine(&dir.path().join("workspace"));
    let import = engine.import(vec![folder.clone()]);
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700)).unwrap();
    import.unwrap();
    if unsafe { libc::geteuid() } != 0 {
        assert!(engine
            .snapshot()
            .media
            .iter()
            .any(|m| m.name == "unreadable"
                && m.error
                    .as_deref()
                    .is_some_and(|e| e.contains("Cannot read folder"))));
    }
    let media = engine
        .snapshot()
        .media
        .into_iter()
        .find(|m| m.error.is_none())
        .unwrap();
    engine
        .start(vec![(media.id, vec![image("png", true, 100.)])])
        .unwrap();
    wait(&engine);
    let candidate = engine
        .snapshot()
        .media
        .iter()
        .flat_map(|m| &m.candidates)
        .next()
        .unwrap()
        .id
        .clone();
    let out = dir.path().join("export");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&out).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, out.join("folder\"quoted")).unwrap();
    assert!(engine
        .export(std::slice::from_ref(&candidate), &out, true)
        .is_err());
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
    std::fs::remove_file(out.join("folder\"quoted")).unwrap();
    engine.export(&[candidate], &out, true).unwrap();
    let report = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "json"))
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    let text = value.to_string();
    assert!(!text.contains(&dir.path().to_string_lossy().to_string()));
    assert!(text.contains("broken.png"));
    assert!(path.exists());
}

#[test]
#[ignore = "requires bundled helpers; verifies long mono, low-rate endpoints and Vorbis input"]
fn long_audio_endpoint_rates_and_vorbis_input() {
    fn wav(path: &Path, rate: u32, channels: u16, seconds: u32) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..rate * seconds {
            let value = if i == 0 || i == rate * seconds - 1 {
                16000
            } else {
                ((i as f64 * 0.17).sin() * 8000.) as i16
            };
            for _ in 0..channels {
                writer.write_sample(value).unwrap();
            }
        }
        writer.finalize().unwrap();
    }
    fn setting(format: &str, bitrate: Option<f64>) -> Settings {
        Settings {
            format: format.into(),
            bitrate,
            lossless: format == "flac",
            quality: None,
            vbr_quality: None,
            effort: Some(0),
            background: None,
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join("long-mono.wav");
    let short = dir.path().join("low-rate.wav");
    let stereo = dir.path().join("vorbis-source.wav");
    wav(&long, 16000, 1, 70);
    wav(&short, 8000, 1, 1);
    wav(&stereo, 44100, 2, 1);
    let vorbis = dir.path().join("generated-vorbis.ogg");
    media_engine::process::run(
        &tools().ffmpeg,
        &[
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-i".into(),
            stereo.to_string_lossy().into(),
            "-c:a".into(),
            "vorbis".into(),
            "-strict".into(),
            "experimental".into(),
            "-metadata".into(),
            "title=Generated title ✓".into(),
            "-metadata".into(),
            "track=3".into(),
            "-metadata".into(),
            "comment=Generated fixture comment".into(),
            vorbis.to_string_lossy().into(),
        ],
        None,
        &std::sync::atomic::AtomicBool::new(false),
        Some(Duration::from_secs(30)),
    )
    .unwrap();
    let hashes = [&long, &short, &vorbis].map(|p| hash_file(p).unwrap());
    let engine = engine(&dir.path().join("workspace"));
    engine
        .import(vec![long.clone(), short.clone(), vorbis.clone()])
        .unwrap();
    let data = engine.snapshot();
    assert!(
        data.media.iter().all(|m| m.error.is_none()),
        "{:?}",
        data.media
    );
    engine
        .start(vec![
            (
                data.media[0].id.clone(),
                vec![
                    setting("opus", Some(0.5)),
                    setting("opus", Some(256.)),
                    setting("flac", None),
                ],
            ),
            (
                data.media[1].id.clone(),
                vec![
                    setting("mp3", Some(8.)),
                    setting("mp3", Some(64.)),
                    setting("aac", Some(48.)),
                ],
            ),
            (data.media[2].id.clone(), vec![setting("opus", Some(96.))]),
        ])
        .unwrap();
    wait(&engine);
    let data = engine.snapshot();
    assert!(
        data.jobs.iter().all(|j| j.errors.is_empty()),
        "{:?}",
        data.jobs
    );
    assert_eq!(data.media[0].candidates.len(), 3);
    assert!(
        data.media[2].candidates[0]
            .diagnostics
            .notices
            .iter()
            .any(|n| n.contains("Preserved text tags")
                && n.contains("title")
                && n.contains("track")
                && n.contains("comment")),
        "{:?}",
        data.media[2].candidates[0].diagnostics
    );
    assert_eq!(
        data.media[0]
            .candidates
            .iter()
            .find(|c| c.settings[0].format == "flac")
            .unwrap()
            .properties
            .bit_depth,
        Some(16)
    );
    let candidates = data
        .media
        .iter()
        .flat_map(|m| &m.candidates)
        .map(|c| c.path.clone())
        .collect::<Vec<_>>();
    engine.import(candidates).unwrap();
    assert!(
        engine.snapshot().media.iter().all(|m| m.error.is_none()),
        "{:?}",
        engine.snapshot().media
    );
    for (path, expected) in [&long, &short, &vorbis].into_iter().zip(hashes) {
        assert_eq!(hash_file(path).unwrap(), expected);
    }
}
