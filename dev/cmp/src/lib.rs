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
extern crate test_assets;

use std::fs::File;

use ogg::PacketReader;
use lewton::inside_ogg::*;
use std::time::{Duration, Instant};

use vorbis::Decoder as NativeDecoder;

pub fn cmp_perf(file_path :&str) -> (Duration, Duration, usize) {
	macro_rules! try {
		($expr:expr) => (match $expr {
			$crate::std::result::Result::Ok(val) => val,
			$crate::std::result::Result::Err(err) => {
				panic!("Error: {:?}", err)
			}
		})
	}

	let f_n = try!(File::open(file_path.clone()));
	let dec = try!(NativeDecoder::new(f_n));
	let mut native_it = dec.into_packets();
	let start_native_decode = Instant::now();

	loop {
		try!(match native_it.next() {
			Some(v) => v,
			None => break,
		});
	}
	let native_decode_duration = Instant::now() - start_native_decode;

	let mut n = 0;
	let f_r = try!(File::open(file_path));
	let mut ogg_rdr = try!(OggStreamReader::new(PacketReader::new(f_r)));

	let start_decode = Instant::now();

	// Reading and discarding the first empty packet
	// The native decoder does this itself.
	try!(ogg_rdr.read_dec_packet());

	while let Some(_) = try!(ogg_rdr.read_dec_packet()) {
		n += 1;
	}
	let decode_duration = Instant::now() - start_decode;
	return (decode_duration, native_decode_duration, n);
}

pub fn get_duration_seconds(dur :&Duration) -> f64 {
	return dur.as_secs() as f64 + (dur.subsec_nanos() as f64) / 1_000_000_000.0;
}

pub fn cmp_output(file_path :&str) -> (usize, usize) {
	macro_rules! try {
		($expr:expr) => (match $expr {
			$crate::std::result::Result::Ok(val) => val,
			$crate::std::result::Result::Err(err) => {
				panic!("Error: {:?}", err)
			}
		})
	}
	let f_n = try!(File::open(file_path.clone()));
	let f_r = try!(File::open(file_path));

	let dec = try!(NativeDecoder::new(f_n));

	let mut ogg_rdr = try!(OggStreamReader::new(PacketReader::new(f_r)));

	if ogg_rdr.ident_hdr.audio_channels > 2 {
		// We haven't implemented interleave code for more than two channels
		println!("Stream error: {} channels are too many!",
			ogg_rdr.ident_hdr.audio_channels);
	}

	// Now the fun starts..
	let mut native_it = dec.into_packets();
	let mut n = 0;

	// Reading and discarding the first empty packet
	// The native decoder does this itself.
	try!(ogg_rdr.read_dec_packet());

	let mut pcks_with_diffs = 0;

	// This parameter is useful when we only want to check whether the
	// actually returned data are the same, regardless of where the
	// two implementations put packet borders.
	// Of course, when debugging bugs which modify the size of packets
	// you usually want to set this flag to false so that you don't
	// suffer from the "carry over" effect of errors.
	let ignore_packet_borders :bool = true;

	let mut native_dec_data = Vec::new();
	let mut dec_data = Vec::new();
	loop {
		n += 1;

		let mut native_decoded = try!(match native_it.next() { Some(v) => v,
			None => break,});
		native_dec_data.append(&mut native_decoded.data);
		let mut pck_decompressed = match try!(ogg_rdr.read_dec_packet_itl()) {
			Some(v) => v,
			None => break, // TODO tell calling code about this condition
		};

		// Asserting some very basic things:
		assert_eq!(native_decoded.rate, ogg_rdr.ident_hdr.audio_sample_rate as u64);
		assert_eq!(native_decoded.channels, ogg_rdr.ident_hdr.audio_channels as u16);

		// Fill dec_data with stuff from this packet

		dec_data.append(&mut pck_decompressed);

		let mut diffs = 0;
		for (s,n) in dec_data.iter().zip(native_dec_data.iter()) {
			let diff = *s as i32 - *n as i32;
			// +- 1 deviation is allowed.
			if diff.abs() > 1 {
				diffs += 1;
			}
		}

		let native_dec_len = native_dec_data.len();
		let dec_len = dec_data.len();

		if diffs > 0 || (!ignore_packet_borders && dec_len != native_dec_len) {
			/*
			print!("Differences found in packet no {}... ", n);
			print!("ours={} native={}", dec_len, native_dec_len);
			println!(" (diffs {})", diffs);
			*/
			pcks_with_diffs += 1;
		}

		if ignore_packet_borders {
			native_dec_data.drain(..::std::cmp::min(native_dec_len, dec_len));
			dec_data.drain(..::std::cmp::min(native_dec_len, dec_len));
		} else {
			native_dec_data.truncate(0);
			dec_data.truncate(0);
		}
	}
	return (pcks_with_diffs, n);
}

use self::test_assets::TestAssetDef;

pub fn get_asset_defs() -> [TestAssetDef; 5] {
	return [
		TestAssetDef {
			filename : format!("bwv_1043_vivace.ogg"),
			hash : format!("839249e46220321e2bbb1106e30d0bef4acd800d3827a482743584f313c8c671"),
			url : format!("https://upload.wikimedia.org/wikipedia/commons/e/e9/Johann_Sebastian_Bach_-_Concerto_for_Two_Violins_in_D_minor_-_1._Vivace.ogg"),
		},
		TestAssetDef {
			filename : format!("bwv_543_fuge.ogg"),
			hash : format!("c5de55fe3613a88ba1622a1c931836c0af5e9bf3afae951418a07975a16e7421"),
			url : format!("https://upload.wikimedia.org/wikipedia/commons/4/4e/BWV_543-fugue.ogg"),
		},
		TestAssetDef {
			filename : format!("maple_leaf_rag.ogg"),
			hash : format!("f66f18de6bc79126f13d96831619d68ddd56f9527e50e1058be0754b479ee350"),
			url : format!("https://upload.wikimedia.org/wikipedia/commons/e/e9/Maple_Leaf_Rag_-_played_by_Scott_Joplin_1916_sample.ogg"),
		},
		TestAssetDef {
			filename : format!("hoelle_rache.ogg"),
			hash : format!("bbdf0a8d4c151aee5a21fb71ed86894b1aae5c7dba9ea767f7af6c0f752915c2"),
			url : format!("https://upload.wikimedia.org/wikipedia/commons/7/7d/Der_Hoelle_Rache.ogg"),
		},
		TestAssetDef {
			filename : format!("thingy-floor0.ogg"),
			hash : format!("02b9e94764db30b876964eba2d0a813dedaecdbfa978a13dc9cef9bdc1f4e9ee"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/abd0dbdb6803f9a591e9491d033d889812e877ae/tests/data/thingy-floor0.ogg"),
		},
	]
}
