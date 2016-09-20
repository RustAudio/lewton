// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

extern crate test_assets;
extern crate cmp;

use test_assets::TestAssetDef;

#[test]
fn test_vals() {
	let asset_defs = [
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

	];
	println!("");
	test_assets::download_test_files(&asset_defs,
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
	cmp_output!("bwv_1043_vivace.ogg", 197);
	cmp_output!("bwv_543_fuge.ogg", 7);
	cmp_output!("maple_leaf_rag.ogg", 5);
	cmp_output!("hoelle_rache.ogg", 281);
}
