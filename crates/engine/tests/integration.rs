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
    let dir = root.join("src-tauri/resources/codecs");
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
        worker: root.join(format!("src-tauri/binaries/media-worker-{target}{ext}")),
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
        ("aac", Some(128), None),
        ("mp3", None, Some(4.)),
        ("opus", Some(96), None),
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
