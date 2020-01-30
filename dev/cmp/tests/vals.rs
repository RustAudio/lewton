// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate test_assets;
extern crate lewton;
#[macro_use]
extern crate cmp;

use lewton::VorbisError::*;
use lewton::header::HeaderReadError::*;
use lewton::audio::AudioReadError::*;

macro_rules! cmp_output {
	($str:expr, $max_diff:expr) => {
		print!("Comparing output for {} ", $str);
		let (diff_pck_count, _) = cmp::cmp_file_output(&format!("test-assets/{}", $str));
		println!(": {} differing packets of allowed {}.", diff_pck_count, $max_diff);
		assert!(diff_pck_count <= $max_diff);
	};
}

#[test]
fn test_vals() {
	println!();
	test_assets::download_test_files(&cmp::get_asset_defs(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("bwv_1043_vivace.ogg", 0);
	cmp_output!("bwv_543_fuge.ogg", 0);
	cmp_output!("maple_leaf_rag.ogg", 0);
	cmp_output!("hoelle_rache.ogg", 0);
	cmp_output!("thingy-floor0.ogg", 0);
	cmp_output!("audio_simple_err.ogg", 0);
}

#[test]
fn test_libnogg_vals() {
	println!();
	test_assets::download_test_files(&cmp::get_libnogg_asset_defs(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("6-mode-bits-multipage.ogg", 2);
	cmp_output!("6-mode-bits.ogg", 2);
	cmp_output!("6ch-all-page-types.ogg", 0);
	cmp_output!("6ch-long-first-packet.ogg", 0);
	cmp_output!("6ch-moving-sine-floor0.ogg", 0);
	cmp_output!("6ch-moving-sine.ogg", 0);
	// NOTE: The bad-continued-packet-flag.ogg test is
	// actually supposed to return an error in libnogg.
	// However, libvorbis doesn't, nor does lewton.
	// Given a (slightly) erroneous ogg file where there
	// are audio packets following the last header packet,
	// we follow libvorbis behaviour and simply ignore those packets.
	// Apparently the test case has been created in a way
	// where this behaviour doesn't evoke an error from lewton.
	cmp_output!("bad-continued-packet-flag.ogg", 0);
	cmp_output!("bitrate-123.ogg", 0);
	cmp_output!("bitrate-456-0.ogg", 0);
	cmp_output!("bitrate-456-789.ogg", 0);
	cmp_output!("empty-page.ogg", 0);
	cmp_output!("large-pages.ogg", 2);
	cmp_output!("long-short.ogg", 2);
	cmp_output!("noise-6ch.ogg", 0);
	cmp_output!("noise-stereo.ogg", 0);
	cmp_output!("partial-granule-position.ogg", 2);
	#[cfg(not(target_os = "windows"))]
	cmp_output!("sample-rate-max.ogg", 0);
	ensure_malformed!("single-code-2bits.ogg", BadHeader(HeaderBadFormat));
	// We can't cmp the output here because native
	// libvorbis doesn't accept the file as valid
	ensure_okay!("single-code-nonsparse.ogg");
	ensure_okay!("single-code-ordered.ogg");
	cmp_output!("single-code-sparse.ogg", 0);
	#[cfg(not(target_os = "macos"))]
	cmp_output!("sketch008-floor0.ogg", 0);
	cmp_output!("sketch008.ogg", 0);
	cmp_output!("sketch039.ogg", 0);
	cmp_output!("split-packet.ogg", 2);
	cmp_output!("square-interleaved.ogg", 0);
	cmp_output!("square-multipage.ogg", 0);
	cmp_output!("square-stereo.ogg", 0);
	// This is really more an issue of the ogg crate,
	// if it's an issue at all.
	// https://github.com/RustAudio/ogg/issues/7
	//ensure_malformed!("square-with-junk.ogg", OggError(NoCapturePatternFound));
	cmp_output!("square.ogg", 0);
	cmp_output!("thingy.ogg", 0);
	cmp_output!("zero-length.ogg", 0);
}

#[test]
fn test_xiph_vals_1() {
	println!();
	test_assets::download_test_files(&cmp::get_xiph_asset_defs_1(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("1.0-test.ogg", 0);
	cmp_output!("1.0.1-test.ogg", 0);
	cmp_output!("48k-mono.ogg", 0);
	cmp_output!("beta3-test.ogg", 0);
	cmp_output!("beta4-test.ogg", 1);
}

#[test]
fn test_xiph_vals_2() {
	println!();
	test_assets::download_test_files(&cmp::get_xiph_asset_defs_2(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("bimS-silence.ogg", 0);
	cmp_output!("chain-test1.ogg", 0);
	cmp_output!("chain-test2.ogg", 0);
	cmp_output!("chain-test3.ogg", 1);
	cmp_output!("highrate-test.ogg", 0);
}

#[test]
fn test_xiph_vals_3() {
	println!();
	test_assets::download_test_files(&cmp::get_xiph_asset_defs_3(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("lsp-test.ogg", 0);
	cmp_output!("lsp-test2.ogg", 0);
	cmp_output!("lsp-test3.ogg", 0);
	cmp_output!("lsp-test4.ogg", 0);
	cmp_output!("mono.ogg", 0);
}

#[test]
fn test_xiph_vals_4() {
	println!();
	test_assets::download_test_files(&cmp::get_xiph_asset_defs_4(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("moog.ogg", 0);
	cmp_output!("one-entry-codebook-test.ogg", 0);
	cmp_output!("out-of-spec-blocksize.ogg", 0);
	cmp_output!("rc1-test.ogg", 0);
	cmp_output!("rc2-test.ogg", 0);
	cmp_output!("rc2-test2.ogg", 0);
	cmp_output!("rc3-test.ogg", 0);
}

#[test]
fn test_xiph_vals_5() {
	println!();
	test_assets::download_test_files(&cmp::get_xiph_asset_defs_5(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("singlemap-test.ogg", 0);
	#[cfg(not(target_os = "macos"))]
	cmp_output!("sleepzor.ogg", 0);
	cmp_output!("test-short.ogg", 0);
	cmp_output!("test-short2.ogg", 0);
	// Contains an out of bounds mode index
	ensure_malformed!("unused-mode-test.ogg", BadAudio(AudioBadFormat));
}
