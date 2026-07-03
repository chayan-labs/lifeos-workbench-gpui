//! Manual live-verification tool for issue #89 - NOT part of the automated
//! test suite (real model + real audio, matches the project's "heavy
//! external dependency verified live manually" boundary). Usage:
//!   cargo run -p lifeos-ingest --example transcribe_live -- <model.bin> <clip.wav>

use lifeos_ingest::{Transcriber, WhisperTranscriber};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let model_path = args.next().expect("usage: transcribe_live <model.bin> <clip.wav>");
    let wav_path = args.next().expect("usage: transcribe_live <model.bin> <clip.wav>");

    let bytes = std::fs::read(&wav_path).expect("read wav file");
    let samples = lifeos_ingest::audio::decode_to_16k_mono_f32(&bytes).expect("decode audio");
    println!("decoded {} samples ({:.2}s at 16kHz)", samples.len(), samples.len() as f64 / 16_000.0);

    let transcriber = WhisperTranscriber { model_path };
    let segments = transcriber.transcribe(&samples).await.expect("transcribe");

    for seg in &segments {
        println!("[{:.2}s -> {:.2}s] {}", seg.t_start_secs, seg.t_end_secs, seg.text);
    }
}
