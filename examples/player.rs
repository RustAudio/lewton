// Vorbis decoder written in Rust
//
// This example file is licensed
// under the CC-0 license:
// https://creativecommons.org/publicdomain/zero/1.0/

extern crate openal;
extern crate lewton;
extern crate byteorder;

use std::env;
use lewton::VorbisError;
use lewton::inside_ogg::OggStreamReader;
use std::fs::File;
use std::thread::sleep;
use std::time::{Instant, Duration};
use openal::al;
use openal::alc;

fn main() {
	match run() {
		Ok(_) =>(),
		Err(err) => println!("Error: {}", err),
	}
}

fn run() -> Result<(), VorbisError> {
	let file_path = env::args().nth(1).expect("No arg found. Please specify a file to open.");
	println!("Opening file: {}", file_path);
	let f = try!(File::open(file_path));

	// Prepare the reading
	let mut srr = try!(OggStreamReader::new(f));

	// Prepare the playback.
	let device = alc::Device::open(None).expect("Could not open device");
	let ctx = device.create_context(&[]).expect("Could not create context");
	ctx.make_current();
	let source = al::Source::gen();
	let sample_rate = srr.ident_hdr.audio_sample_rate as al::ALsizei;

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
	while let Some(pck_samples) = try!(srr.read_dec_packet_itl()) {
		println!("Decoded packet no {}, with {} samples.", n, pck_samples.len());
		n += 1;
		let buffer = al::Buffer::gen();
		let format = if srr.ident_hdr.audio_channels == 1 {
			al::Format::Mono16
		} else {
			al::Format::Stereo16
		};
		unsafe {
			buffer.buffer_data(format, &pck_samples, sample_rate)
		}
		source.queue_buffer(&buffer);
		len_play += pck_samples.len() as f32 / srr.ident_hdr.audio_sample_rate as f32;
		// If we are faster than realtime, we can already start playing now.
		if n == 100 {
			let cur = Instant::now();
			if cur - start_decode_time < Duration::from_millis((len_play * 1000.0) as u64) {
				start_play_time = Some(cur);
				source.play();
			}
		}
	}
	let total_duration = Duration::from_millis((len_play * 1000.0) as u64);
	let sleep_duration = total_duration - match start_play_time {
			None => {
				source.play();
				Duration::from_millis(0)
			},
			Some(t) => (Instant::now() - t)
		};
	println!("The piece is {} s long.", len_play);
	sleep(sleep_duration);

	ctx.destroy();
	device.close().ok().expect("Unable to close device");
	Ok(())
}
