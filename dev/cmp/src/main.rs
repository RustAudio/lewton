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

mod lib;
use lib::*;

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
	let file_path = env::args().nth(2).expect("Please specify a file to open via arg.");
	println!("Opening file: {}", file_path);
	let (decode_duration, native_decode_duration, n) = cmp_perf(&file_path);

	println!("Time to decode {} packets with lewton: {} s",
		n, get_duration_seconds(&decode_duration));
	println!("Ratio of difference: {:.2}x",
		get_duration_seconds(&decode_duration) /
		get_duration_seconds(&native_decode_duration));
}

fn run_vals() {
	let file_path = env::args().nth(2).expect("Please specify a file to open via arg.");
	println!("Opening file: {}", file_path);
	let (pcks_with_diffs, n) = cmp_output(&file_path);
	if pcks_with_diffs > 0 {
		println!("Total number of packets with differences: {} of {} ({}%)",
			pcks_with_diffs, n, pcks_with_diffs as f32 / n as f32 * 100.0);
	} else {
		println!("No differences found.");
	}
}
