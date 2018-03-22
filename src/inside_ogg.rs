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

/// Reads the three vorbis headers from an ogg stream as well as stream serial information
///
/// Please note that this function doesn't work well with async
/// I/O. In order to support this use case, enable the `async_ogg` feature,
/// and use the `HeadersReader` struct instead.
pub fn read_headers<'a, T: Read + Seek + 'a>(rdr: &mut PacketReader<T>) ->
		Result<(HeaderSet, u32), VorbisError> {
	let pck :Packet = try!(rdr.read_packet_expected());
	let ident_hdr = try!(read_header_ident(&pck.data));

	let pck :Packet = try!(rdr.read_packet_expected());
	let comment_hdr = try!(read_header_comment(&pck.data));

	let pck :Packet = try!(rdr.read_packet_expected());
	let setup_hdr = try!(read_header_setup(&pck.data, ident_hdr.audio_channels,
		(ident_hdr.blocksize_0, ident_hdr.blocksize_1)));

	return Ok(((ident_hdr, comment_hdr, setup_hdr), pck.stream_serial));
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

	stream_serial :u32,

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
		let ((ident_hdr, comment_hdr, setup_hdr), stream_serial) =
			try!(read_headers(&mut rdr));
		return Ok(OggStreamReader {
			rdr,
			pwr : PreviousWindowRight::new(),
			ident_hdr,
			comment_hdr,
			setup_hdr,
			stream_serial,
			absgp_of_last_read : None,
		});
	}
	pub fn into_inner(self) -> PacketReader<T> {
		self.rdr
	}
	fn read_next_audio_packet(&mut self) -> Result<Option<Packet>, VorbisError> {
		let pck = match try!(self.rdr.read_packet()) {
			Some(p) => p,
			None => return Ok(None),
		};
		if pck.stream_serial != self.stream_serial {
			if pck.first_packet {
				// We have a chained ogg file. This means we need to
				// re-initialize the internal context.
				let ident_hdr = try!(read_header_ident(&pck.data));

				let pck :Packet = try!(self.rdr.read_packet_expected());
				let comment_hdr = try!(read_header_comment(&pck.data));

				let pck :Packet = try!(self.rdr.read_packet_expected());
				let setup_hdr = try!(read_header_setup(&pck.data, ident_hdr.audio_channels,
					(ident_hdr.blocksize_0, ident_hdr.blocksize_1)));

				// Update the context
				self.pwr = PreviousWindowRight::new();
				self.ident_hdr = ident_hdr;
				self.comment_hdr = comment_hdr;
				self.setup_hdr = setup_hdr;
				self.stream_serial = pck.stream_serial;
				self.absgp_of_last_read = None;

				// Now, read the first audio packet to prime the pwr
				// and discard the packet.
				let pck = match try!(self.rdr.read_packet()) {
					Some(p) => p,
					None => return Ok(None),
				};
				let decoded_pck = try!(read_audio_packet(&self.ident_hdr,
					&self.setup_hdr, &pck.data, &mut self.pwr));
				self.absgp_of_last_read = Some(pck.absgp_page);

				return Ok(try!(self.rdr.read_packet()));
			} else {
				// TODO make this a proper error case
				// We most likely got here due to seeking or an invalid file
				return Ok(Some(pck));
			}
		} else {
			return Ok(Some(pck));
		}
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
		let pck = match try!(self.read_next_audio_packet()) {
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
		let decoded_pck = match try!(self.read_dec_packet()) {
			Some(p) => p,
			None => return Ok(None),
		};
		// Interleave
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
/**
Support for async I/O

This module provides support for asyncronous I/O.
*/
pub mod async {

	use super::*;
	use ogg::OggReadError;
	use ogg::reading::async::PacketReader;
	use futures::stream::Stream;
	use tokio_io::AsyncRead;
	use futures::{Async, Future, Poll};
	use std::io::{Error, ErrorKind};
	use std::mem::replace;

	/// Async ready creator utility to read headers out of an
	/// ogg stream.
	///
	/// All functions this struct has are ready to be used for operation with async I/O.
	pub struct HeadersReader<T: AsyncRead> {
		pck_rd :PacketReader<T>,
		ident_hdr :Option<IdentHeader>,
		comment_hdr :Option<CommentHeader>,
	}
	impl<T: AsyncRead> HeadersReader<T> {
		pub fn new(inner :T) -> Self {
			HeadersReader::from_packet_reader(PacketReader::new(inner))
		}
		pub fn from_packet_reader(pck_rd :PacketReader<T>) -> Self {
			HeadersReader {
				pck_rd,
				ident_hdr : None,
				comment_hdr : None,
			}
		}
	}
	impl<T: AsyncRead> Future for HeadersReader<T> {
		type Item = HeaderSet;
		type Error = VorbisError;
		fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
			macro_rules! rd_pck {
				() => {
					if let Some(pck) = try_ready!(self.pck_rd.poll()) {
						pck
					} else {
						// Note: we are stealing the Io variant from
						// the ogg crate here which is not 100% clean,
						// but I think in general it is what the
						// read_packet_expected function of the ogg
						// crate does too, and adding our own case
						// to the VorbisError enum that only fires
						// in an async mode is too complicated IMO.
						try!(Err(OggReadError::ReadError(Error::new(ErrorKind::UnexpectedEof,
							"Expected header packet but found end of stream"))))
					}
				}
			}
			if self.ident_hdr.is_none() {
				let pck = rd_pck!();
				self.ident_hdr = Some(try!(read_header_ident(&pck.data)));
			}
			if self.comment_hdr.is_none() {
				let pck = rd_pck!();
				self.comment_hdr = Some(try!(read_header_comment(&pck.data)));
			}
			let setup_hdr = {
				let ident = self.ident_hdr.as_ref().unwrap();
				let pck = rd_pck!();
				try!(read_header_setup(&pck.data,
					ident.audio_channels, (ident.blocksize_0, ident.blocksize_1)))
			};
			let ident_hdr = replace(&mut self.ident_hdr, None).unwrap();
			let comment_hdr = replace(&mut self.comment_hdr, None).unwrap();
			Ok(Async::Ready(((ident_hdr, comment_hdr, setup_hdr))))
		}
	}
	/// Reading ogg/vorbis files or streams
	///
	/// This is a small helper struct to help reading ogg/vorbis files
	/// or streams in that format.
	///
	/// It only supports the main use case of pure audio ogg files streams.
	/// Reading a file where vorbis is only one of multiple streams, like
	/// in the case of ogv, is not supported.
	///
	/// If you need support for this, you need to use the lower level methods
	/// instead.
	pub struct OggStreamReader<T :AsyncRead> {
		pck_rd :PacketReader<T>,
		pwr :PreviousWindowRight,

		pub ident_hdr :IdentHeader,
		pub comment_hdr :CommentHeader,
		pub setup_hdr :SetupHeader,

		absgp_of_last_read :Option<u64>,
	}

	impl<T :AsyncRead> OggStreamReader<T> {
		/// Creates a new OggStreamReader from the given parameters
		pub fn new(hdr_rdr :HeadersReader<T>, hdrs :HeaderSet) -> Self {
			OggStreamReader::from_pck_rdr(hdr_rdr.pck_rd, hdrs)
		}
		/// Creates a new OggStreamReader from the given parameters
		pub fn from_pck_rdr(pck_rd :PacketReader<T>, hdrs :HeaderSet) -> Self {
			OggStreamReader {
				pck_rd,
				pwr : PreviousWindowRight::new(),

				ident_hdr : hdrs.0,
				comment_hdr : hdrs.1,
				setup_hdr : hdrs.2,

				absgp_of_last_read : None,
			}
		}
	}

	impl<T :AsyncRead> Stream for OggStreamReader<T> {
		type Item = Vec<Vec<i16>>;
		type Error = VorbisError;

		fn poll(&mut self) -> Poll<Option<Vec<Vec<i16>>>, VorbisError> {
			let pck = match try_ready!(self.pck_rd.poll()) {
				Some(p) => p,
				None => return Ok(Async::Ready(None)),
			};
			let decoded_pck = try!(read_audio_packet(&self.ident_hdr,
				&self.setup_hdr, &pck.data, &mut self.pwr));
			self.absgp_of_last_read = Some(pck.absgp_page);
			Ok(Async::Ready(Some(decoded_pck)))
		}
	}
}
