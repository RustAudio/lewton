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

macro_rules! try {
	($expr:expr) => (match $expr {
		$crate::std::result::Result::Ok(val) => val,
		$crate::std::result::Result::Err(err) => {
			panic!("Error: {:?}", err)
		}
	})
}

macro_rules! etry {
	($expr:expr, $expected:pat, $action:tt) => (match $expr {
		Ok(val) => val,
		Err($expected) => {
			$action
		},
		Err(e) => {
			panic!("Unexpected error: {:?}\nExpected: {:?}", e, stringify!($type));
		},
	})
}

// Ensures that a file is malformed and returns an error,
// but
macro_rules! ensure_malformed {
	($name:expr, $expected:pat) => {{
		// Read the file to memory
		let f = try!(File::open(format!("test-assets/{}", $name)));
		if let Some(mut ogg_rdr) = etry!(OggStreamReader::new(f).map(|v| Some(v)), $expected, None) {
			loop {
				match etry!(ogg_rdr.read_dec_packet_itl(), $expected, break) {
					Some(_) => (),
					None => panic!("File {} decoded without errors", $name),
				};
			}
		}
	}}
}

#[test]
fn test_malformed_fuzzed() {
	println!();
	test_assets::download_test_files(&cmp::get_malformed_asset_defs(),
		"test-assets", true).unwrap();
	println!();

	use lewton::VorbisError::*;
	use lewton::audio::AudioReadError::*;
	use lewton::header::HeaderReadError::*;

	ensure_malformed!("27_really_minimized_testcase_crcfix.ogg", BadAudio(AudioBadFormat));
	ensure_malformed!("32_minimized_crash_testcase.ogg", BadHeader(HeaderBadFormat));
}
