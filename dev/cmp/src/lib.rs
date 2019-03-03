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
use lewton::header::{IdentHeader, CommentHeader, SetupHeader};
use std::time::{Duration, Instant};
use std::io::{Cursor, Read, Seek};

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

pub fn cmp_file_output(file_path :&str) -> (usize, usize) {
	macro_rules! try {
		($expr:expr) => (match $expr {
			$crate::std::result::Result::Ok(val) => val,
			$crate::std::result::Result::Err(err) => {
				panic!("Error: {:?}", err)
			}
		})
	}
	let f = try!(File::open(&file_path));
	let f_2 = try!(File::open(&file_path));
	try!(cmp_output(f, f_2, |u, v, _, _, _, _, _| (u, v)))
}

pub fn cmp_output<R :Read + Seek, T, F :Fn(usize, usize, usize,
		bool,
		&IdentHeader, &CommentHeader, &SetupHeader)->T>(
		rdr :R, rdr_2 :R, f :F) -> Result<T, String> {
	macro_rules! try {
		($expr:expr) => (match $expr {
			$crate::std::result::Result::Ok(val) => val,
			$crate::std::result::Result::Err(err) => {
				return Err(format!("{:?}", err))
			}
		})
	}
	let f_n = rdr;
	let f_r = rdr_2;

	let dec = try!(NativeDecoder::new(f_n));

	let mut ogg_rdr = try!(OggStreamReader::new(f_r));
	let stream_serial = ogg_rdr.stream_serial();

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

	let mut total_sample_count = 0;

	let mut chained_ogg_file = false;
	loop {
		n += 1;

		let mut native_decoded = try!(match native_it.next() { Some(v) => v,
			None => break,});
		native_dec_data.extend_from_slice(&mut native_decoded.data);
		let mut pck_decompressed = match try!(ogg_rdr.read_dec_packet_itl()) {
			Some(v) => v,
			None => break, // TODO tell calling code about this condition
		};

		// Asserting some very basic things:
		assert_eq!(native_decoded.rate, ogg_rdr.ident_hdr.audio_sample_rate as u64);
		assert_eq!(native_decoded.channels, ogg_rdr.ident_hdr.audio_channels as u16);

		total_sample_count += pck_decompressed.len();
		if stream_serial != ogg_rdr.stream_serial() {
			// Chained ogg file
			chained_ogg_file = true;
		}

		// Fill dec_data with stuff from this packet

		dec_data.extend_from_slice(&mut pck_decompressed);

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
	return Ok(f(pcks_with_diffs, n, total_sample_count,
		chained_ogg_file,
		&ogg_rdr.ident_hdr, &ogg_rdr.comment_hdr, &ogg_rdr.setup_hdr));
}

/// Like try, but performs an action if an "expected" error
/// is intercepted
#[macro_export]
macro_rules! try_expected {
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

/// Ensures that a file is malformed and returns an error,
/// but doesn't panic or crash or anything of the like
#[macro_export]
macro_rules! ensure_malformed {
	($name:expr, $expected:pat) => {{
		use std::fs::File;
		use lewton::inside_ogg::OggStreamReader;
		// Read the file to memory
		let f = File::open(format!("test-assets/{}", $name)).unwrap();
		if let Some(mut ogg_rdr) = try_expected!(OggStreamReader::new(f).map(|v| Some(v)), $expected, None) {
			loop {
				match try_expected!(ogg_rdr.read_dec_packet_itl(), $expected, break) {
					Some(_) => (),
					None => panic!("File {} decoded without errors", $name),
				};
			}
		}
	}}
}

/// Ensures that a file decodes without errors
#[macro_export]
macro_rules! ensure_okay {
	($name:expr) => {{
		use std::fs::File;
		use lewton::inside_ogg::OggStreamReader;
		// Read the file to memory
		let f = File::open(format!("test-assets/{}", $name)).unwrap();
		if let Some(mut ogg_rdr) = OggStreamReader::new(f).map(|v| Some(v)).unwrap() {
			loop {
				match ogg_rdr.read_dec_packet_itl().unwrap() {
					Some(_) => (),
					None => break,
				};
			}
		}
	}}
}

use self::test_assets::TestAssetDef;

pub fn get_asset_defs() -> [TestAssetDef; 6] {
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
		TestAssetDef {
			filename : format!("audio_simple_err.ogg"),
			hash : format!("1b97b2b151b34f1ca6868aa0088535792252aa5c7c990e1de9eedd6a33d3c0dd"),
			url : format!("https://github.com/RustAudio/lewton/files/1543593/audio_simple_err.zip"),
		},
	];
}

#[allow(unused)]
pub fn get_libnogg_asset_defs() -> [TestAssetDef; 32] {
	return [
		TestAssetDef {
			filename : format!("6-mode-bits-multipage.ogg"),
			hash : format!("e68f06c58a8125933d869d4c831ee298500b1894ab27dc4841f075c104093c27"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/6-mode-bits-multipage.ogg"),
		},
		TestAssetDef {
			filename : format!("6-mode-bits.ogg"),
			hash : format!("48ec7d1b3284ea8cdb9a3511f4f1dd4d8170be9482dd7c0e8edb49802318e1c8"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/6-mode-bits.ogg"),
		},
		TestAssetDef {
			filename : format!("6ch-all-page-types.ogg"),
			hash : format!("c965f1f03be8af3869d22fcad41bbde0111ba18748f7c9b27fb46e4a33b857cd"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/6ch-all-page-types.ogg"),
		},
		TestAssetDef {
			filename : format!("6ch-long-first-packet.ogg"),
			hash : format!("7ac5d89b9cc69762dd191d17778b0625462d4a2c5488d10e8aef6a77e990c7f7"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/6ch-long-first-packet.ogg"),
		},
		TestAssetDef {
			filename : format!("6ch-moving-sine-floor0.ogg"),
			hash : format!("95443ad7c16f3dc7f66ce34aeb3ec90f14fb717c79326e8f3c92826f5c0606e1"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/6ch-moving-sine-floor0.ogg"),
		},
		TestAssetDef {
			filename : format!("6ch-moving-sine.ogg"),
			hash : format!("05dae404fc266671598aaf2fd55f52d563e7f26631abbd3487cc9b63d458500d"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/6ch-moving-sine.ogg"),
		},
		TestAssetDef {
			filename : format!("bad-continued-packet-flag.ogg"),
			hash : format!("8c93b1ec92746b4c9eb8c855e65218f14cd8c3024ddea5b3264c8a66fee7d6ee"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/bad-continued-packet-flag.ogg"),
		},
		TestAssetDef {
			filename : format!("bitrate-123.ogg"),
			hash : format!("8cbb82b8eab2e4d4115b62f62323c85d05e2352519677996d7a1e53c01ebb436"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/bitrate-123.ogg"),
		},
		TestAssetDef {
			filename : format!("bitrate-456-0.ogg"),
			hash : format!("2ae12b963c333164f1fbbbf4fc75bc9aa6b5b4289a9791ba9a3a7c275d725571"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/bitrate-456-0.ogg"),
		},
		TestAssetDef {
			filename : format!("bitrate-456-789.ogg"),
			hash : format!("326bb289c1dcc4f79ee007199a8124dd14bfea4a1f7fb94f9c7f9394c0b53584"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/bitrate-456-789.ogg"),
		},
		TestAssetDef {
			filename : format!("empty-page.ogg"),
			hash : format!("51010e14b84dee562b76d3a4ddb760d4b3a740d78714b11b6307e2de73ee6d48"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/empty-page.ogg"),
		},
		TestAssetDef {
			filename : format!("large-pages.ogg"),
			hash : format!("53b63f9661ddf726bd0b9d1933b7fe54ef10248742354ff215b294f34ac0aec0"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/large-pages.ogg"),
		},
		TestAssetDef {
			filename : format!("long-short.ogg"),
			hash : format!("96a166446ee171f9df3b7a4701567d19e52d8cfc593e17ca4f8697dde551de63"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/long-short.ogg"),
		},
		TestAssetDef {
			filename : format!("noise-6ch.ogg"),
			hash : format!("879ab419c0a848b0d17b1d6b9d8557c4cec35ffb0d973371743eeee70c6f7e17"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/noise-6ch.ogg"),
		},
		TestAssetDef {
			filename : format!("noise-stereo.ogg"),
			hash : format!("53c9e52bf47f89d292644e4cd467da70c49709fdcc7a2c99d171b4e1883a0499"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/noise-stereo.ogg"),
		},
		TestAssetDef {
			filename : format!("partial-granule-position.ogg"),
			hash : format!("d42765b76989a74fc9071d082409a34e9a4c603eecb51b253d4d026a9751580e"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/partial-granule-position.ogg"),
		},
		TestAssetDef {
			filename : format!("sample-rate-max.ogg"),
			hash : format!("c758248cdc2d2ed67ed80f27e8436565fd5b94c5c662a88ab9a5fabd6d23ff04"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/sample-rate-max.ogg"),
		},
		TestAssetDef {
			filename : format!("single-code-2bits.ogg"),
			hash : format!("ee1eb710f37709fea87d1fac6cdb6ab86012497c50dae58d237b6607165656b9"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/single-code-2bits.ogg"),
		},
		TestAssetDef {
			filename : format!("single-code-nonsparse.ogg"),
			hash : format!("9fbbbe8ba4988d8362a66f4252fe8f528e41ac659db23f377a9166bba843bb7b"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/single-code-nonsparse.ogg"),
		},
		TestAssetDef {
			filename : format!("single-code-ordered.ogg"),
			hash : format!("27e53ee98f2773405b98f6ad402aad2d0be3d3fd7439acab720f46c73957bc93"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/single-code-ordered.ogg"),
		},
		TestAssetDef {
			filename : format!("single-code-sparse.ogg"),
			hash : format!("3327a0eb7287bc2df2a10932446930c9da6fa3383b0a63a81cfd04f832e160a6"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/single-code-sparse.ogg"),
		},
		TestAssetDef {
			filename : format!("sketch008-floor0.ogg"),
			hash : format!("64fc1efe12609a0544448021c730c2832a758637982a4698a138f138a1417b5c"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/sketch008-floor0.ogg"),
		},
		TestAssetDef {
			filename : format!("sketch008.ogg"),
			hash : format!("d0b34d94a5379edc6eb633743ecd187d81b02e5354fed989f35320ecfd32be71"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/sketch008.ogg"),
		},
		TestAssetDef {
			filename : format!("sketch039.ogg"),
			hash : format!("c595bec5d9bad0103527f779dba13837743866c15ff5ad7aeedc6dab49516d9a"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/sketch039.ogg"),
		},
		TestAssetDef {
			filename : format!("split-packet.ogg"),
			hash : format!("e5e5598845d733a1efaeb258a6c07563af8dd30204117044d25df13cd2944e0e"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/split-packet.ogg"),
		},
		TestAssetDef {
			filename : format!("square-interleaved.ogg"),
			hash : format!("305a703187f5ad84facbf5b8990007cfe93d4035e3efb9f86014967604374356"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/square-interleaved.ogg"),
		},
		TestAssetDef {
			filename : format!("square-multipage.ogg"),
			hash : format!("691988c4fefe850dd265fd8c91f2e90c92f94578ce18c8005b40e88277dc26c9"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/square-multipage.ogg"),
		},
		TestAssetDef {
			filename : format!("square-stereo.ogg"),
			hash : format!("b2eb353cdd9ddd3f809647474e1a8bff6913e28b53cac52a54906f7ff203501f"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/square-stereo.ogg"),
		},
		TestAssetDef {
			filename : format!("square-with-junk.ogg"),
			hash : format!("51ede72de15b2998cf8de6dd3f57a6abc280f9278d775f0f5fec2d2679e341b6"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/square-with-junk.ogg"),
		},
		TestAssetDef {
			filename : format!("square.ogg"),
			hash : format!("31c9fa7d2f374ebf9ffc1c21e95b8369c2ced3ae230151c00208613ca308d812"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/square.ogg"),
		},
		// Omit thingy-floor0.ogg
		TestAssetDef {
			filename : format!("thingy.ogg"),
			hash : format!("646f05235723aa09e69e123c73560ffb753f9ffe00e3c54b99f8f0c1cd583707"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/thingy.ogg"),
		},
		TestAssetDef {
			filename : format!("zero-length.ogg"),
			hash : format!("aa71c87218f6dec51383110bc1b77d204bbb21f7867e2cc8283042417522b330"),
			url : format!("https://bitbucket.org/achurch_/libnogg/raw/c80b37a361e13803c459bd578f68db09362a9f63/tests/data/zero-length.ogg"),
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

#[allow(unused)]
/// Regression tests for bugs obtained by fuzzing
///
/// The test files are licensed under CC-0:
/// * https://github.com/RustAudio/lewton/issues/33#issuecomment-419640709
/// * http://web.archive.org/web/20180910135020/https://github.com/RustAudio/lewton/issues/33
pub fn get_fuzzed_asset_defs() -> [TestAssetDef; 12] {
	return [
		TestAssetDef {
			filename : format!("27_really_minimized_testcase_crcfix.ogg"),
			hash : format!("83f6d6f36ae926000f064007e79ef7c45ed561e49223d9b68f980d264050d683"),
			url : format!("https://github.com/RustAudio/lewton/files/2363013/27_really_minimized_testcase_crcfix.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("32_minimized_crash_testcase.ogg"),
			hash : format!("644170ccc3e48f2e2bf28cadddcd837520df09671c3d3d991b128b9fdb281da6"),
			url : format!("https://github.com/RustAudio/lewton/files/2363080/32_minimized_crash_testcase.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("33_minimized_panic_testcase.ogg"),
			hash : format!("4812e725d7c6bdb48e745b4e0a396efc96ea5cb30e304cf9710dadda3d963171"),
			url : format!("https://github.com/RustAudio/lewton/files/2363173/33_minimized_panic_testcase.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("35_where_did_my_memory_go_repacked.ogg"),
			hash : format!("2f202e71ca0440a2de4a15443beae9d3230e81e47bc01d29929fc86ee731887c"),
			url : format!("https://github.com/RustAudio/lewton/files/2889595/35_where-did-my-memory-go-repacked.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-42-sample009.ogg"),
			hash : format!("7e3d7fd6d306cd1c1704d0586b4e62cc897c499e3ffc1911f62ec0fc3a062871"),
			url : format!("https://github.com/RustAudio/lewton/files/2905014/bug-42-sample009.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-42-sample012.ogg"),
			hash : format!("8d92c4359bbe987b77459f309859b6bba0a11724e71fd5e81873c597ec71d857"),
			url : format!("https://github.com/RustAudio/lewton/files/2905017/bug-42-sample012.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-42-sample015.ogg"),
			hash : format!("274c17222d7cfc1044d2fee3e60377eac87f5ee8d952eeaf3d636b016b1db7d3"),
			url : format!("https://github.com/RustAudio/lewton/files/2905018/bug-42-sample015.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-42-sample016.ogg"),
			hash : format!("ab02fd55a275b1ec0c6c56a667834231bf34b3a79038f43196d1015c1555e535"),
			url : format!("https://github.com/RustAudio/lewton/files/2905019/bug-42-sample016.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-42-sample029.ogg"),
			hash : format!("1436fff4d8fa61ff2b22ffd021c2bd80f072556b8b58cfc72fdfc0434efd9a24"),
			url : format!("https://github.com/RustAudio/lewton/files/2905020/bug-42-sample029.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-44-sample059.ogg"),
			hash : format!("4c1452e387a64090465132724a83f02846457336fa58ddc6ee9c6df598d756c0"),
			url : format!("https://github.com/RustAudio/lewton/files/2922511/bug-44-sample059.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-44-sample060.ogg"),
			hash : format!("b8bd42831a8922c4c78ff1ea5b42ecbb874135ba7e7fcd60c4fff7a419d857a4"),
			url : format!("https://github.com/RustAudio/lewton/files/2922512/bug-44-sample060.ogg.zip"),
		},
		TestAssetDef {
			filename : format!("bug-46-sample001.ogg"),
			hash : format!("d5015f9a3b79a28bf621ecc2e96286c20ef742e936e256f77b8978e6bce66aad"),
			url : format!("https://github.com/RustAudio/lewton/files/2923287/bug-46-sample001.ogg.zip"),
		},
	];
}
