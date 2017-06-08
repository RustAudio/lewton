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
use ::header::HeaderSet;

/// Reads the three vorbis headers from an ogg stream
///
/// Please note that this function doesn't work well with async
/// I/O. In order to support this use case, enable the `async_ogg` feature,
/// and use the `HeadersReader` struct instead.
pub fn read_headers<'a, T: Read + Seek + 'a>(rdr: &mut PacketReader<T>) ->
		Result<HeaderSet, VorbisError> {
	let pck :Packet = try!(rdr.read_packet_expected());
	let ident_hdr = try!(read_header_ident(&pck.data));

	let pck :Packet = try!(rdr.read_packet_expected());
	let comment_hdr = try!(read_header_comment(&pck.data));

	let pck :Packet = try!(rdr.read_packet_expected());
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
pub struct OggStreamReader<T: Read + Seek> {
	rdr :PacketReader<T>,
	pwr :PreviousWindowRight,

	pub ident_hdr :IdentHeader,
	pub comment_hdr :CommentHeader,
	pub setup_hdr :SetupHeader,

	absgp_of_last_read :Option<u64>,
}

impl<T: Read + Seek> OggStreamReader<T> {
	/// Constructs a new OggStreamReader from a given implementation of `Read + Seek`.
	///
	/// Please note that this function doesn't work well with async
	/// I/O. In order to support this use case, enable the `async_ogg` feature,
	/// and use the `HeadersReader` struct instead.
	pub fn new(rdr :T) ->
			Result<OggStreamReader<T>, VorbisError> {
		OggStreamReader::from_ogg_reader(PacketReader::new(rdr))
	}
	/// Constructs a new OggStreamReader from a given Ogg PacketReader.
	///
	/// The `new` function is a nice wrapper around this function that
	/// also creates the ogg reader.
	///
	/// Please note that this function doesn't work well with async
	/// I/O. In order to support this use case, enable the `async_ogg` feature,
	/// and use the `HeadersReader` struct instead.
	pub fn from_ogg_reader(mut rdr :PacketReader<T>) ->
			Result<OggStreamReader<T>, VorbisError> {
		let (ident_hdr, comment_hdr, setup_hdr) = try!(read_headers(&mut rdr));
		return Ok(OggStreamReader {
			rdr : rdr,
			pwr : PreviousWindowRight::new(),
			ident_hdr : ident_hdr,
			comment_hdr : comment_hdr,
			setup_hdr : setup_hdr,
			absgp_of_last_read : None,
		});
	}
	pub fn into_inner(self) -> PacketReader<T> {
		self.rdr
	}
	/// Reads and decompresses an audio packet from the stream.
	///
	/// On read errors, it returns Err(e) with the error.
	///
	/// On success, it either returns None, when the end of the
	/// stream has been reached, or Some(packet_data),
	/// with the data of the decompressed packet.
	pub fn read_dec_packet(&mut self) ->
			Result<Option<Vec<Vec<i16>>>, VorbisError> {
		let pck = match try!(self.rdr.read_packet()) {
			Some(p) => p,
			None => return Ok(None),
		};
		let decoded_pck = try!(read_audio_packet(&self.ident_hdr,
			&self.setup_hdr, &pck.data, &mut self.pwr));
		self.absgp_of_last_read = Some(pck.absgp_page);
		return Ok(Some(decoded_pck));
	}
	/// Reads and decompresses an audio packet from the stream (interleaved).
	///
	/// On read errors, it returns Err(e) with the error.
	///
	/// On success, it either returns None, when the end of the
	/// stream has been reached, or Some(packet_data),
	/// with the data of the decompressed packet.
	///
	/// Unlike `read_dec_packet`, this function returns the
	/// interleaved samples.
	pub fn read_dec_packet_itl(&mut self) ->
			Result<Option<Vec<i16>>, VorbisError> {
		let pck = match try!(self.rdr.read_packet()) {
			Some(p) => p,
			None => return Ok(None),
		};
		let decoded_pck = try!(read_audio_packet(&self.ident_hdr,
			&self.setup_hdr, &pck.data, &mut self.pwr));
		self.absgp_of_last_read = Some(pck.absgp_page);
		// Now interleave
		// TODO make int sample generation and
		// interleaving one step.
		let channel_count = decoded_pck.len();
		// Note that a channel count of 0 is forbidden
		// by the spec and the header decoding code already
		// checks for that.
		let samples_interleaved = if channel_count == 1 {
			// Because decoded_pck[0] doesn't work...
			decoded_pck.into_iter().next().unwrap()
		} else {
			let len = decoded_pck[0].len();
			let mut samples = Vec::with_capacity(len * channel_count);
			for i in 0 .. len {
				for ref chan in decoded_pck.iter() {
					samples.push(chan[i]);
				}
			}
			samples
		};
		return Ok(Some(samples_interleaved));
	}

	/// Returns the absolute granule position of the last read page.
	///
	/// In the case of ogg/vorbis, the absolute granule position is given
	/// as number of PCM samples, on a per channel basis.
	pub fn get_last_absgp(&self) -> Option<u64> {
		self.absgp_of_last_read
	}

	/// Seeks to the specified absolute granule position, with a page granularity.
	///
	/// The granularity is per-page, and the obtained position is
	/// then <= the seeked absgp.
	///
	/// In the case of ogg/vorbis, the absolute granule position is given
	/// as number of PCM samples, on a per channel basis.
	pub fn seek_absgp_pg(&mut self, absgp :u64) -> Result<(), VorbisError> {
		try!(self.rdr.seek_absgp(None, absgp));
		// Reset the internal state after the seek
		self.absgp_of_last_read = None;
		self.pwr = PreviousWindowRight::new();
		Ok(())
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
	pub struct HeadersReader<T: Read + Seek + AdvanceAndSeekBack> {
		rdr :PacketReader<T>,

		ident_hdr :Option<IdentHeader>,
		comment_hdr :Option<CommentHeader>,
		setup_hdr :Option<SetupHeader>,
	}

	impl <T: Read + Seek + AdvanceAndSeekBack> HeadersReader<T> {
		pub fn new(rdr :PacketReader<T>) -> Self {
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
		pub fn into_ogg_stream_reader(self) -> OggStreamReader<T> {
			return OggStreamReader {
				rdr : self.rdr,
				pwr : PreviousWindowRight::new(),
				ident_hdr : self.ident_hdr.unwrap(),
				comment_hdr : self.comment_hdr.unwrap(),
				setup_hdr : self.setup_hdr.unwrap(),
				absgp_of_last_read : None,
			};
		}
		/// Returns the headers that have been read
		///
		/// Panics if the header reading process is not finished yet.
		pub fn into_header_triple(self)
				-> (IdentHeader, CommentHeader, SetupHeader) {
			return (self.ident_hdr.unwrap(), self.comment_hdr.unwrap(), self.setup_hdr.unwrap());
		}

		pub fn into_inner(self) -> PacketReader<T> {
			return self.rdr;
		}
	}
}

#[cfg(feature = "async_ogg")]
pub use self::async_utils::HeadersReader as HeadersReader;
