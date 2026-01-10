//! File transcription commands - transcribe audio files to text
//!
//! Supports common audio formats: wav, mp3, m4a, ogg, flac, webm
//! Uses the same transcription infrastructure as live recording.

use crate::audio_toolkit::apply_custom_words;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, TranscriptionProvider};
use log::{debug, error, info};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Result of a file transcription operation
#[derive(Serialize, Type)]
pub struct FileTranscriptionResult {
    /// The transcribed text
    pub text: String,
    /// Path where the text file was saved (if save_to_file was true)
    pub saved_file_path: Option<String>,
}

/// Supported audio file extensions
const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "ogg", "flac", "webm"];

/// Get the list of supported audio file extensions
#[tauri::command]
#[specta::specta]
pub fn get_supported_audio_extensions() -> Vec<String> {
    SUPPORTED_EXTENSIONS.iter().map(|s| s.to_string()).collect()
}

/// Transcribe an audio file to text
///
/// # Arguments
/// * `file_path` - Path to the audio file
/// * `profile_id` - Optional transcription profile ID (uses active profile if not specified)
/// * `save_to_file` - If true, saves the transcription to a .txt file in Documents folder
///
/// # Returns
/// FileTranscriptionResult with the transcribed text and optional saved file path
#[tauri::command]
#[specta::specta]
pub async fn transcribe_audio_file(
    app: AppHandle,
    file_path: String,
    profile_id: Option<String>,
    save_to_file: bool,
) -> Result<FileTranscriptionResult, String> {
    let path = PathBuf::from(&file_path);

    // Validate file exists
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Validate extension
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "Unsupported audio format: .{}. Supported formats: {}",
            extension,
            SUPPORTED_EXTENSIONS.join(", ")
        ));
    }

    info!("Transcribing audio file: {}", file_path);

    // Read and decode the audio file to PCM samples
    let samples = decode_audio_file(&path).map_err(|e| {
        error!("Failed to decode audio file: {}", e);
        format!("Failed to decode audio file: {}", e)
    })?;

    if samples.is_empty() {
        return Err("Audio file contains no audio data".to_string());
    }

    debug!("Decoded {} samples from audio file", samples.len());

    // Get settings and determine profile to use
    let settings = get_settings(&app);
    let profile_id = profile_id.unwrap_or_else(|| settings.active_profile_id.clone());
    let profile = settings.transcription_profile(&profile_id);

    // Perform transcription
    let transcription_text =
        if settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible {
            // Remote STT
            let remote_manager = app.state::<Arc<RemoteSttManager>>();

            let prompt = profile
                .as_ref()
                .map(|p| p.system_prompt.clone())
                .filter(|p| !p.trim().is_empty())
                .or_else(|| {
                    settings
                        .transcription_prompts
                        .get(&settings.remote_stt.model_id)
                        .filter(|p| !p.trim().is_empty())
                        .cloned()
                });

            let text = remote_manager
                .transcribe(&settings.remote_stt, &samples, prompt)
                .await
                .map_err(|e| format!("Remote transcription failed: {}", e))?;

            // Apply custom word corrections
            if settings.custom_words.is_empty() {
                text
            } else {
                apply_custom_words(
                    &text,
                    &settings.custom_words,
                    settings.word_correction_threshold,
                )
            }
        } else {
            // Local transcription
            let tm = app.state::<Arc<TranscriptionManager>>();

            // Ensure model is loaded before transcription
            tm.initiate_model_load();

            if let Some(p) = &profile {
                tm.transcribe_with_overrides(
                    samples,
                    Some(&p.language),
                    Some(p.translate_to_english),
                    if p.system_prompt.trim().is_empty() {
                        None
                    } else {
                        Some(p.system_prompt.clone())
                    },
                )
                .map_err(|e| format!("Local transcription failed: {}", e))?
            } else {
                tm.transcribe(samples)
                    .map_err(|e| format!("Local transcription failed: {}", e))?
            }
        };

    info!(
        "Transcription completed: {} characters",
        transcription_text.len()
    );

    // Save to file if requested
    let saved_file_path = if save_to_file {
        let output_path = get_output_file_path(&path)?;
        std::fs::write(&output_path, &transcription_text)
            .map_err(|e| format!("Failed to save transcription: {}", e))?;
        info!("Saved transcription to: {}", output_path.display());
        Some(output_path.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(FileTranscriptionResult {
        text: transcription_text,
        saved_file_path,
    })
}

/// Decode an audio file to f32 PCM samples at 16kHz
fn decode_audio_file(path: &PathBuf) -> Result<Vec<f32>, String> {
    use rodio::Source;
    use std::fs::File;
    use std::io::BufReader; // Import trait for sample_rate() and channels()

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // For WAV files, use hound for direct reading
    if extension == "wav" {
        return decode_wav_file(path);
    }

    // For other formats, use rodio's decoder
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);

    let source =
        rodio::Decoder::new(reader).map_err(|e| format!("Failed to decode audio: {}", e))?;

    // Get source sample rate and channels
    let sample_rate = source.sample_rate();
    let channels = source.channels() as usize;

    debug!("Audio file: {} Hz, {} channels", sample_rate, channels);

    // Collect all samples as f32 (rodio decoder outputs f32)
    let samples: Vec<f32> = source.collect();

    // Convert to mono if stereo
    let mono_samples: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if necessary
    let target_sample_rate = 16000;
    let resampled = if sample_rate != target_sample_rate {
        resample_audio(&mono_samples, sample_rate, target_sample_rate)?
    } else {
        mono_samples
    };

    Ok(resampled)
}

/// Decode a WAV file directly using hound
fn decode_wav_file(path: &PathBuf) -> Result<Vec<f32>, String> {
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Failed to open WAV file: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    debug!(
        "WAV file: {} Hz, {} channels, {} bits",
        sample_rate, channels, spec.bits_per_sample
    );

    // Read samples based on format
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            // Use i64 for the shift to avoid overflow with 32-bit samples
            let max_val = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(Result::ok)
            .collect(),
    };

    // Convert to mono if stereo
    let mono_samples: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if necessary
    let target_sample_rate = 16000;
    let resampled = if sample_rate != target_sample_rate {
        resample_audio(&mono_samples, sample_rate, target_sample_rate)?
    } else {
        mono_samples
    };

    Ok(resampled)
}

/// Resample audio from one sample rate to another
fn resample_audio(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, String> {
    use rubato::{FftFixedIn, Resampler};

    // Use a reasonable chunk size
    let chunk_size = 1024.min(samples.len());
    if chunk_size == 0 {
        return Ok(Vec::new());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        1, // sub_chunks
        1, // channels
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let mut output = Vec::new();

    // Process in chunks
    for chunk in samples.chunks(chunk_size) {
        // Pad last chunk if needed
        let mut input_chunk = chunk.to_vec();
        if input_chunk.len() < chunk_size {
            input_chunk.resize(chunk_size, 0.0);
        }

        let result = resampler
            .process(&[input_chunk], None)
            .map_err(|e| format!("Failed to resample audio: {}", e))?;

        if let Some(out_chunk) = result.first() {
            output.extend_from_slice(out_chunk);
        }
    }

    Ok(output)
}

/// Get the output file path for saving transcription
/// Saves to Documents folder with same name as audio file but .txt extension
fn get_output_file_path(audio_path: &PathBuf) -> Result<PathBuf, String> {
    // Get Documents folder
    let documents_dir =
        dirs::document_dir().ok_or_else(|| "Could not find Documents folder".to_string())?;

    // Create output filename from audio filename
    let stem = audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("transcription");

    let output_path = documents_dir.join(format!("{}.txt", stem));

    Ok(output_path)
}
