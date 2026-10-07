use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub format: String,
    pub lossless: bool,
    pub quality: Option<f64>,
    pub bitrate: Option<u32>,
    pub vbr_quality: Option<f64>,
    pub effort: Option<u8>,
    pub background: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Properties {
    pub kind: String,
    pub format: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub alpha: bool,
    pub bit_depth: Option<u8>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub duration: Option<f64>,
    pub sample_format: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub ssim_light: Option<f64>,
    pub ssim_dark: Option<f64>,
    pub alpha_max_error: Option<u8>,
    pub alpha_mean_error: Option<f64>,
    pub pixel_identical: Option<bool>,
    pub duration_delta: Option<f64>,
    pub notices: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub media_id: String,
    pub bytes: u64,
    pub sha256: String,
    pub settings: Vec<Settings>,
    pub properties: Properties,
    pub diagnostics: Diagnostics,
    pub preview: Option<String>,
    pub exported: Vec<String>,
    #[serde(skip)]
    pub path: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub id: String,
    pub name: String,
    pub relative_name: String,
    pub bytes: u64,
    pub sha256: String,
    pub properties: Option<Properties>,
    pub preview: Option<String>,
    pub error: Option<String>,
    pub candidates: Vec<Candidate>,
    #[serde(skip)]
    pub path: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub media_id: String,
    pub state: String,
    pub stage: String,
    pub completed: usize,
    pub total: usize,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub media: Vec<Media>,
    pub jobs: Vec<Job>,
    pub tools: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tools {
    pub worker: PathBuf,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub pngquant: PathBuf,
    pub cwebp: PathBuf,
    pub cjpeg: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageRequest {
    pub action: String,
    pub input: PathBuf,
    pub output: PathBuf,
    pub settings: Option<Settings>,
    pub tools: Tools,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageResult {
    pub properties: Properties,
    pub diagnostics: Diagnostics,
}
