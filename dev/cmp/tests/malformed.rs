// Vorbis decoder written in Rust
//
// Copyright (c) 2018 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate test_assets;
extern crate cmp;
extern crate lewton;

use std::fs::File;
use lewton::inside_ogg::OggStreamReader;
use lewton::VorbisError;

macro_rules! try {
	($expr:expr) => (match $expr {
		$crate::std::result::Result::Ok(val) => val,
		$crate::std::result::Result::Err(err) => {
			panic!("Error: {:?}", err)
		}
	})
}

// Ensures that a file is malformed and returns an error,
// but
macro_rules! ensure_malformed {
	($name:expr, $type:expr) => {{
		// Read the file to memory
		let f = try!(File::open(format!("test-assets/{}", $name)));
		let mut ogg_rdr = try!(OggStreamReader::new(f));
		loop {
			match ogg_rdr.read_dec_packet_itl() {
				Ok(Some(_)) => (),
				Ok(None) => panic!("File {} decoded without errors", $name),
				Err(VorbisError::BadAudio(e)) => {
					assert_eq!(e, $type);
					break;
				},
				Err(e) => {
					panic!("Unexpected error: {:?}", e);
				},
			};
		}
	}}
}

#[test]
fn test_malformed_fuzzed() {
	println!();
	test_assets::download_test_files(&cmp::get_malformed_asset_defs(),
		"test-assets", true).unwrap();
	println!();

	use lewton::audio::AudioReadError::*;

	ensure_malformed!("27_really_minimized_testcase_crcfix.ogg", AudioBadFormat);
}
