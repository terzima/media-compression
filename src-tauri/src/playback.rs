use rodio::{DeviceSinkBuilder, Player, Source};
use serde::Serialize;
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    num::{NonZeroU16, NonZeroU32},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub id: Option<String>,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f32,
    pub error: Option<String>,
}
enum Command {
    Play(String, PathBuf, u32, u16, f64),
    Pause(bool),
    Volume(f32),
    Loop(Option<(f64, f64)>),
    Stop,
}
pub struct Playback {
    sender: mpsc::Sender<Command>,
    info: Arc<Mutex<Info>>,
}
impl Playback {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let info = Arc::new(Mutex::new(Info {
            volume: 1.,
            paused: true,
            ..Default::default()
        }));
        let state = info.clone();
        std::thread::spawn(move || {
            let mut position = Arc::new(AtomicU64::new(0));
            let loops = Arc::new(Mutex::new(None));
            let mut device = None;
            let mut player: Option<Player> = None;
            let mut shape = (48000u32, 1u16);
            while let Ok(command) = receiver
                .recv_timeout(Duration::from_millis(50))
                .or_else(|e| match e {
                    mpsc::RecvTimeoutError::Timeout => {
                        Ok(Command::Volume(state.lock().unwrap().volume))
                    }
                    _ => Err(e),
                })
            {
                match command {
                    Command::Play(id, path, rate, channels, seek) => {
                        let result = (|| -> Result<(), String> {
                            if device.is_none() {
                                device = Some(
                                    DeviceSinkBuilder::open_default_sink()
                                        .map_err(|e| format!("Audio output unavailable: {e}"))?,
                                );
                            }
                            let duration =
                                std::fs::metadata(&path).map_err(|e| e.to_string())?.len() as f64
                                    / (4. * rate as f64 * channels as f64);
                            let sample = (seek.max(0.).min(duration) * rate as f64).floor() as u64
                                * channels as u64;
                            let mut file = File::open(path).map_err(|e| e.to_string())?;
                            file.seek(SeekFrom::Start(sample * 4))
                                .map_err(|e| e.to_string())?;
                            if let Some(old) = player.take() {
                                old.stop();
                            }
                            position = Arc::new(AtomicU64::new(sample));
                            shape = (rate, channels);
                            let source = PcmSource {
                                file: BufReader::new(file),
                                rate,
                                channels,
                                position: position.clone(),
                                loops: loops.clone(),
                                duration,
                            };
                            let new = Player::connect_new(device.as_ref().unwrap().mixer());
                            new.set_volume(state.lock().unwrap().volume);
                            new.append(source);
                            new.play();
                            player = Some(new);
                            let volume = state.lock().unwrap().volume;
                            *state.lock().unwrap() = Info {
                                id: Some(id),
                                position: seek,
                                duration,
                                paused: false,
                                volume,
                                error: None,
                            };
                            Ok(())
                        })();
                        if let Err(error) = result {
                            state.lock().unwrap().error = Some(error);
                        }
                    }
                    Command::Pause(paused) => {
                        if let Some(p) = &player {
                            if paused {
                                p.pause()
                            } else {
                                p.play()
                            };
                        }
                        state.lock().unwrap().paused = paused;
                    }
                    Command::Volume(volume) => {
                        if let Some(p) = &player {
                            p.set_volume(volume);
                        }
                        state.lock().unwrap().volume = volume;
                    }
                    Command::Loop(value) => {
                        *loops.lock().unwrap() = value;
                    }
                    Command::Stop => {
                        position.store(0, Ordering::Relaxed);
                        *loops.lock().unwrap() = None;
                        if let Some(p) = player.take() {
                            p.stop();
                        }
                        let volume = state.lock().unwrap().volume;
                        *state.lock().unwrap() = Info {
                            volume,
                            paused: true,
                            ..Default::default()
                        };
                    }
                }
                let mut info = state.lock().unwrap();
                info.position = if info.id.is_none() {
                    0.
                } else {
                    position.load(Ordering::Relaxed) as f64 / (shape.0 as f64 * shape.1 as f64)
                };
                if player.as_ref().is_some_and(|p| p.empty()) {
                    info.paused = true;
                }
            }
        });
        Self { sender, info }
    }
    pub fn play(&self, id: String, path: PathBuf, rate: u32, channels: u16, position: f64) {
        let _ = self
            .sender
            .send(Command::Play(id, path, rate, channels, position));
    }
    pub fn control(
        &self,
        action: &str,
        value: Option<f64>,
        end: Option<f64>,
    ) -> Result<(), String> {
        if value.is_some_and(|v| !v.is_finite()) || end.is_some_and(|v| !v.is_finite()) {
            return Err("Playback values must be finite".into());
        }
        let command = match action {
            "pause" => Command::Pause(true),
            "resume" => Command::Pause(false),
            "stop" => Command::Stop,
            "volume" => Command::Volume(value.unwrap_or(1.).clamp(0., 1.) as f32),
            "loop" => Command::Loop(match (value, end) {
                (Some(a), Some(b)) if a >= 0. && b > a => Some((a, b)),
                _ => None,
            }),
            _ => return Err("Unknown playback action".into()),
        };
        self.sender.send(command).map_err(|e| e.to_string())
    }
    pub fn info(&self) -> Info {
        self.info.lock().unwrap().clone()
    }
}
struct PcmSource {
    file: BufReader<File>,
    rate: u32,
    channels: u16,
    position: Arc<AtomicU64>,
    loops: Arc<Mutex<Option<(f64, f64)>>>,
    duration: f64,
}
impl Iterator for PcmSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some((start, end)) = *self.loops.lock().ok()? {
            let current = self.position.load(Ordering::Relaxed);
            if current >= ((end * self.rate as f64).floor() as u64 * self.channels as u64) {
                let sample = (start * self.rate as f64).floor() as u64 * self.channels as u64;
                self.file.seek(SeekFrom::Start(sample * 4)).ok()?;
                self.position.store(sample, Ordering::Relaxed);
            }
        }
        let mut bytes = [0; 4];
        self.file.read_exact(&mut bytes).ok()?;
        self.position.fetch_add(1, Ordering::Relaxed);
        Some(f32::from_le_bytes(bytes))
    }
}
impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> NonZeroU16 {
        NonZeroU16::new(self.channels).unwrap()
    }
    fn sample_rate(&self) -> NonZeroU32 {
        NonZeroU32::new(self.rate).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.duration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_seek_and_loop_keep_channel_alignment() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        for value in [1f32, 2., 3., 4., 5., 6.] {
            file.write_all(&value.to_le_bytes()).unwrap();
        }
        file.flush().unwrap();
        let position = Arc::new(AtomicU64::new(0));
        let loops = Arc::new(Mutex::new(Some((0., 1.))));
        let mut source = PcmSource {
            file: BufReader::new(File::open(file.path()).unwrap()),
            rate: 2,
            channels: 2,
            position: position.clone(),
            loops,
            duration: 1.5,
        };
        assert_eq!(source.next(), Some(1.));
        assert_eq!(source.next(), Some(2.));
        assert_eq!(source.next(), Some(3.));
        assert_eq!(source.next(), Some(4.));
        assert_eq!(source.next(), Some(1.));
        assert_eq!(position.load(Ordering::Relaxed), 1);
    }
    #[test]
    #[ignore = "requires an actual audio output device; run on a test installation"]
    fn native_playback_device_switch_pause_loop_volume() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        for i in 0..44100 * 2 * 3 {
            file.write_all(&((i as f32 * 0.0627).sin() * 0.1).to_le_bytes())
                .unwrap();
        }
        file.flush().unwrap();
        let playback = Playback::new();
        playback.control("volume", Some(0.), None).unwrap();
        playback.play("original".into(), file.path().into(), 44100, 2, 0.);
        let start = std::time::Instant::now();
        loop {
            let info = playback.info();
            assert!(info.error.is_none(), "{:?}", info.error);
            if info.position > 0.1 {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(20));
        }
        playback.control("pause", None, None).unwrap();
        std::thread::sleep(Duration::from_millis(80));
        assert!(playback.info().paused);
        let position = playback.info().position;
        playback.play("candidate".into(), file.path().into(), 44100, 2, position);
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(playback.info().id.as_deref(), Some("candidate"));
        assert!(playback.info().position >= position);
        assert_eq!(playback.info().volume, 0.);
        playback.control("loop", Some(0.), Some(0.25)).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(playback.info().position < 0.30);
        playback.control("stop", None, None).unwrap();
        std::thread::sleep(Duration::from_millis(80));
        assert!(playback.info().id.is_none());
        assert_eq!(playback.info().position, 0.);
    }
}
