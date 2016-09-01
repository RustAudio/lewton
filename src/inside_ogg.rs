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

pub fn read_headers<'a, T: Read + Seek + 'a>(rdr: &mut PacketReader<T>) ->
		Result<(IdentHeader, CommentHeader, SetupHeader), VorbisError> {
	let pck :Packet = try!(rdr.read_packet());
	let ident_hdr = try!(read_header_ident(&pck.data));

	let pck :Packet = try!(rdr.read_packet());
	let comment_hdr = try!(read_header_comment(&pck.data));

	let pck :Packet = try!(rdr.read_packet());
	let setup_hdr = try!(read_header_setup(&pck.data, ident_hdr.audio_channels));

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
