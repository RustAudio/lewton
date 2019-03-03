// Vorbis decoder written in Rust
//
// Copyright (c) 2018 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate test_assets;
#[macro_use]
extern crate cmp;
extern crate lewton;

#[test]
fn test_malformed_fuzzed() {
	println!();
	test_assets::download_test_files(&cmp::get_fuzzed_asset_defs(),
		"test-assets", true).unwrap();
	println!();

	use lewton::VorbisError::*;
	use lewton::audio::AudioReadError::*;
	use lewton::header::HeaderReadError::*;

	ensure_malformed!("27_really_minimized_testcase_crcfix.ogg", BadAudio(AudioBadFormat));
	ensure_malformed!("32_minimized_crash_testcase.ogg", BadHeader(HeaderBadFormat));
	ensure_malformed!("35_where_did_my_memory_go_repacked.ogg", BadHeader(HeaderBadFormat));

	ensure_malformed!("bug-42-sample009.ogg", BadAudio(AudioBadFormat));
	ensure_malformed!("bug-42-sample012.ogg", BadAudio(AudioBadFormat));
	//ensure_malformed!("bug-42-sample015.ogg", BadAudio(AudioBadFormat));

	ensure_malformed!("bug-44-sample059.ogg", BadHeader(HeaderBadFormat));
	ensure_malformed!("bug-44-sample060.ogg", BadHeader(HeaderBadFormat));

	ensure_malformed!("bug-46-sample001.ogg", BadAudio(AudioBadFormat));
}

#[test]
fn test_okay_fuzzed() {
	println!();
	test_assets::download_test_files(&cmp::get_fuzzed_asset_defs(),
		"test-assets", true).unwrap();
	println!();

	ensure_okay!("33_minimized_panic_testcase.ogg");
	ensure_okay!("bug-42-sample016.ogg");
	ensure_okay!("bug-42-sample029.ogg");
}
