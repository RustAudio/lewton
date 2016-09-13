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
