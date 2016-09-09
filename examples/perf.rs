// Vorbis decoder written in Rust
//
// This example file is licensed
// under the CC-0 license:
// https://creativecommons.org/publicdomain/zero/1.0/

extern crate lewton;
extern crate byteorder;
extern crate ogg;


fn main() {
	match run() {
		Ok(_) =>(),
		Err(err) => println!("Error: {}", err),
	}
}

use std::env;
use lewton::VorbisError;
use ogg::{PacketReader, OggReadError};
use lewton::inside_ogg::OggStreamReader;
use std::fs::File;
use std::time::Instant;

pub fn run() -> Result<(), VorbisError> {
	let file_path = env::args().nth(1).expect("No arg found. Please specify a file to open.");
	println!("Opening file: {}", file_path);
	let mut f = try!(File::open(file_path));
	let mut pck_rdr = PacketReader::new(&mut f);

	let mut srr = try!(OggStreamReader::new(&mut pck_rdr));

	println!("Sample rate: {}", srr.ident_hdr.audio_sample_rate);

	// Now the fun starts..
	let mut n = 0;
	let mut len_play = 0.0;
	let start_decode_time = Instant::now();
	loop {
		use std::io::ErrorKind;
		let pck = match srr.read_decompressed_packet() {
			Ok(p) => p,
			Err(VorbisError::OggError(OggReadError::ReadError(ref e)))
				if e.kind() == ErrorKind::UnexpectedEof => {
				println!("Seems to be the end."); break; },
			Err(VorbisError::OggError(OggReadError::ReadError(ref e)))
				if e.kind() == ErrorKind::WouldBlock => continue,
			Err(e) => {
				panic!("OGG stream decode failure: {}", e);
			},
		};
		n += 1;
		// This is guaranteed by the docs
		assert_eq!(pck.0.len(), srr.ident_hdr.audio_channels as usize);
		len_play += pck.0[0].len() as f32 / srr.ident_hdr.audio_sample_rate as f32;
	}
	let decode_duration = Instant::now() - start_decode_time;
	println!("The piece is {} s long ({} packets).", len_play, n);
	println!("Decoded in {} s.", decode_duration.as_secs() as f64 + (decode_duration.subsec_nanos() as f64) / 1_000_000_000.0);

	Ok(())
}
