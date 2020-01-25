# lewton

[![docs](https://docs.rs/lewton/badge.svg)](https://docs.rs/crate/lewton)
[![crates.io](https://img.shields.io/crates/v/lewton.svg)](https://crates.io/crates/lewton)
[![dependency status](https://deps.rs/repo/github/rustaudio/lewton/status.svg)](https://deps.rs/repo/github/rustaudio/lewton)

Vorbis decoder written in pure Rust.

To give the decoder a try, you can do:
```sh
cargo run --example player /path/to/your/audio_file.ogg
```
It will then play back the audio.

If you want to know how to use this crate, look at the examples folder.

This crate has a low level API for per-packet decoding in the `audio` and `header` modules,
and a high level API for ogg/vorbis streams in the `inside_ogg` module.

Some parts were created with help from the public domain
[stb_vorbis](http://nothings.org/stb_vorbis/) decoder implementation.

The minimum required Rust version is 1.36.

## Use of unsafe

The entire library uses not a single line of unsafe code.
In fact, lib.rs contains the `#![forbid(unsafe_code)]` directive.

## About the history of this crate

I've started started to work on this crate in December 2015.
The goal was to learn more about Rust and audio processing,
while also delivering something useful to the Rust ecosystem.

I've tried not to look into the libvorbis implementation,
as then I'd have to first abide the BSD license, and second
as I didn't want this crate to become "just" a translation
from C to Rust. Instead I wanted this crate to base on the
spec only, so that I'd learn more about how vorbis worked.

The only time I did look into the libvorbis implementation
was to look up the implementation of a function needed by
the ogg crate (the CRC function), which is why that crate
is BSD licensed, and attributes the authors.
This crate however contains no code, translated or
otherwise, from the libvorbis implementation.

After some time I realized that without any help of a working
implementation progress would become too slow.

Therefore, I've continued to work with the public domain
`stb_vorbis` implementation and used some of its
code (most prominently for the imdct algorithm) to
translate it to rust. I've also used it for debugging by
comparing its outputs with mine.

Most of this crate however was created by reading the spec
only.

## Use from C
**lewton** provides a C-compatible set of library, header and pkg-config file.

To build and install it you can use [cargo-c](https://crates.io/crates/cargo-c):

```sh
cargo install cargo-c
cargo cinstall --release --destdir /tmp/lewton
sudo cp -a /tmp/lewton/* /
```

## License

Licensed under Apache 2 or MIT (at your option). For details, see the [LICENSE](LICENSE) file.

All examples inside the `examples/` folder are licensed under the
[CC-0](https://creativecommons.org/publicdomain/zero/1.0/) license.

### License of your contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license,
shall be dual licensed / CC-0 licensed as above, without any additional terms or conditions.
