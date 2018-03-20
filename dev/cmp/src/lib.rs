// Vorbis decoder written in Rust
//
// Copyright (c) 2016-2017 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate lewton;
extern crate vorbis;
extern crate test_assets;

use std::fs::File;

use lewton::inside_ogg::*;
use std::time::{Duration, Instant};
use std::io::{Cursor, Read};

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

	// Read the file to memory to create fairer playing ground
	let mut f = try!(File::open(file_path));
	let mut file_buf = Vec::new();
	try!(f.read_to_end(&mut file_buf));

	let r_n = Cursor::new(&file_buf);
	let start_native_decode = Instant::now();
	let dec = try!(NativeDecoder::new(r_n));
	let mut native_it = dec.into_packets();
	loop {
		try!(match native_it.next() {
			Some(v) => v,
			None => break,
		});
	}
	let native_decode_duration = Instant::now() - start_native_decode;

	let mut n = 0;
	let r_r = Cursor::new(&file_buf);
	let start_decode = Instant::now();
	let mut ogg_rdr = try!(OggStreamReader::new(r_r));

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

	let mut ogg_rdr = try!(OggStreamReader::new(f_r));

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
	];
}

#[allow(unused)]
pub fn get_xiph_asset_defs_1() -> [TestAssetDef; 5] {
	return [
		TestAssetDef {
			filename : format!("1.0-test.ogg"),
			hash : format!("9a882710314bcc1d2b4cdefb7f89911f4375acd1feb9e64a12eca9f7202377d3"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/1.0-test.ogg"),
		},
		TestAssetDef {
			filename : format!("1.0.1-test.ogg"),
			hash : format!("8c9423e00826d6d2457d78c09d6e2a94bcdf9216af796bb22a4c4f450c2e72bb"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/1.0.1-test.ogg"),
		},
		TestAssetDef {
			filename : format!("48k-mono.ogg"),
			hash : format!("f51459d9bdd04ca3ec6f6732b8a01efcdc83e5c6fc79c7d5347527bfd4948e9e"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/48k-mono.ogg"),
		},
		TestAssetDef {
			filename : format!("beta3-test.ogg"),
			hash : format!("7fc791a4d5a0d3b7cef2448093ccf2ae54600acc81667be90c6b209c366c32ea"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/beta3-test.ogg"),
		},
		TestAssetDef {
			filename : format!("beta4-test.ogg"),
			hash : format!("bc367c0d4dcdbf1f0a2f81abf74cc52c8f0ab8366d150a75830782ca2dba080a"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/beta4-test.ogg"),
		},
	];
}

#[allow(unused)]
pub fn get_xiph_asset_defs_2() -> [TestAssetDef; 5] {
	return [
		TestAssetDef {
			filename : format!("bimS-silence.ogg"),
			hash : format!("e2a38871d390ed651faf0fec5253a6873530da4f2503d85021c99325dfd23813"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/bimS-silence.ogg"),
		},
		TestAssetDef {
			filename : format!("chain-test1.ogg"),
			hash : format!("d9c37533a1f456d2a996755a43d112ef46b6ef953319913907bc79f1c014d79e"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/chain-test1.ogg"),
		},
		TestAssetDef {
			filename : format!("chain-test2.ogg"),
			hash : format!("5b5bf834e93e9a93b7114be084161e972f33d2031963518492ba2c91dc4418a7"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/chain-test2.ogg"),
		},
		TestAssetDef {
			filename : format!("chain-test3.ogg"),
			hash : format!("ed039ba775d1b31e805d26d6413a3ef2c6663bf4c5ea47ce1462f8f23c84d8dc"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/chain-test3.ogg"),
		},
		TestAssetDef {
			filename : format!("highrate-test.ogg"),
			hash : format!("0942c88369f84b125388cc8437575b66d67bc97baaeeb997b6602144821509df"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/highrate-test.ogg"),
		},
	];
}

#[allow(unused)]
pub fn get_xiph_asset_defs_3() -> [TestAssetDef; 5] {
	return [
		TestAssetDef {
			filename : format!("lsp-test.ogg"),
			hash : format!("ad1b07b68576ae2c85178475ef3607e1a96c09e96296cae83ee7a9726f8311eb"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/lsp-test.ogg"),
		},
		TestAssetDef {
			filename : format!("lsp-test2.ogg"),
			hash : format!("7b00f893a93071bdf243b8a22c1758a6ab09c1ab8eee89ca21659d98ec0f53ea"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/lsp-test2.ogg"),
		},
		TestAssetDef {
			filename : format!("lsp-test3.ogg"),
			hash : format!("7a5c4064fc31285f6fea0b2424a0d415a8b18ef13d1aa1ef931a429ab9a593d1"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/lsp-test3.ogg"),
		},
		TestAssetDef {
			filename : format!("lsp-test4.ogg"),
			hash : format!("cb0b28931dfc8ef8b2d320f3028fab548171e7c6e94006f0c23532478a8d3596"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/lsp-test4.ogg"),
		},
		TestAssetDef {
			filename : format!("mono.ogg"),
			hash : format!("d8abca95445a07186c9a158d4f573f38918985dd2498a9bc8d1811bd72fe1d1a"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/mono.ogg"),
		},
	];
}

#[allow(unused)]
pub fn get_xiph_asset_defs_4() -> [TestAssetDef; 7] {
	return [
		TestAssetDef {
			filename : format!("moog.ogg"),
			hash : format!("bd5b51bb1d6855e0e990e3ebdd230fc16265407809bd6db44aea691ada498943"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/moog.ogg"),
		},
		TestAssetDef {
			filename : format!("one-entry-codebook-test.ogg"),
			hash : format!("789b5146f2a7c0864a228ee4a870606a32c4169e22097be65129623705c54749"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/one-entry-codebook-test.ogg"),
		},
		TestAssetDef {
			filename : format!("out-of-spec-blocksize.ogg"),
			hash : format!("0970e66291744815f2ca0dec7523f5bbc112907c5c75978a0f91d3b0ed6f03b2"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/out-of-spec-blocksize.ogg"),
		},
		TestAssetDef {
			filename : format!("rc1-test.ogg"),
			hash : format!("ccfac6ac7c75615bec0632b94bcf088c027c0b112cd136c997d04f037252cd3d"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/rc1-test.ogg"),
		},
		TestAssetDef {
			filename : format!("rc2-test.ogg"),
			hash : format!("4adcda786dfeea4188d7b1df35571dbe29c687f2d1cac680ed115353abfe1637"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/rc2-test.ogg"),
		},
		TestAssetDef {
			filename : format!("rc2-test2.ogg"),
			hash : format!("24ade471eefe1c7642f73a6810e56266b34c2d7d30a3b5ad05a34c098128226c"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/rc2-test2.ogg"),
		},
		TestAssetDef {
			filename : format!("rc3-test.ogg"),
			hash : format!("edd984e84c7c2a59af7801f3e8d2db11a6619134fd8d0274d1289e70bf60cde7"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/rc3-test.ogg"),
		},
	];
}

#[allow(unused)]
pub fn get_xiph_asset_defs_5() -> [TestAssetDef; 5] {
	return [
		TestAssetDef {
			filename : format!("singlemap-test.ogg"),
			hash : format!("50d8077608a4192b8f8505aec0217be8b6c25def4068899afc19c17f30e1d521"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/singlemap-test.ogg"),
		},
		TestAssetDef {
			filename : format!("sleepzor.ogg"),
			hash : format!("01c67ecaf7a58b5ac5f1fe3bd060b5d61536595e97927a1b1cf0129a62b5cfcf"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/sleepzor.ogg"),
		},
		TestAssetDef {
			filename : format!("test-short.ogg"),
			hash : format!("183510552403021fb90ce43796fcc88e16c8bb4ae5d0d72a316b8e51afc395fa"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/test-short.ogg"),
		},
		TestAssetDef {
			filename : format!("test-short2.ogg"),
			hash : format!("6bd3f59e0fa77904edd35c73dadd6558e2a36d78c9d7bc5db223862bf092fcc2"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/test-short2.ogg"),
		},
		TestAssetDef {
			filename : format!("unused-mode-test.ogg"),
			hash : format!("e27ae6fc2f7c0037c28328801c46d966abcee3050e3ebd40bf097f7986f50f94"),
			url : format!("https://people.xiph.org/~xiphmont/test-vectors/vorbis/unused-mode-test.ogg"),
		},
	];
}
