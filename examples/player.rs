// Vorbis decoder written in Rust
//
// This example file is licensed
// under the CC-0 license:
// https://creativecommons.org/publicdomain/zero/1.0/

extern crate alto;
extern crate lewton;
extern crate byteorder;

use std::env;
use lewton::VorbisError;
use lewton::inside_ogg::OggStreamReader;
use std::fs::File;
use std::thread::sleep;
use std::time::{Instant, Duration};
use alto::{Alto, Mono, Stereo, Source};

fn main() {
	match run() {
		Ok(_) =>(),
		Err(err) => println!("Error: {}", err),
	}
}

fn run() -> Result<(), VorbisError> {
	let file_path = env::args().nth(1).expect("No arg found. Please specify a file to open.");
	println!("Opening file: {}", file_path);
	let f = File::open(file_path).expect("Can't open file");

	// Prepare the reading
	let mut srr = try!(OggStreamReader::new(f));

	// Prepare the playback.
	let al = Alto::load_default().expect("Could not load alto");
	let device = al.open(None).expect("Could not open device");
	let cxt = device.new_context(None).expect("Could not create context");
	let mut str_src = cxt.new_streaming_source()
		.expect("could not create streaming src");
	let sample_rate = srr.ident_hdr.audio_sample_rate as i32;

	if srr.ident_hdr.audio_channels > 2 {
		// the openal crate can't process these many channels directly
		println!("Stream error: {} channels are too many!", srr.ident_hdr.audio_channels);
	}

	println!("Sample rate: {}", srr.ident_hdr.audio_sample_rate);

	// Now the fun starts..
	let mut n = 0;
	let mut len_play = 0.0;
	let mut start_play_time = None;
	let start_decode_time = Instant::now();
	let sample_channels = srr.ident_hdr.audio_channels as f32 *
		srr.ident_hdr.audio_sample_rate as f32;
	while let Some(pck_samples) = try!(srr.read_dec_packet_itl()) {
		println!("Decoded packet no {}, with {} samples.", n, pck_samples.len());
		n += 1;
		let buf = match srr.ident_hdr.audio_channels {
			1 => cxt.new_buffer::<Mono<i16>,_>(&pck_samples, sample_rate),
			2 => cxt.new_buffer::<Stereo<i16>,_>(&pck_samples, sample_rate),
			n => panic!("unsupported number of channels: {}", n),
		}.unwrap();

		str_src.queue_buffer(buf).unwrap();

		len_play += pck_samples.len() as f32 / sample_channels;
		// If we are faster than realtime, we can already start playing now.
		if n == 100 {
			let cur = Instant::now();
			if cur - start_decode_time < Duration::from_millis((len_play * 1000.0) as u64) {
				start_play_time = Some(cur);
				str_src.play();
			}
		}
	}
	let total_duration = Duration::from_millis((len_play * 1000.0) as u64);
	let sleep_duration = total_duration - match start_play_time {
			None => {
				str_src.play();
				Duration::from_millis(0)
			},
			Some(t) => (Instant::now() - t)
		};
	println!("The piece is {} s long.", len_play);
	sleep(sleep_duration);

	Ok(())
}
