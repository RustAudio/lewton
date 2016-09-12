// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate lewton;
extern crate time;
extern crate ogg;
extern crate vorbis;

use std::env;
use std::fs::File;

use ogg::PacketReader;
use lewton::VorbisError;
use lewton::inside_ogg::*;
use std::time::{Duration, Instant};

use vorbis::Decoder as NativeDecoder;

fn main() {
	let command_name = env::args().nth(1).expect("No command found.");
	match command_name.as_ref() {
		"vals" =>  run_vals(), // Comparison of the output
		"perf" =>  run_perf(), // Comparison of speed
		_ => println!("Error: invalid command.\n\
		Usage: <Command> <Filename>. Valid commands are \
		'vals' for output comparison and 'perf' for speed comparison."),
	}
}

fn run_perf() {
	macro_rules! try {
		($expr:expr) => (match $expr {
			$crate::std::result::Result::Ok(val) => val,
			$crate::std::result::Result::Err(err) => {
				panic!("Error: {:?}", err)
			}
		})
	}
	let file_path = env::args().nth(2).expect("Please specify a file to open via arg.");
	println!("Opening file: {}", file_path);

	let mut n_native = 0;

	let f_n = try!(File::open(file_path.clone()));
	let dec = try!(NativeDecoder::new(f_n));
	let mut native_it = dec.into_packets();
	let start_native_decode = Instant::now();

	loop {
		n_native += 1;
		try!(match native_it.next() {
			Some(v) => v,
			None => { break }
		});
	}
	let native_decode_duration = Instant::now() - start_native_decode;

	println!("Time to decode {} packets with libvorbis: {}",
		n_native, get_duration_seconds(&native_decode_duration));

	let mut n = 0;
	let mut f_r = try!(File::open(file_path));
	let mut pck_rdr = PacketReader::new(&mut f_r);
	let mut ogg_rdr :OggStreamReader<_> = try!(OggStreamReader::new(&mut pck_rdr));

	let start_decode = Instant::now();

	// Reading and discarding the first empty packet
	// The native decoder does this itself.
	try!(ogg_rdr.read_decompressed_packet());

	println!("Sample rate: {}", ogg_rdr.ident_hdr.audio_sample_rate);

	loop {
		n += 1;
		use std::io::ErrorKind;
		use ogg::OggReadError;
		match ogg_rdr.read_decompressed_packet() {
			Ok(p) => p,
			Err(VorbisError::OggError(OggReadError::ReadError(ref e)))
				if e.kind() == ErrorKind::UnexpectedEof => {
				println!("Seems to be the end."); break; },
			Err(e) => {
				panic!("OGG stream decode failure: {}", e);
			},
		};
	}
	let decode_duration = Instant::now() - start_decode;

	println!("Time to decode {} packets with lewton: {}",
		n, get_duration_seconds(&decode_duration));
}

fn get_duration_seconds(dur :&Duration) -> f64 {
	return dur.as_secs() as f64 + (dur.subsec_nanos() as f64) / 1_000_000_000.0;
}

fn run_vals() {
	macro_rules! try {
		($expr:expr) => (match $expr {
			$crate::std::result::Result::Ok(val) => val,
			$crate::std::result::Result::Err(err) => {
				panic!("Error: {:?}", err)
			}
		})
	}
	let file_path = env::args().nth(2).expect("Please specify a file to open via arg.");
	println!("Opening file: {}", file_path);
	let     f_n = try!(File::open(file_path.clone()));
	let mut f_r = try!(File::open(file_path));

	let dec = try!(NativeDecoder::new(f_n));

	let mut pck_rdr = PacketReader::new(&mut f_r);
	let mut ogg_rdr :OggStreamReader<_> = try!(OggStreamReader::new(&mut pck_rdr));

	if ogg_rdr.ident_hdr.audio_channels > 2 {
		// We haven't implemented interleave code for more than two channels
		println!("Stream error: {} channels are too many!",
			ogg_rdr.ident_hdr.audio_channels);
	}
	println!("Sample rate: {}", ogg_rdr.ident_hdr.audio_sample_rate);

	// Now the fun starts..
	let mut native_it = dec.into_packets();
	let mut n = 0;

	// Reading and discarding the first empty packet
	// The native decoder does this itself.
	try!(ogg_rdr.read_decompressed_packet());

	let mut pcks_with_diffs = 0;

	loop {
		n += 1;
		let native_decoded = try!(match native_it.next() { Some(v) => v,
			None => { break }});
		let (pck_decompressed, _) = try!(ogg_rdr.read_decompressed_packet());

		// Asserting some very basic things:
		assert_eq!(native_decoded.rate, ogg_rdr.ident_hdr.audio_sample_rate as u64);
		assert_eq!(native_decoded.channels, ogg_rdr.ident_hdr.audio_channels as u16);

		let decompressed_len = pck_decompressed.iter().fold(0, |s, e| s + e.len());

		let mut samples :Vec<i16> = Vec::with_capacity(pck_decompressed[0].len() * ogg_rdr.ident_hdr.audio_channels as usize);

		let dc_iter = if ogg_rdr.ident_hdr.audio_channels == 1 {
			pck_decompressed[0].iter()
		} else {
			// Fill samples with stuff
			for (ls, rs) in pck_decompressed[0].iter().zip(pck_decompressed[1].iter()) {
				samples.push(*ls);
				samples.push(*rs);
			}
			samples.iter()
		};
		let mut diffs = 0;
		for (s,n) in dc_iter.zip(native_decoded.data.iter()) {
			let diff = s - n;
			// +- 1 deviation is allowed.
			if diff.abs() > 1 {
				diffs += 1;
			}
		}
		if diffs > 0 || decompressed_len != native_decoded.data.len() {
			/*
			print!("Differences found in packet no {}... ", n);
			print!("{} {}", decompressed_len, native_decoded.data.len());
			println!(" (diffs {})", diffs);
			*/
			pcks_with_diffs += 1;
		}
	}
	if pcks_with_diffs > 0 {
		println!("Total number of packets with differences: {} of {} ({}%)",
			pcks_with_diffs, n, pcks_with_diffs as f32 / n as f32 * 100.0);
	} else {
		println!("No differences found.");
	}
}
