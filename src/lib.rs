// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

#![cfg_attr(not(cargo_c), forbid(unsafe_code))]
#![cfg_attr(test, deny(warnings))]

/*!
A `vorbis` decoder, written in Rust.

If you "just" want to decode `ogg/vorbis` files, take a look into
the `inside_ogg` module (make sure you haven't disabled the `ogg` feature).

For lower level, per-packet usage, you can have a look at the `audio` and `header`
modules.
*/

extern crate byteorder;
extern crate tinyvec;
#[cfg(feature = "ogg")]
extern crate ogg;
#[cfg(feature = "async_ogg")]
#[macro_use]
extern crate futures;
#[cfg(feature = "async_ogg")]
extern crate tokio_io;

macro_rules! try {
	($expr:expr) => (match $expr {
		$crate::std::result::Result::Ok(val) => val,
		$crate::std::result::Result::Err(err) => {
			return Err($crate::std::convert::From::from(err));
		}
	})
}

/*
// This little thing is very useful.
macro_rules! try {
	($expr:expr) => (match $expr {
		$crate::std::result::Result::Ok(val) => val,
		$crate::std::result::Result::Err(err) => {
			panic!("Panic on Err turned on for debug reasons. Encountered Err: {:?}", err)
		}
	})
}
// */

// The following macros are super useful for debugging

macro_rules! record_residue_pre_inverse {
	($residue_vectors:expr) => {
// 		for v in $residue_vectors.iter() {
// 			for &re in v {
// 				println!("{}", re);
// 			}
// 		}
	}
}

macro_rules! record_residue_post_inverse {
	($residue_vectors:expr) => {
// 		for v in $residue_vectors.iter() {
// 			for &re in v {
// 				println!("{}", re);
// 			}
// 		}
	}
}

macro_rules! record_pre_mdct {
	($audio_spectri:expr) => {
// 		for v in $audio_spectri.iter() {
// 			for &s in v {
// 				println!("{:.5}", s);
// 			}
// 		}
	}
}

macro_rules! record_post_mdct {
	($audio_spectri:expr) => {
// 		for v in $audio_spectri.iter() {
// 			for &s in v {
// 				println!("{:.4}", s);
// 			}
// 		}
	}
}

pub mod header;
mod header_cached;
mod huffman_tree;
mod imdct;
#[cfg(test)]
mod imdct_test;
pub mod audio;
mod bitpacking;
#[cfg(feature = "ogg")]
pub mod inside_ogg;
pub mod samples;

#[cfg(feature = "ogg")]
#[doc(no_inline)]
pub use ogg::OggReadError;

#[cfg(cargo_c)]
mod capi;

#[cfg(cargo_c)]
pub use capi::*;

/// Errors that can occur during decoding
#[derive(Debug)]
pub enum VorbisError {
	BadAudio(audio::AudioReadError),
	BadHeader(header::HeaderReadError),
	#[cfg(feature = "ogg")]
	OggError(OggReadError),
}

impl std::error::Error for VorbisError {}

impl std::fmt::Display for VorbisError {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
		write!(fmt, "{}", match self {
			VorbisError::BadAudio(_) => "Vorbis bitstream audio decode problem",
			VorbisError::BadHeader(_) => "Vorbis bitstream header decode problem",
			#[cfg(feature = "ogg")]
			VorbisError::OggError(_) => "Ogg decode problem",
		})
	}
}

impl From<audio::AudioReadError> for VorbisError {
	fn from(err :audio::AudioReadError) -> VorbisError {
		VorbisError::BadAudio(err)
	}
}

impl From<header::HeaderReadError> for VorbisError {
	fn from(err :header::HeaderReadError) -> VorbisError {
		VorbisError::BadHeader(err)
	}
}

#[cfg(feature = "ogg")]
impl From<OggReadError> for VorbisError {
	fn from(err :OggReadError) -> VorbisError {
		VorbisError::OggError(err)
	}
}

fn ilog(val :u64) -> u8 {
	64 - val.leading_zeros() as u8
}

#[test]
fn test_ilog() {
	// Uses the test vectors from the Vorbis I spec
	assert_eq!(ilog(0), 0);
	assert_eq!(ilog(1), 1);
	assert_eq!(ilog(2), 2);
	assert_eq!(ilog(3), 2);
	assert_eq!(ilog(4), 3);
	assert_eq!(ilog(7), 3);
}

fn bit_reverse(n :u32) -> u32 {
	// From the stb_vorbis implementation
	let mut nn = n;
	nn = ((nn & 0xAAAAAAAA) >> 1) | ((nn & 0x55555555) << 1);
	nn = ((nn & 0xCCCCCCCC) >> 2) | ((nn & 0x33333333) << 2);
	nn = ((nn & 0xF0F0F0F0) >> 4) | ((nn & 0x0F0F0F0F) << 4);
	nn = ((nn & 0xFF00FF00) >> 8) | ((nn & 0x00FF00FF) << 8);
	return (nn >> 16) | (nn << 16);
}


#[allow(dead_code)]
fn print_u8_slice(arr :&[u8]) {
	if arr.len() <= 4 {
		for a in arr {
			print!("0x{:02x} ", a);
		}
		println!("");
		return;
	}
	println!("[");
	let mut i :usize = 0;
	while i * 4 < arr.len() - 4 {
		println!("\t0x{:02x}, 0x{:02x}, 0x{:02x}, 0x{:02x},",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2], arr[i * 4 + 3]);
		i += 1;
	}
	match arr.len() as i64 - i as i64 * 4 {
		1 => println!("\t0x{:02x}];", arr[i * 4]),
		2 => println!("\t0x{:02x}, 0x{:02x}];", arr[i * 4], arr[i * 4 + 1]),
		3 => println!("\t0x{:02x}, 0x{:02x}, 0x{:02x}];",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2]),
		4 => println!("\t0x{:02x}, 0x{:02x}, 0x{:02x}, 0x{:02x}];",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2], arr[i * 4 + 3]),
		de => panic!("impossible value {}", de),
	}
}

#[allow(dead_code)]
fn print_u32_slice(arr :&[u32]) {
	if arr.len() <= 4 {
		for a in arr {
			print!("0x{:02x} ", a);
		}
		println!("");
		return;
	}
	println!("[");
	let mut i :usize = 0;
	while i * 4 < arr.len() - 4 {
		println!("\t0x{:08x}, 0x{:08x}, 0x{:08x}, 0x{:08x},",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2], arr[i * 4 + 3]);
		i += 1;
	}
	match arr.len() as i64 - i as i64 * 4 {
		1 => println!("\t0x{:08x}];", arr[i * 4]),
		2 => println!("\t0x{:08x}, 0x{:08x}];", arr[i * 4], arr[i * 4 + 1]),
		3 => println!("\t0x{:08x}, 0x{:08x}, 0x{:08x}];",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2]),
		4 => println!("\t0x{:08x}, 0x{:08x}, 0x{:08x}, 0x{:08x}];",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2], arr[i * 4 + 3]),
		de => panic!("impossible value {}", de),
	}
}


#[allow(dead_code)]
fn print_f64_slice(arr :&[f64]) {
	if arr.len() <= 4 {
		for a in arr {
			print!("0x{} ", a);
		}
		println!("");
		return;
	}
	println!("[");
	let mut i :usize = 0;
	while i * 4 < arr.len() - 4 {
		println!("\t{}, {}, {}, {},",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2], arr[i * 4 + 3]);
		i += 1;
	}
	match arr.len() as i64 - i as i64 * 4 {
		1 => println!("\t{}];", arr[i * 4]),
		2 => println!("\t{}, {}];", arr[i * 4], arr[i * 4 + 1]),
		3 => println!("\t{}, {}, {}];",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2]),
		4 => println!("\t{}, {}, {}, {}];",
				arr[i * 4], arr[i * 4 + 1], arr[i * 4 + 2], arr[i * 4 + 3]),
		de => panic!("impossible value {}", de),
	}
}
