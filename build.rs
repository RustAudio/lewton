#[cfg(feature = "bindgen")]
extern crate cbindgen;
#[cfg(feature = "bindgen")]
use std::env;
#[cfg(feature = "bindgen")]
use std::path::PathBuf;

#[cfg(feature = "bindgen")]
fn build_c_bindings() {
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
	let config = cbindgen::Config::from_file("cbindgen.toml").unwrap();
	// Generate bindings to target/BUILD_PROFILE/lewton.h
	cbindgen::generate_with_config(&crate_dir, config)
		.unwrap()
		.write_to_file(out_dir.join("../../../lewton.h"));
}

fn main() {
	#[cfg(feature = "bindgen")]
	build_c_bindings();
}
