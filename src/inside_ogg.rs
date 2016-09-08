// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

/*!
Higher-level utilities for Ogg streams and files

This module provides higher level access to the library functionality,
and useful helper methods for the Ogg `PacketReader` struct.
*/

use ogg::{PacketReader, Packet};
use header::*;
use VorbisError;
use std::io::{Read, Seek};
use ::audio::{PreviousWindowRight, read_audio_packet};

/// Reads the three vorbis headers from an ogg stream
///
/// Please note that this function doesn't work well with async
/// I/O. In order to support this use case, enable the `async_ogg` feature,
/// and use the `HeadersReader` struct instead.
pub fn read_headers<'a, T: Read + Seek + 'a>(rdr: &mut PacketReader<T>) ->
		Result<(IdentHeader, CommentHeader, SetupHeader), VorbisError> {
	let pck :Packet = try!(rdr.read_packet());
	let ident_hdr = try!(read_header_ident(&pck.data));

	let pck :Packet = try!(rdr.read_packet());
	let comment_hdr = try!(read_header_comment(&pck.data));

	let pck :Packet = try!(rdr.read_packet());
	let setup_hdr = try!(read_header_setup(&pck.data, ident_hdr.audio_channels,
		(ident_hdr.blocksize_0, ident_hdr.blocksize_1)));

	return Ok((ident_hdr, comment_hdr, setup_hdr));
}

/**
Reading ogg/vorbis files or streams

This is a small helper struct to help reading ogg/vorbis files
or streams in that format.

It only supports the main use case of pure audio ogg files streams.
Reading a file where vorbis is only one of multiple streams, like
in the case of ogv, is not supported.

If you need support for this, you need to use the lower level methods
instead.
*/
pub struct OggStreamReader<'a, T: Read + Seek + 'a> {
	rdr :&'a mut PacketReader<'a, T>,
	pwr :PreviousWindowRight,

	pub ident_hdr :IdentHeader,
	pub comment_hdr :CommentHeader,
	pub setup_hdr :SetupHeader,
}

impl<'a, T: Read + Seek + 'a> OggStreamReader<'a, T> {
	/// Constructs a new OggStreamReader from a given PacketReader.
	///
	/// Please note that this function doesn't work well with async
	/// I/O. In order to support this use case, enable the `async_ogg` feature,
	/// and use the `HeadersReader` struct instead.
	pub fn new(rdr :&'a mut PacketReader<'a, T>) ->
			Result<OggStreamReader<'a, T>, VorbisError> {
		let (ident_hdr, comment_hdr, setup_hdr) = try!(read_headers(rdr));
		return Ok(OggStreamReader {
			rdr : rdr,
			pwr : PreviousWindowRight::new(),
			ident_hdr : ident_hdr,
			comment_hdr : comment_hdr,
			setup_hdr : setup_hdr,
		});
	}
	/// Reads and decompresses an audio packet from the stream.
	pub fn read_decompressed_packet(&mut self) ->
			Result<(Vec<Vec<i16>>, usize), VorbisError> {
		let pck = try!(self.rdr.read_packet());
		let pck_len = pck.data.len();
		return Ok((try!(read_audio_packet(&self.ident_hdr,
			&self.setup_hdr, &pck.data, &mut self.pwr)), pck_len));
	}
}


#[cfg(feature = "async_ogg")]
mod async_utils {

	use ogg::{AdvanceAndSeekBack, Packet};
	use ::inside_ogg::OggStreamReader;
	use std::io::{Read, Seek};
	use header::*;
	use VorbisError;
	use ogg::PacketReader;
	use ::audio::PreviousWindowRight;

	/// Async ready creator utility to read headers out of an
	/// ogg stream.
	///
	/// This struct is async ready, meaning that it keeps its
	/// internal state consistent even if some calls to underlying
	/// read result with non fatal errors like the `WouldBlock` error
	/// kind.
	///
	/// This allows trivial wrapping with your favourite async framework.
	///
	/// All functions this struct has are ready to be used for operation with async I/O.
	pub struct HeadersReader<'a, T: Read + Seek + AdvanceAndSeekBack + 'a> {
		rdr :&'a mut PacketReader<'a, T>,

		ident_hdr :Option<IdentHeader>,
		comment_hdr :Option<CommentHeader>,
		setup_hdr :Option<SetupHeader>,
	}

	impl <'a, T: Read + Seek + AdvanceAndSeekBack + 'a> HeadersReader<'a, T> {
		pub fn new(rdr :&'a mut PacketReader<'a, T>) ->
				HeadersReader<'a, T> {
			return HeadersReader {
				rdr : rdr,
				ident_hdr : None,
				comment_hdr : None,
				setup_hdr : None,
			};
		}
		/// Tries to advance the header read process
		///
		/// Call this function to try to advance the header read process.
		/// Once it returns `Ok(())`, the header reading is done. After that
		/// you may call the into_ functions.
		///
		/// This function is async-ready, meaning that it will keep the internal
		/// state consistent, and pass through any WouldBlock error kind errors.
		pub fn try_read_headers(&mut self) -> Result<(), VorbisError> {
			if self.ident_hdr.is_none() {
				let pck :Packet = try!(self.rdr.read_packet());
				self.ident_hdr = Some(try!(read_header_ident(&pck.data)));
			}
			if self.comment_hdr.is_none() {
				let pck :Packet = try!(self.rdr.read_packet());
				self.comment_hdr = Some(try!(read_header_comment(&pck.data)));
			}
			if self.setup_hdr.is_none() {
				let pck :Packet = try!(self.rdr.read_packet());
				let ident_hdr = self.ident_hdr.as_ref().unwrap();
				self.setup_hdr = Some(try!(read_header_setup(&pck.data,
					ident_hdr.audio_channels, (ident_hdr.blocksize_0, ident_hdr.blocksize_1))));
			}
			return Ok(());
		}

		/// Initializes an OggStreamReader with the headers that have been read
		///
		/// Panics if the header reading process is not finished yet.
		pub fn into_ogg_stream_reader(mut self) -> OggStreamReader<'a, T> {
			return OggStreamReader {
				rdr : self.rdr,
				pwr : PreviousWindowRight::new(),
				ident_hdr : self.ident_hdr.unwrap(),
				comment_hdr : self.comment_hdr.unwrap(),
				setup_hdr : self.setup_hdr.unwrap(),
			};
		}
		/// Returns the headers that have been read
		///
		/// Panics if the header reading process is not finished yet.
		pub fn into_header_triple(self)
				-> (IdentHeader, CommentHeader, SetupHeader) {
			return (self.ident_hdr.unwrap(), self.comment_hdr.unwrap(), self.setup_hdr.unwrap());
		}
	}
}

#[cfg(feature = "async_ogg")]
pub use self::async_utils::HeadersReader as HeadersReader;
