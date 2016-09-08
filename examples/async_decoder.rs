// Vorbis decoder written in Rust
//
// This example file is licensed
// under the CC-0 license:
// https://creativecommons.org/publicdomain/zero/1.0/

extern crate lewton;
extern crate byteorder;
extern crate ogg;
extern crate rand;

#[cfg(not(feature = "async_ogg"))]
fn main() {
	panic!("This example requires the async_ogg feature to do something meaningful.");
}

#[cfg(feature = "async_ogg")]
fn main() {
	match stuff::run() {
		Ok(_) =>(),
		Err(err) => println!("Error: {}", err),
	}
}

#[cfg(feature = "async_ogg")]
mod stuff {
	use std::env;
	use lewton::VorbisError;
	use ogg::{PacketReader, OggReadError, BufReader as OggBufReader};
	use lewton::inside_ogg::HeadersReader;
	use std::fs::File;
	use std::time::Instant;
	use std::io;

	struct RandomWouldBlock<T>(T);
	impl <T: io::Read> io::Read for RandomWouldBlock<T> {
		fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
			if ::rand::random() {
				return Err(io::Error::new(io::ErrorKind::WouldBlock, "would block"));
			}
			self.0.read(buf)
		}
	}

	impl <T: io::Seek> io::Seek for RandomWouldBlock<T> {
		fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
			if ::rand::random() {
				return Err(io::Error::new(io::ErrorKind::WouldBlock, "would block"));
			}
			self.0.seek(pos)
		}
	}

	macro_rules! continue_trying {
		($e:expr) => {
			(|| {
				loop {
					match $e {
						Ok(val) => return Ok(val),
						Err(VorbisError::OggError(OggReadError::ReadError(ref err)))
							if err.kind() == io::ErrorKind::WouldBlock => (),
						Err(err) => return Err(err),
					}
				}
			}) ()
		}
	}

	pub fn run() -> Result<(), VorbisError> {
		let file_path = env::args().nth(1).expect("No arg found. Please specify a file to open.");
		println!("Opening file: {}", file_path);
		let mut f = RandomWouldBlock(try!(File::open(file_path)));
		let mut br = OggBufReader::new(&mut f);
		let mut pck_rdr = PacketReader::new(&mut br);

		let mut hrdr = HeadersReader::new(&mut pck_rdr);
		try!(continue_trying!(hrdr.try_read_headers()));
		let mut srr = hrdr.into_ogg_stream_reader();

		println!("Sample rate: {}", srr.ident_hdr.audio_sample_rate);

		// Now the fun starts..
		let mut n = 0;
		let mut len_play = 0.0;
		let start_decode_time = Instant::now();
		loop {
			use std::io::ErrorKind;
			let pck = match srr.read_decompressed_packet() {
				Ok(p) => p,
				Err(VorbisError::OggError(OggReadError::ReadError(ref e)))
					if e.kind() == ErrorKind::UnexpectedEof => {
					println!("Seems to be the end."); break; },
				Err(VorbisError::OggError(OggReadError::ReadError(ref e)))
					if e.kind() == ErrorKind::WouldBlock => continue,
				Err(e) => {
					panic!("OGG stream decode failure: {}", e);
				},
			};
			n += 1;
			// This is guaranteed by the docs
			assert_eq!(pck.0.len(), srr.ident_hdr.audio_channels as usize);
			len_play += pck.0[0].len() as f32 / srr.ident_hdr.audio_sample_rate as f32;
		}
		let decode_duration = Instant::now() - start_decode_time;
		println!("The piece is {} s long ({} packets).", len_play, n);
		println!("Decoded in {} s.", decode_duration.as_secs() as f64 + (decode_duration.subsec_nanos() as f64) / 1_000_000_000.0);

		Ok(())
	}
}
