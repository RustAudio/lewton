// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate test_assets;
extern crate cmp;

#[test]
fn test_vals() {
	println!("");
	test_assets::download_test_files(&cmp::get_asset_defs(),
		"test-assets", true).unwrap();
	println!("");
	macro_rules! cmp_output {
		($str:expr, $max_diff:expr) => {
			print!("Comparing output for {} ", $str);
			let (diff_pck_count, _) = cmp::cmp_output(&format!("test-assets/{}", $str));
			println!(": {} differing packets of allowed {}.", diff_pck_count, $max_diff);
			assert!(diff_pck_count <= $max_diff);
		};
	}
	cmp_output!("bwv_1043_vivace.ogg", 0);
	cmp_output!("bwv_543_fuge.ogg", 0);
	cmp_output!("maple_leaf_rag.ogg", 0);
	cmp_output!("hoelle_rache.ogg", 0);
	cmp_output!("thingy-floor0.ogg", 1);
}
