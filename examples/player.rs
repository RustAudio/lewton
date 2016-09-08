// Vorbis decoder written in Rust
//
// This example file is licensed
// under the CC-0 license:
// https://creativecommons.org/publicdomain/zero/1.0/

extern crate openal;
extern crate lewton;
extern crate byteorder;
extern crate ogg;

use std::env;
use lewton::VorbisError;
use lewton::audio;
use ogg::PacketReader;
use lewton::inside_ogg::read_headers;
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
	let mut f = try!(File::open(file_path));
	let mut pck_rdr = PacketReader::new(&mut f);

	let (ident_hdr, _, setup_hdr) = try!(read_headers(&mut pck_rdr));

	let device = alc::Device::open(None).expect("Could not open device");
	let ctx = device.create_context(&[]).expect("Could not create context");
	ctx.make_current();

	let source = al::Source::gen();

	let sample_rate = ident_hdr.audio_sample_rate as al::ALsizei;

	if ident_hdr.audio_channels > 2 {
		// the openal crate can't process these many channels directly
		println!("Stream error: {} channels are too many!", ident_hdr.audio_channels);
	}
	println!("Sample rate: {}", ident_hdr.audio_sample_rate);

	// Now the fun starts..
	let mut n = 0;
	let mut pwr = audio::PreviousWindowRight::new();
	let mut len_play = 0.0;
	// For development you might want to set it to false
	let do_playing = true;
	let mut start_play_time = None;
	let start_decode_time = Instant::now();
	loop {
		print!("Reading packet no {}... ", n);
		n += 1;
		let pck_compressed = match pck_rdr.read_packet() {
			Ok(p) => p,
			Err(ogg::OggReadError::ReadError(e)) => {
				use std::io::ErrorKind;
				if e.kind() == ErrorKind::UnexpectedEof {
					println!("Seems to be the end.");
					break;
				} else {
					panic!("OGG stream decode failure: {}", e);
				}
			},
			Err(e) => {
				panic!("OGG stream decode failure: {}", e);
			},
		};
		print!("({} bytes compressed)", pck_compressed.data.len());
		let pck_decompressed = audio::read_audio_packet(&ident_hdr,
			&setup_hdr, &pck_compressed.data, &mut pwr);
		let pck_data = pck_decompressed.expect("vorbis packet decode failure");
		let unc_dim :Vec<usize> = pck_data.iter().map(|l| l.len()).collect();
		println!(" (with uncompressed dimensions {:?})", unc_dim);
		//println!("(and {} bytes uncompressed)", pck_data.len)
		if pck_data.len() == 0  {
			println!("Skipping packet with no channels from playback.");
			println!("This is expected for the first packet, but for any");
			println!("packet after this, this is an error.");
			continue;
		}
		// This is guaranteed by the docs
		assert_eq!(pck_data.len(), ident_hdr.audio_channels as usize);
		let buffer = al::Buffer::gen();
		if ident_hdr.audio_channels == 1 {
			unsafe {
			buffer.buffer_data(al::Format::Mono16, &pck_data[0], sample_rate) };
		} else {
			let mut samples :Vec<i16> = Vec::with_capacity(pck_data[0].len() * ident_hdr.audio_channels as usize);
			// Fill samples with stuff
			for (ls, rs) in pck_data[0].iter().zip(pck_data[1].iter()) {
				samples.push(*ls);
				samples.push(*rs);
			}
			unsafe {
			buffer.buffer_data(al::Format::Stereo16, &samples, sample_rate) };
		}
		source.queue_buffer(&buffer);
		len_play += pck_data[0].len() as f32 / ident_hdr.audio_sample_rate as f32;
		if n == 100 {
			let cur = Instant::now();
			if cur - start_decode_time < Duration::from_millis((len_play * 1000.0) as u64) {
				if do_playing {
					start_play_time = Some(cur);
					source.play();
				}
			}
		}
	}
	let total_duration = Duration::from_millis((len_play * 1000.0) as u64);
	let sleep_duration = total_duration - match start_play_time {
			None => {
				if do_playing {
					source.play();
				}
				Duration::from_millis(0)
			},
			Some(t) => (Instant::now() - t)
		};
	println!("The piece is {} s long.", len_play);
	if do_playing {
		sleep(sleep_duration);
	}

	ctx.destroy();
	device.close().ok().expect("Unable to close device");
	Ok(())

}
