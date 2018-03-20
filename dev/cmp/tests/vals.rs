// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate test_assets;
extern crate cmp;

macro_rules! cmp_output {
	($str:expr, $max_diff:expr) => {
		print!("Comparing output for {} ", $str);
		let (diff_pck_count, _) = cmp::cmp_output(&format!("test-assets/{}", $str));
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
	cmp_output!("thingy-floor0.ogg", 1);
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
	// TODO fix these
	//cmp_output!("chain-test1.ogg", 1);
	//cmp_output!("chain-test2.ogg", 1);
	//cmp_output!("chain-test3.ogg", 1);
	cmp_output!("highrate-test.ogg", 0);
}

#[test]
fn test_xiph_vals_3() {
	println!();
	test_assets::download_test_files(&cmp::get_xiph_asset_defs_3(),
		"test-assets", true).unwrap();
	println!();

	cmp_output!("lsp-test.ogg", 0);
	cmp_output!("lsp-test2.ogg", 2);
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

	// TODO fix the commented out test
	//cmp_output!("singlemap-test.ogg", 0);
	cmp_output!("sleepzor.ogg", 9);
	// TODO fix this test as well
	cmp_output!("test-short.ogg", 69);
	cmp_output!("test-short2.ogg", 0);
	// TODO fix the commented out test
	//cmp_output!("unused-mode-test.ogg", 0);
}
