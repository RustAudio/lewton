// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate lewton;
extern crate vorbis;
extern crate test_assets;

use std::env;

mod lib;
use lib::*;

fn main() {
	let command_name = env::args().nth(1).expect("No command found.");
	match command_name.as_ref() {
		"vals" =>  run_vals(), // Comparison of the output
		"perf" =>  run_perf(), // Comparison of speed
		"bench" =>  run_bench(), // Comparison of the output
		_ => println!("Error: invalid command.\n\
		Usage: <Command> <Filename>. Valid commands are \
		'vals' for output comparison, 'perf' for speed comparison, \
		and 'bench' for benchmarking a test file suite."),
	}
}

fn run_perf() {
	let file_path = env::args().nth(2).expect("Please specify a file to open via arg.");
	println!("Opening file: {}", file_path);
	let (decode_duration, native_decode_duration, n) = cmp_perf(&file_path);

	println!("Time to decode {} packets with libvorbis: {} s",
		n, native_decode_duration.as_secs_f64());
	println!("Time to decode {} packets with lewton: {} s",
		n, decode_duration.as_secs_f64());
	println!("Ratio of difference: {:.2}x",
		decode_duration.as_secs_f64() /
		native_decode_duration.as_secs_f64());
}

fn run_vals() {
	let file_path = env::args().nth(2).expect("Please specify a file to open via arg.");
	println!("Opening file: {}", file_path);
	let (pcks_with_diffs, n) = cmp_file_output(&file_path);
	if pcks_with_diffs > 0 {
		println!("Total number of packets with differences: {} of {} ({}%)",
			pcks_with_diffs, n, pcks_with_diffs as f32 / n as f32 * 100.0);
	} else {
		println!("No differences found.");
	}
}

fn run_bench() {
	println!("");
	test_assets::download_test_files(&get_asset_defs(),
		"test-assets", true).unwrap();
	println!("");
	use std::time::Duration;
	let mut total_native_time = Duration::from_secs(0);
	let mut total_time = Duration::from_secs(0);
	macro_rules! cmp_perf {
		($str:expr, $fill:expr) => {
			print!("Comparing speed for {} ", $str);
			let (decode_duration, native_decode_duration, _) =
				cmp_perf(&format!("test-assets/{}", $str));
			let ratio = decode_duration.as_secs_f64() /
				native_decode_duration.as_secs_f64();
			println!("{}: libvorbis={:.04}s we={:.4}s difference={:.2}x",
				$fill,
				native_decode_duration.as_secs_f64(),
				decode_duration.as_secs_f64(),
				ratio);
			total_native_time += native_decode_duration;
			total_time += decode_duration;
		};
	}
	cmp_perf!("bwv_1043_vivace.ogg", "");
	cmp_perf!("bwv_543_fuge.ogg", "   ");
	cmp_perf!("maple_leaf_rag.ogg", " ");
	cmp_perf!("hoelle_rache.ogg", "   ");
	cmp_perf!("thingy-floor0.ogg", "  ");
	println!("");
	println!("Overall time spent for decoding by libvorbis: {:.04}s",
		total_native_time.as_secs_f64());
	println!("Overall time spent for decoding by us: {:.04}s",
		total_time.as_secs_f64());
	println!("Overall ratio of difference: {:.2}x",
		total_time.as_secs_f64() /
		total_native_time.as_secs_f64());
}
