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
use ::audio::{PreviousWindowRight, read_audio_packet,
	read_audio_packet_generic};
use ::header::HeaderSet;
use ::samples::{Samples, InterleavedSamples};

/// Reads the three vorbis headers from an ogg stream as well as stream serial information
///
/// Please note that this function doesn't work well with async
/// I/O. In order to support this use case, enable the `async_ogg` feature,
/// and use the `HeadersReader` struct instead.
pub fn read_headers<'a, T: Read + Seek + 'a>(rdr: &mut PacketReader<T>) ->
		Result<(HeaderSet, u32), VorbisError> {
	let pck :Packet = try!(rdr.read_packet_expected());
	let ident_hdr = try!(read_header_ident(&pck.data));
	let stream_serial = pck.stream_serial();

	let mut pck :Packet = try!(rdr.read_packet_expected());
	while pck.stream_serial() != stream_serial {
		pck = try!(rdr.read_packet_expected());
	}
	let comment_hdr = try!(read_header_comment(&pck.data));

	let mut pck :Packet = try!(rdr.read_packet_expected());
	while pck.stream_serial() != stream_serial {
		pck = try!(rdr.read_packet_expected());
	}
	let setup_hdr = try!(read_header_setup(&pck.data, ident_hdr.audio_channels,
		(ident_hdr.blocksize_0, ident_hdr.blocksize_1)));

	rdr.delete_unread_packets();
	return Ok(((ident_hdr, comment_hdr, setup_hdr), pck.stream_serial()));
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

	cur_absgp :Option<u64>,
}

impl<T: Read + Seek> OggStreamReader<T> {
	/// Constructs a new OggStreamReader from a given implementation of `Read + Seek`.
	///
	/// Please note that this function doesn't work well with async
	/// I/O. In order to support this use case, enable the `async_ogg` feature,
	/// and use the `HeadersReader` struct instead.
	pub fn new(rdr :T) ->
			Result<Self, VorbisError> {
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
			Result<Self, VorbisError> {
		let ((ident_hdr, comment_hdr, setup_hdr), stream_serial) =
			try!(read_headers(&mut rdr));
		return Ok(OggStreamReader {
			rdr,
			pwr : PreviousWindowRight::new(),
			ident_hdr,
			comment_hdr,
			setup_hdr,
			stream_serial,
			cur_absgp : None,
		});
	}
	pub fn into_inner(self) -> PacketReader<T> {
		self.rdr
	}
	fn read_next_audio_packet(&mut self) -> Result<Option<Packet>, VorbisError> {
		loop {
			let pck = match try!(self.rdr.read_packet()) {
				Some(p) => p,
				None => return Ok(None),
			};
			if pck.stream_serial() != self.stream_serial {
				if pck.first_in_stream() {
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
					self.stream_serial = pck.stream_serial();
					self.cur_absgp = None;

					// Now, read the first audio packet to prime the pwr
					// and discard the packet.
					let pck = match try!(self.rdr.read_packet()) {
						Some(p) => p,
						None => return Ok(None),
					};
					let _decoded_pck = try!(read_audio_packet(&self.ident_hdr,
						&self.setup_hdr, &pck.data, &mut self.pwr));
					self.cur_absgp = Some(pck.absgp_page());

					return Ok(try!(self.rdr.read_packet()));
				} else {
					// Ignore every packet that has a mismatching stream serial
				}
			} else {
				return Ok(Some(pck));
			}
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
		let pck = try!(self.read_dec_packet_generic());
		Ok(pck)
	}
	/// Reads and decompresses an audio packet from the stream (generic).
	///
	/// On read errors, it returns Err(e) with the error.
	///
	/// On success, it either returns None, when the end of the
	/// stream has been reached, or Some(packet_data),
	/// with the data of the decompressed packet.
	pub fn read_dec_packet_generic<S :Samples>(&mut self) ->
			Result<Option<S>, VorbisError> {
		let pck = match try!(self.read_next_audio_packet()) {
			Some(p) => p,
			None => return Ok(None),
		};
		let mut decoded_pck :S = try!(read_audio_packet_generic(&self.ident_hdr,
			&self.setup_hdr, &pck.data, &mut self.pwr));

		// If this is the last packet in the logical bitstream,
		// we need to truncate it so that its ending matches
		// the absgp of the current page.
		// This is what the spec mandates and also the behaviour
		// of libvorbis.
		if let (Some(absgp), true) = (self.cur_absgp, pck.last_in_stream()) {
			let target_length = pck.absgp_page().saturating_sub(absgp) as usize;
			decoded_pck.truncate(target_length);
		}
		if pck.last_in_page() {
			self.cur_absgp = Some(pck.absgp_page());
		} else if let &mut Some(ref mut absgp) = &mut self.cur_absgp {
			*absgp += decoded_pck.num_samples() as u64;
		}

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
		let decoded_pck :InterleavedSamples<_> = match try!(self.read_dec_packet_generic()) {
			Some(p) => p,
			None => return Ok(None),
		};
		return Ok(Some(decoded_pck.samples));
	}

	/// Returns the stream serial of the current stream
	///
	/// The stream serial can change in chained ogg files.
	pub fn stream_serial(&self) -> u32 {
		self.stream_serial
	}

	/// Returns the absolute granule position of the last read page.
	///
	/// In the case of ogg/vorbis, the absolute granule position is given
	/// as number of PCM samples, on a per channel basis.
	pub fn get_last_absgp(&self) -> Option<u64> {
		self.cur_absgp
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
		self.cur_absgp = None;
		self.pwr = PreviousWindowRight::new();
		Ok(())
	}
}

#[cfg(feature = "async_ogg")]
/**
Support for async I/O

This module provides support for asyncronous I/O.
*/
pub mod async_api {

	use super::*;
	use ogg::OggReadError;
	use ogg::reading::async_api::PacketReader;
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
			Ok(Async::Ready((ident_hdr, comment_hdr, setup_hdr)))
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
			self.absgp_of_last_read = Some(pck.absgp_page());
			Ok(Async::Ready(Some(decoded_pck)))
		}
	}
}
