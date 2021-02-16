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

use OggReadError;
use ogg::{PacketReader, Packet};
use header::*;
use VorbisError;
use std::io::{Read, Seek, SeekFrom};
use ::audio::{PreviousWindowRight, read_audio_packet,
	read_audio_packet_generic, get_decoded_sample_count};
use ::header::HeaderSet;
use ::samples::{Samples, InterleavedSamples};

/// Reads the three vorbis headers from an ogg stream as well as stream serial information
///
/// Please note that this function doesn't work well with async
/// I/O. In order to support this use case, enable the `async_ogg` feature,
/// and use the `HeadersReader` struct instead.
pub fn read_headers<'a, T: Read + 'a>(rdr: &mut PacketReader<T>) ->
		Result<(HeaderSet, u32), VorbisError> {
	let ident_packet = try!(rdr.read_packet_expected());
	let (headers, stream_serial, _stream_ends) =
		try!(read_headers_with_ident_packet(rdr, ident_packet));
	Ok((headers, stream_serial))
}

fn read_headers_with_ident_packet<'a, T>(
	mut rdr: &mut PacketReader<T>,
	ident_packet: Packet,
) -> Result<(HeaderSet, u32, bool), VorbisError>
where
	T: Read + 'a,
{
	let pck :Packet = ident_packet;
	let ident_hdr = try!(read_header_ident(&pck.data));
	let stream_serial = pck.stream_serial();

	let pck :Packet = try!(read_expected_packet_with_stream_serial(&mut rdr, stream_serial));
	let comment_hdr = try!(read_header_comment(&pck.data));

	let pck :Packet = try!(read_expected_packet_with_stream_serial(&mut rdr, stream_serial));
	let setup_hdr = try!(read_header_setup(&pck.data, ident_hdr.audio_channels,
		(ident_hdr.blocksize_0, ident_hdr.blocksize_1)));

	// The first audio packet must begin on a fresh page
	// TODO: do we really need this?
	rdr.delete_unread_packets();
	Ok((
		(ident_hdr, comment_hdr, setup_hdr),
		pck.stream_serial(),
		pck.last_in_stream(),
	))
}

fn read_expected_packet_with_stream_serial<R: Read>(
	reader :&mut PacketReader<R>, stream_serial: u32
) -> Result<Packet, VorbisError> {
	loop {
		let packet = try!(reader.read_packet_expected());
		if packet.stream_serial() == stream_serial {
			return Ok(packet);
		}
	}
}

/**
Reading an ogg/vorbis stream

This is a small helper struct to help reading an ogg/vorbis stream in that format.

It only supports the main use case of unmultiplexed, pure audio ogg files streams.
Reading a file where vorbis is only one of multiplexed streams, like in the case of ogv, is not supported.
(The packet that does not belong to the stream are skipped.)
If you need support for this, you need to use the lower level methods instead.

This struct only takes care of a single logical audio stream.
After reaching the end of a stream,
`read_dec_packet_*` functions do no longer return any audio,
even if there are another stream awaiting.
You can obtain another `OggStreamReader` via `` function, if any.
*/
pub struct OggStreamReader<T: Read> {
	rdr :PacketReader<T>,
	pwr :PreviousWindowRight,

	stream_serial :u32,

	ident_hdr :IdentHeader,
	comment_hdr :CommentHeader,
	setup_hdr :SetupHeader,

	state: ReaderState,
	skip_count: u64,
	start_absgp: u64,
	cur_absgp: u64,

	next_packet: Option<Packet>,
}

enum ReaderState {
	Processing,
	Finished,
}

impl<T: Read> OggStreamReader<T> {
	/// Constructs a new OggStreamReader from a given implementation of `Read`.
	///
	/// Please note that this function doesn't work well with async I/O.
	/// In order to support this use case, enable the `async_ogg` feature,
	/// and use the `HeadersReader` struct instead.
	pub fn new(rdr :T) -> Result<Self, VorbisError> {
		Self::from_ogg_reader(PacketReader::new(rdr))
	}

	/// Constructs a new OggStreamReader from a given Ogg PacketReader.
	///
	/// The `new` function is a nice wrapper around this function that
	/// also creates the ogg reader.
	///
	/// Please note that this function doesn't work well with async I/O.
	/// In order to support this use case, enable the `async_ogg` feature,
	/// and use the `HeadersReader` struct instead.
	pub fn from_ogg_reader(mut rdr :PacketReader<T>) -> Result<Self, VorbisError> {
		let ident_packet = try!(rdr.read_packet_expected());
		Self::from_ogg_reader_and_previous_packet(rdr, ident_packet, |_| {})
			.map(|x| x.0)
	}

	fn from_ogg_reader_and_previous_packet<F, R>(
		mut rdr: PacketReader<T>,
		ident_packet: Packet,
		mut after_reading_header: F,
	) -> Result<(Self, R), VorbisError>
	where
		F: FnMut(&mut PacketReader<T>) -> R,
	{
		let ((ident_hdr, comment_hdr, setup_hdr), stream_serial, no_more_packets) =
			try!(read_headers_with_ident_packet(&mut rdr, ident_packet));
		let after_reading_header = after_reading_header(&mut rdr);

		let mut reader = OggStreamReader {
			rdr,
			pwr: PreviousWindowRight::new(),
			ident_hdr,
			comment_hdr,
			setup_hdr,
			stream_serial,
			// The following fields will be overwritten for normal initialization
			// (for stream with no less than two packets)
			state: ReaderState::Finished,
			skip_count: 0,
			start_absgp: 0,
			cur_absgp: 0,
			next_packet: None,
		};

		// If there are less than two audio packets, we cannot obtain any samples.
		// TODO I am not sure this is compliant with spec
		if no_more_packets {
			// There are zero audio packets.
			return Ok((reader, after_reading_header));
		}
		let first_packet = try!(read_expected_packet_with_stream_serial(&mut reader.rdr, stream_serial));
		if first_packet.last_in_stream() {
			// There is one audio packet.
			return Ok((reader, after_reading_header));
		}
		// Decode the first packet into pwr.
		try!(read_audio_packet(&reader.ident_hdr, &reader.setup_hdr, &first_packet.data, &mut reader.pwr));
		// The second packet will actually be parsed later.
		try!(reader.load_second_audio_packet());

		Ok((reader, after_reading_header))
	}

	/// Read the second packet in a logical stream and adjust skip_count.
	/// The second packet must exist.
	fn load_second_audio_packet(&mut self) -> Result<(), VorbisError> {
		let second_packet = try!(read_expected_packet_with_stream_serial(&mut self.rdr, self.stream_serial));

		// The spec requires that the third audio packet will start in a fresh page,
		// and determine how many leading samples to drop.
		// However, some real-world ogg files does not seem to obey this.
		// In such case, we don't do such adjustment.
		if second_packet.last_in_page() {
			let second_packet_sample_count =
				try!(get_decoded_sample_count(&self.ident_hdr, &self.setup_hdr, &second_packet.data)) as u64;

			let skip_count = second_packet_sample_count.saturating_sub(second_packet.absgp_page());
			let start_absgp = second_packet.absgp_page().saturating_sub(second_packet_sample_count);
			assert_eq!(start_absgp + skip_count + second_packet_sample_count, second_packet.absgp_page());

			self.skip_count = skip_count;
			self.start_absgp = start_absgp;
			self.cur_absgp = start_absgp;
		}
		self.state = ReaderState::Processing;
		self.next_packet = Some(second_packet);

		Ok(())
	}

	/// Returns the wrapped reader, consuming the `OggStreamReader`.
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
	pub fn read_dec_packet_generic<S :Samples>(&mut self) -> Result<Option<S>, VorbisError> {
	// 	self.read_dec_packet_generic_debug(false)
	// }
	// pub fn read_dec_packet_generic_debug<S :Samples>(&mut self, debug: bool) -> Result<Option<S>, VorbisError> {
		if let ReaderState::Finished = self.state {
			return Ok(None);
		}

		let pck = if let Some(next_packet) = self.next_packet.take() {
			next_packet
		} else {
			try!(read_expected_packet_with_stream_serial(&mut self.rdr, self.stream_serial))
		};
		let mut decoded_pck :S = try!(read_audio_packet_generic(
				&self.ident_hdr, &self.setup_hdr, &pck.data, &mut self.pwr));

		// TODO the following comment is wrong
		// TODO If there was only two audio packets in a logical stream,
		// and the absgp for the second one is less than the number of samples retrieved from them,
		// then the current code truncate the beginning, not the ending.
		// But is it the correct way to do so?
		// I couldn't found any information about that in the spec.
		// Also this means that, if we want to indicate a positive initial absgp,
		// then we cannot truncate the trailing samepls,
		// thus cannot represent an audio whose number of samples are less than
		// short_block_length / 2.
		// Or can we?

		// TODO maybe we should remove the following comment
			// If this is the first packet in the logical stream,
			// we need to truncate the beginning so that its ending matches the absgp of the current page.

		// The leading samples are skipped after parsing the second audio packet for a logical stream,
		// or after seeking.
		let skip_count = self.skip_count.min(decoded_pck.num_samples() as u64);
		self.skip_count -= skip_count;
		decoded_pck.truncate_begin(skip_count as usize);
		// if debug { dbg!(skip_count); }

		if pck.last_in_stream() {
			if self.skip_count == 0 {
				// If this is the last packet in the logical bitstream,
				// we need to truncate it so that its ending matches the absgp of the current page.
				// This is what the spec mandates and also the behaviour of libvorbis.
				let truncate_size = (self.cur_absgp + decoded_pck.num_samples() as u64)
					.saturating_sub(pck.absgp_page());
				decoded_pck.truncate(truncate_size as usize);
				// if debug { dbg!(truncate_size); }
			}
			// If skip count is non-zero, then it means that a seek beyond

			self.state = ReaderState::Finished;
		}

		self.cur_absgp += decoded_pck.num_samples() as u64;
		if pck.last_in_page() {
			if self.cur_absgp != pck.absgp_page() {
				// Should we do something else?
				// At least, it is not a good idea to panic, since the input file is subject to corruption.
				// eprintln!("cur_absgp does not match.  Calculated: {}, provided: {}", self.cur_absgp, pck.absgp_page());
				self.cur_absgp = pck.absgp_page();
			}
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

	pub fn start_absgp(&self) -> u64 {
		self.start_absgp
	}
	/// Returns the absolute granule position of the last read packet.
	///
	/// In the case of ogg/vorbis,
	/// the absolute granule position is given as number of PCM samples, on a per channel basis
	/// (that is, "a stereo stream’s granule position does not increment at twice the speed of a mono stream").
	pub fn cur_absgp(&self) -> u64 {
		self.cur_absgp
	}

	pub fn ident_hdr(&self) -> &IdentHeader {
		&self.ident_hdr
	}

	pub fn comment_hdr(&self) -> &CommentHeader {
		&self.comment_hdr
	}

	pub fn setup_hdr(&self) -> &SetupHeader {
		&self.setup_hdr
	}
}

pub struct SeekableOggStreamReader<T: Read + Seek> {
	rdr: OggStreamReader<T>,
	stream_start_pos: u64,
	audio_packet_start_pos: u64,
	stream_end_pos: Option<u64>,
}

impl <T: Read + Seek> SeekableOggStreamReader<T> {
	pub fn new(mut rdr: T) -> Result<Self, VorbisError> {
		let map_io_error = |e| VorbisError::OggError(OggReadError::ReadError(e));

		let stream_start_pos = try!(rdr.seek(SeekFrom::Current(0)).map_err(map_io_error));
		let mut packet_reader = PacketReader::new(rdr);
		let ident_packet = try!(packet_reader.read_packet_expected());
		let (ogg_stream_reader, audio_strea_start_pos) = try!(OggStreamReader::from_ogg_reader_and_previous_packet(
			packet_reader,
			ident_packet,
			|rdr| rdr.seek_bytes(SeekFrom::Current(0)),
		));

		Ok(Self {
			rdr: ogg_stream_reader,
			stream_start_pos,
			audio_packet_start_pos: try!(audio_strea_start_pos.map_err(map_io_error)),
			stream_end_pos: None,
		})
	}

	pub fn inner(&self) -> &OggStreamReader<T> {
		&self.rdr
	}
	pub fn inner_mut(&mut self) -> &mut OggStreamReader<T> {
		&mut self.rdr
	}
	pub fn into_inner(self) -> OggStreamReader<T> {
		self.rdr
	}

	/// Seeks to the specified absolute granule position, with a sample granularity.
	///
	/// If the provided absgp was less than (or equal to) `start_absgp`,
	/// then it seeks to the beginning of the logical stream.
	///
	/// In the case of ogg/vorbis,
	/// the absolute granule position is given as number of PCM samples, on a per channel basis
	/// (that is, "a stereo stream’s granule position does not increment at twice the speed of a mono stream").
	///
	/// This function assumes unmultiplexed streams.
	pub fn seek_absgp(&mut self, absgp :u64) -> Result<(), VorbisError> {
		self.rdr.pwr = PreviousWindowRight::new();
		let search_range = self.audio_packet_start_pos..try!(self.stream_end_pos());
		let target_absgp = absgp.saturating_sub(1 << self.rdr.ident_hdr.blocksize_1);
		let seeked_absgp = try!(self.rdr.rdr.seek_absgp_new(
				target_absgp, Some(self.rdr.stream_serial), search_range));
		// dbg!(target_absgp, seeked_absgp);

		let first_packet = match try!(self.rdr.rdr.read_packet()).and_then(|packet| {
			if packet.stream_serial() == self.rdr.stream_serial {
				Some(packet)
			} else {
				// If the stream serial of the next packet does not match,
				// then we assume that that is the start of another stream.
				self.rdr.next_packet = Some(packet);
				None
			}
		}) {
			None => {
				// We seeked to the end of logical stream.
				self.rdr.state = ReaderState::Finished;
				if let Some(seeked_absgp) = seeked_absgp {
					// seeked_absgp is None only if the logical stream has not a single audio packet.
					self.rdr.cur_absgp = seeked_absgp;
				}
				return Ok(());
			}
			Some(packet) => packet
		};
		// Decode the first packet into pwr.
		try!(read_audio_packet(&self.rdr.ident_hdr, &self.rdr.setup_hdr, &first_packet.data, &mut self.rdr.pwr));
		let first_packet_sample_count =
			try!(get_decoded_sample_count(&self.rdr.ident_hdr, &self.rdr.setup_hdr, &first_packet.data)) as u64;

		self.rdr.state = ReaderState::Processing;
		self.rdr.cur_absgp = absgp;
		match seeked_absgp {
			Some(seeked_absgp) => {
				self.rdr.skip_count = absgp - seeked_absgp - first_packet_sample_count;
			}
			None => {
				try!(self.rdr.load_second_audio_packet());
				// In addition to discarding the first samples,
				// we should additionally seeek to the specified position.
				self.rdr.skip_count += absgp.saturating_sub(self.rdr.start_absgp);
			}
		};
		// dbg!(first_packet_sample_count, self.rdr.skip_count);

		Ok(())
	}

	fn stream_end_pos(&mut self) -> Result<u64, OggReadError> {
		let pos = match self.stream_end_pos {
			Some(pos) => pos,
			None => {
				try!(self.rdr.rdr.seek_bytes(SeekFrom::Start(self.stream_start_pos)));
				let pos = try!(self.rdr.rdr.find_end_of_logical_stream());
				let pos = pos.expect("There should be at least one packet after stream_start_pos");
				self.stream_end_pos = Some(pos);
				pos
			}
		};
		Ok(pos)
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
