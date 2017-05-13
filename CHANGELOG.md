# Changes

## Release 0.1 - September 1, 2016

Initial release.

## Release 0.2 - September 13, 2016

* Improved speed by about 20%.
* Added async ready API to the `inside_ogg` module to work with async IO.
  Still behind a feature as it relies on the unstable [specialisation feature](https://github.com/rust-lang/rust/issues/31844).
* Removed parts of the API that were irrelevant to users of the crate.
  This gives a better overview for our users.
  Unfortunately due to [pub(crate) not being stable yet](https://github.com/rust-lang/rust/issues/32409),
  not all parts of the API could have been made private.
* Examples are CC-0 now, this should ease adoption.
* Documentation improvements
* Implemented a tool to compare our speed and output with libvorbis.
  To see how correct this crate is, cd to `dev/cmp` and do `cargo run --release vals /path/to/test_file.ogg`.
  For speed tests, swap "vals" with "perf".

## Release 0.3 - October 4, 2016

* Added support for floor 0. It is not used in practice anymore,
  but now all features of the vorbis format are supported.
* Improved the API for reading decoded packets.
* Fixed a bug in comment header parsing.
* Various minor simplifications.
* Improved the cmp tool. You can now compare our output to libvorbis
  with `cargo test --release -- --nocapture`,
  and our speed with `cargo run --release bench`.

## Release 0.4 - October 4, 2016

* Updated ogg.
* Made the `inside_ogg` API own the reader.

## Release 0.4.1 - November 17, 2016

* Fixed a panic issue with reading huffman trees.

## Release 0.5 - February 15, 2017

* New, more convenient, constructor for OggStreamReader.
* Updated to Byteorder 1.0.

## Release 0.5.1 - April 30, 2017

* Bugfix to work on newest Rust nightly/beta
* Bugfix to work with the alto crate instead of openal-rs which has been yanked
* Bugfix in the player example for duration calculation

## Release 0.5.2 - May 13, 2017

* Removed two unused macros to prevent warnings about them
