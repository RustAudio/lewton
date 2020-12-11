// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

/*!
Header decoding

This module takes care of decoding of the three vorbis headers:

1. Identification
2. Comment
3. Setup

It builds only on the internal bitpacking layer and the internal
huffman tree handling mod. Everything else about the headers is
decoded in this mod.
*/

use std::error;
use std::fmt;
use ::bitpacking::BitpackCursor;
use ::huffman_tree::{VorbisHuffmanTree, HuffmanError};
use std::io::{Cursor, ErrorKind, Read, Error};
use byteorder::{ReadBytesExt, LittleEndian};
use std::string::FromUtf8Error;
use header_cached::{CachedBlocksizeDerived, compute_bark_map_cos_omega};

/// Errors that can occur during Header decoding
#[derive(Debug)]
#[derive(PartialEq)]
pub enum HeaderReadError {
	EndOfPacket,
	/// If the passed data don't start with the "vorbis"
	/// capture pattern, this error is returned.
	NotVorbisHeader,
	UnsupportedVorbisVersion,
	/// If the header violates the vorbis spec
	HeaderBadFormat,
	/// The given packet indeed seems to be a vorbis header,
	/// but it looks like it is a different header type than
	/// the function it was passed to.
	///
	/// It is not guaranteed that the type is a valid header type.
	HeaderBadType(u8),
	/// The given packet does not seem to be a header as per vorbis spec,
	/// instead it seems to be an audio packet.
	HeaderIsAudio,
	Utf8DecodeError,
	/// If the needed memory isn't addressable by us
	///
	/// This error is returned if a calculation yielded a higher value for
	/// an internal buffer size that doesn't fit into the platform's address range.
	/// Note that if we "simply" encounter an allocation failure (OOM, etc),
	/// we do what libstd does in these cases: crash.
	///
	/// This error is not automatically an error of the passed data,
	/// but rather is due to insufficient decoder hardware.
	BufferNotAddressable,
}

// For the () error type returned by the bitpacking layer
// TODO that type choice was a bit unfortunate,
// perhaps one day fix this
impl From<()> for HeaderReadError {
	fn from(_ :()) -> HeaderReadError {
		HeaderReadError::EndOfPacket
	}
}

impl From<HuffmanError> for HeaderReadError {
	fn from(_ :HuffmanError) -> HeaderReadError {
		HeaderReadError::HeaderBadFormat
	}
}

impl From<Error> for HeaderReadError {
	fn from(err :Error) -> HeaderReadError {
		match err.kind() {
			ErrorKind::UnexpectedEof => HeaderReadError::EndOfPacket,
			_ => panic!("Non EOF Error occured when reading from Cursor<&[u8]>: {}", err),
		}
	}
}

impl From<FromUtf8Error> for HeaderReadError {
	fn from(_ :FromUtf8Error) -> HeaderReadError {
		HeaderReadError::Utf8DecodeError
	}
}

impl error::Error for HeaderReadError {}

impl fmt::Display for HeaderReadError {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		let description = match self {
			HeaderReadError::EndOfPacket => "End of packet reached.",
			HeaderReadError::NotVorbisHeader => "The packet is not a vorbis header",
			HeaderReadError::UnsupportedVorbisVersion => "The vorbis version is not supported",
			HeaderReadError::HeaderBadFormat => "Invalid header",
			HeaderReadError::HeaderBadType(_) => "Invalid/unexpected header type",
			HeaderReadError::HeaderIsAudio => "Packet seems to be audio",
			HeaderReadError::Utf8DecodeError => "UTF-8 decoding error",
			HeaderReadError::BufferNotAddressable => "Requested to create buffer of non-addressable size",
		};
		write!(fmt, "{}", description)
	}
}

/// Macro to convert values of any unsigned integral non-usize type to
/// usize, and then check whether there had been any losses due to conversion.
///
/// If there were, it will return the BufferNotAddressable error.
macro_rules! convert_to_usize {
( $val:expr, $val_type:ident ) => { {
	let converted :usize = $val as usize;
	if $val != converted as $val_type {
		try!(Err(HeaderReadError::BufferNotAddressable));
	}
	converted
}}
}

// Internal function, tries to find out whether the
// data returned by rdr belong to a vorbis header
// On success it returns Some(n) with n as packet type
// (you must check that n from 1,3,5)
macro_rules! read_header_begin_body {
( $rdr:expr ) => { {
	let res = try!($rdr.read_u8());
	if res & 1 == 0 {
		// This is an audio packet per vorbis spec, if anything.
		// (audio packets have their first bit set to 0,
		// header packets have it set to 1)
		try!(Err(HeaderReadError::HeaderIsAudio));
	}
	let is_vorbis =
		try!($rdr.read_u8()) == 0x76 && // 'v'
		try!($rdr.read_u8()) == 0x6f && // 'o'
		try!($rdr.read_u8()) == 0x72 && // 'r'
		try!($rdr.read_u8()) == 0x62 && // 'b'
		try!($rdr.read_u8()) == 0x69 && // 'i'
		try!($rdr.read_u8()) == 0x73;   // 's'
	if !is_vorbis {
		try!(Err(HeaderReadError::NotVorbisHeader));
	}
	return Ok(res);
}}
}
fn read_header_begin(rdr :&mut BitpackCursor) -> Result<u8, HeaderReadError> {
	read_header_begin_body!(rdr)
}
fn read_header_begin_cursor(rdr :&mut Cursor<&[u8]>) -> Result<u8, HeaderReadError> {
	read_header_begin_body!(rdr)
}


#[test]
fn test_read_hdr_begin() {
	// Only tests flawed header begins, correct headers
	// are tested later by the test methods for the headers

	// Flawed ident header (see char before the /**/)
	let test_arr = &[0x01, 0x76, 0x6f, 0x72,
	0x62, 0x69, 0x72, /**/ 0x00, 0x00, 0x00, 0x00, 0x02,
	0x44, 0xac, 0x00,      0x00, 0x00, 0x00, 0x00, 0x00,
	0x80, 0xb5, 0x01,      0x00, 0x00, 0x00, 0x00, 0x00,
	0xb8, 0x01];
	let mut rdr :BitpackCursor = BitpackCursor::new(test_arr);
	assert_eq!(read_header_begin(&mut rdr), Err(HeaderReadError::NotVorbisHeader));
}

/// The set of the three Vorbis headers
pub type HeaderSet = (IdentHeader, CommentHeader, SetupHeader);

/**
Representation for the identification header

The identification header is the first of the three
headers inside each vorbis stream.

It covers basic information about the stream.
*/
pub struct IdentHeader {
	/// The number of audio channels in the stream
	pub audio_channels :u8,
	/// The sample rate of the stream
	pub audio_sample_rate :u32,
	/// The maximum bit rate of the stream
	///
	/// Note that this value is only a hint
	/// and may be off by a large amount.
	pub bitrate_maximum :i32,
	/// The nominal bit rate of the stream
	///
	/// Note that this value is only a hint
	/// and may be off by a large amount.
	pub bitrate_nominal :i32,
	/// The minimum bit rate of the stream
	///
	/// Note that this value is only a hint
	/// and may be off by a large amount.
	pub bitrate_minimum :i32,
	pub blocksize_0 :u8,
	pub blocksize_1 :u8,
	pub(crate) cached_bs_derived :[CachedBlocksizeDerived; 2],
}

/**
Reading the Identification header

If it returns Err(sth) when being called with the first packet in a stream,
the whole stream is to be considered undecodable as per the Vorbis spec.
The function returns Err(`HeaderReadError::HeaderBadType`) if the header type
doesn't match the ident header.
*/
pub fn read_header_ident(packet :&[u8]) -> Result<IdentHeader, HeaderReadError> {
	let mut rdr = BitpackCursor::new(packet);
	let hd_id = try!(read_header_begin(&mut rdr));
	if hd_id != 1 {
		try!(Err(HeaderReadError::HeaderBadType(hd_id)));
	}
	let vorbis_version = try!(rdr.read_u32());
	if vorbis_version != 0 {
		try!(Err(HeaderReadError::UnsupportedVorbisVersion));
	}
	let audio_channels = try!(rdr.read_u8());
	let audio_sample_rate = try!(rdr.read_u32());
	let bitrate_maximum = try!(rdr.read_i32());
	let bitrate_nominal = try!(rdr.read_i32());
	let bitrate_minimum = try!(rdr.read_i32());
	let blocksize_0 = try!(rdr.read_u4());
	let blocksize_1 = try!(rdr.read_u4());
	let framing = try!(rdr.read_u8());
	if blocksize_0 < 6 || blocksize_0 > 13 ||
			blocksize_1 < 6 || blocksize_1 > 13 ||
			(framing != 1) || blocksize_0 > blocksize_1 ||
			audio_channels == 0 || audio_sample_rate == 0 {
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	let hdr :IdentHeader = IdentHeader {
		audio_channels,
		audio_sample_rate,
		bitrate_maximum,
		bitrate_nominal,
		bitrate_minimum,
		blocksize_0,
		blocksize_1,
		cached_bs_derived : [
			CachedBlocksizeDerived::from_blocksize(blocksize_0),
			CachedBlocksizeDerived::from_blocksize(blocksize_1),
		],
	};
	return Ok(hdr);
}

#[test]
fn test_read_header_ident() {
	// Valid ident header
	let test_arr = &[0x01, 0x76, 0x6f, 0x72,
	0x62, 0x69, 0x73, 0x00, 0x00, 0x00, 0x00, 0x02,
	0x44, 0xac, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
	0x80, 0xb5, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
	0xb8, 0x01];
	let hdr = read_header_ident(test_arr).unwrap();
	assert_eq!(hdr.audio_channels, 2);
	assert_eq!(hdr.audio_sample_rate, 0x0000ac44);
	assert_eq!(hdr.bitrate_maximum, 0);
	assert_eq!(hdr.bitrate_nominal, 0x0001b580);
	assert_eq!(hdr.bitrate_minimum, 0);
	assert_eq!(hdr.blocksize_0, 8);
	assert_eq!(hdr.blocksize_1, 11);
}

/**
Representation of the comment header

The comment header is the second of the three
headers inside each vorbis stream.

It contains text comment metadata
about the stream, encoded as key-value pairs,
and the vendor name.
*/
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CommentHeader {
	/// An identification string of the
	/// software/library that encoded
	/// the stream.
	pub vendor :String,
	/// A key-value list of the comments
	/// attached to the stream.
	pub comment_list :Vec<(String, String)>,
}

/**
Reading the Comment header

You should call this function with the second packet in the stream.

The function does not check whether the comment field names consist
of characters `0x20` through `0x7D` (`0x3D` excluded), as the vorbis
spec requires.
*/
pub fn read_header_comment(packet :&[u8]) -> Result<CommentHeader, HeaderReadError> {
	let mut rdr = Cursor::new(packet);
	let hd_id = try!(read_header_begin_cursor(&mut rdr));
	if hd_id != 3 {
		try!(Err(HeaderReadError::HeaderBadType(hd_id)));
	}
	// First read the vendor string
	let vendor_length = try!(rdr.read_u32::<LittleEndian>()) as usize;
	let mut vendor_buf = vec![0; vendor_length]; // TODO fix this, we initialize memory for NOTHING!!! Out of some reason, this is seen as "unsafe" by rustc.
	try!(rdr.read_exact(&mut vendor_buf));
	let vendor = try!(String::from_utf8(vendor_buf));

	// Now read the comments
	let comment_count = try!(rdr.read_u32::<LittleEndian>()) as usize;
	let mut comment_list = Vec::with_capacity(comment_count);
	for _ in 0 .. comment_count {
		let comment_length = try!(rdr.read_u32::<LittleEndian>()) as usize;
		let mut comment_buf = vec![0; comment_length]; // TODO fix this, we initialize memory for NOTHING!!! Out of some reason, this is seen as "unsafe" by rustc.
		try!(rdr.read_exact(&mut comment_buf));
		let comment = match String::from_utf8(comment_buf) {
			Ok(comment) => comment,
			// Uncomment for closer compliance with the spec.
			// The spec explicitly states that the comment entries
			// should be UTF-8 formatted, however it seems that other
			// decoder libraries tolerate non-UTF-8 formatted strings
			// in comments. This has led to some files circulating
			// with such errors inside. If we deny to decode such files,
			// lewton would be the odd one out. Thus we just
			// gracefully ignore them.
			Err(_) => continue,
		};
		let eq_idx = match comment.find("=") {
			Some(k) => k,
			// Uncomment for closer compliance with the spec.
			// It appears that some ogg files have fields without a = sign in the comments.
			// Well there is not much we can do but gracefully ignore their stuff.
			None => continue // try!(Err(HeaderReadError::HeaderBadFormat))
		};
		let (key_eq, val) = comment.split_at(eq_idx + 1);
		let (key, _) = key_eq.split_at(eq_idx);
		comment_list.push((String::from(key), String::from(val)));
	}
	let framing = try!(rdr.read_u8());
	if framing != 1 {
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	let hdr :CommentHeader = CommentHeader {
		vendor,
		comment_list,
	};
	return Ok(hdr);
}

pub(crate) struct Codebook {
	pub codebook_dimensions :u16,
	pub codebook_entries :u32,

	// None if codebook_lookup_type == 0
	pub codebook_vq_lookup_vec :Option<Vec<f32>>,

	pub codebook_huffman_tree :VorbisHuffmanTree,
}

pub(crate) struct Residue {
	pub residue_type :u8,
	pub residue_begin :u32,
	pub residue_end :u32,
	pub residue_partition_size :u32,
	pub residue_classifications :u8,
	pub residue_classbook :u8,
	pub residue_books :Vec<ResidueBook>,
}

pub(crate) struct Mapping {
	pub mapping_submaps :u8,
	pub mapping_magnitudes :Vec<u8>,
	pub mapping_angles :Vec<u8>,
	pub mapping_mux :Vec<u8>,
	pub mapping_submap_floors :Vec<u8>,
	pub mapping_submap_residues :Vec<u8>,
}

pub(crate) struct ModeInfo {
	pub mode_blockflag :bool,
	pub mode_mapping :u8,
}

pub(crate) enum Floor {
	TypeZero(FloorTypeZero),
	TypeOne(FloorTypeOne),
}

pub(crate) struct FloorTypeZero {
	pub floor0_order :u8,
	pub floor0_rate :u16,
	pub floor0_bark_map_size :u16,
	pub floor0_amplitude_bits :u8,
	pub floor0_amplitude_offset :u8,
	pub floor0_number_of_books :u8,
	pub floor0_book_list :Vec<u8>,
	pub cached_bark_cos_omega :[Vec<f32>; 2],
}

pub(crate) struct FloorTypeOne {
	pub floor1_multiplier :u8,
	pub floor1_partition_class :Vec<u8>,
	pub floor1_class_dimensions :Vec<u8>,
	pub floor1_class_subclasses :Vec<u8>,
	pub floor1_subclass_books :Vec<Vec<i16>>,
	pub floor1_class_masterbooks :Vec<u8>,
	pub floor1_x_list :Vec<u32>,
	pub floor1_x_list_sorted :Vec<(usize, u32)>,
}

pub(crate) struct ResidueBook {
	vals_used :u8,
	val_i :[u8; 8],
}

impl ResidueBook {
	pub fn get_val(&self, i :u8) -> Option<u8> {
		if i >= 8 {
			// This is a precondition...
			panic!("Tried to get ResidueBook value out of bounds (index = {})",
				i);
		}
		return if self.vals_used & (1 << i) > 0 {
			Some(self.val_i[i as usize])
		} else {
			None
		};
	}
	/// Reads the `ResidueBook` from a `BitpackCursor`.
	fn read_book(rdr :&mut BitpackCursor,
			vals_used :u8, codebooks :&[Codebook])
			-> Result<Self, HeaderReadError> {
		let mut val_i :[u8; 8] = [0; 8];
		for i in 0 .. 7 {
			if vals_used & (1 << i) == 0 {
				continue;
			}
			let val_entry = try!(rdr.read_u8());
			if match codebooks.get(val_entry as usize) {
				Some(v) => v.codebook_vq_lookup_vec.is_none(),
				None => true,
			} {
				// Both of the cases are forbidden by spec
				// (the codebook being out of bounds, or
				// not having a value mapping)
				try!(Err(HeaderReadError::HeaderBadFormat))
			}
			val_i[i] = val_entry;
		}
		return Ok(ResidueBook { vals_used, val_i });
	}
}

pub struct SetupHeader {
	pub(crate) codebooks :Vec<Codebook>,
	pub(crate) floors :Vec<Floor>,
	pub(crate) residues :Vec<Residue>,
	pub(crate) mappings :Vec<Mapping>,
	pub(crate) modes :Vec<ModeInfo>,
}

struct CodebookVqLookup {
	codebook_lookup_type :u8,
	codebook_minimum_value :f32,
	codebook_delta_value :f32,
	codebook_sequence_p :bool,
	codebook_multiplicands :Vec<u32>,
}

/// Vector value decode for lookup
///
/// Prepares the VQ context vectors for later lookup
/// by the codebook abstraction layer.
///
/// Returns `codebook_entries` many vectors,
/// each being `codebook_dimensions` scalars wide),
/// all stored in one Vec.
fn lookup_vec_val_decode(lup :&CodebookVqLookup, codebook_entries :u32, codebook_dimensions :u16) -> Vec<f32> {
	let mut value_vectors = Vec::with_capacity(
		codebook_entries as usize * codebook_dimensions as usize);
	if lup.codebook_lookup_type == 1 {
		let codebook_lookup_values = lup.codebook_multiplicands.len();
		for lookup_offset in 0 .. codebook_entries {
			let mut last = 0.;
			let mut index_divisor = 1;
			for _ in 0 .. codebook_dimensions {
				let multiplicand_offset = (lookup_offset / index_divisor as u32) as usize %
					codebook_lookup_values;
				let vec_elem = lup.codebook_multiplicands[multiplicand_offset] as f32 *
					lup.codebook_delta_value + lup.codebook_minimum_value + last;
				if lup.codebook_sequence_p {
					last = vec_elem;
				}
				value_vectors.push(vec_elem);
				index_divisor *= codebook_lookup_values;
			}
		}
	} else {
		for lookup_offset in 0 .. codebook_entries {
			let mut last = 0.;
			let mut multiplicand_offset :usize = lookup_offset as usize * codebook_dimensions as usize;
			for _ in 0 .. codebook_dimensions {
				let vec_elem = lup.codebook_multiplicands[multiplicand_offset] as f32 *
					lup.codebook_delta_value + lup.codebook_minimum_value + last;
				if lup.codebook_sequence_p {
					last = vec_elem;
				}
				value_vectors.push(vec_elem);
				multiplicand_offset += 1;
			}
		}
	}
	return value_vectors;
}


/// Small Error type for `BitpackCursor::read_huffman_vq`.
///
/// This is in order to enable calling code to distinguish
/// between the two cases of the enum. Esp. in some cases
/// the decoder might have to reject packages with the
/// NoVqLookupForCodebook variant, but have to treat EndOfPacket
/// as normal occurence.
pub(crate) enum HuffmanVqReadErr {
	EndOfPacket,
	NoVqLookupForCodebook,
}

impl <'a> BitpackCursor <'a> {
	/// Reads a huffman word using the codebook abstraction via a VQ context
	pub(crate) fn read_huffman_vq<'b>(&mut self, b :&'b Codebook) -> Result<&'b[f32], HuffmanVqReadErr> {

		let idx = match self.read_huffman(&b.codebook_huffman_tree) {
			Ok(v) => v as usize,
			Err(_) => return Err(HuffmanVqReadErr::EndOfPacket),
		};
		let codebook_vq_lookup_vec :&[f32] = match b.codebook_vq_lookup_vec.as_ref() {
			Some(ref v) => v,
			None => return Err(HuffmanVqReadErr::NoVqLookupForCodebook),
		};
		let dim = b.codebook_dimensions as usize;
		return Ok(&codebook_vq_lookup_vec[idx * dim .. (idx + 1) * dim]);
	}
}

static MAX_BASES_WITHOUT_OVERFLOW : &[u32] = &[
	0xffffffff, 0xffffffff, 0x0000ffff, 0x00000659,
	0x000000ff, 0x00000054, 0x00000028, 0x00000017,
	0x0000000f, 0x0000000b, 0x00000009, 0x00000007,
	0x00000006, 0x00000005, 0x00000004, 0x00000004,
	0x00000003, 0x00000003, 0x00000003, 0x00000003,
	0x00000003, 0x00000002, 0x00000002, 0x00000002,
	0x00000002, 0x00000002, 0x00000002, 0x00000002,
	0x00000002, 0x00000002, 0x00000002, 0x00000002];

static MAX_BASE_MAX_BITS_WITHOUT_OVERFLOW : &[u8] = &[
	0x1f, 0x1f, 0x0f, 0x0a,
	0x07, 0x06, 0x05, 0x04,
	0x03, 0x03, 0x03, 0x02,
	0x02, 0x02, 0x02, 0x02,
	0x01, 0x01, 0x01, 0x01,
	0x01, 0x01, 0x01, 0x01,
	0x01, 0x01, 0x01, 0x01,
	0x01, 0x01, 0x01, 0x01];

// For this little function I won't include the num crate.
// precondition: base ^ exponent must not overflow.
fn exp_fast(base :u32, exponent: u8) -> u32 {
	let mut res :u32 = 1;
	let mut selfmul = base;
	for i in 0 .. 8 {
		if (1 << i) & exponent > 0 {
			res *= selfmul;
		}
		if let Some(newselfmul) = u32::checked_mul(selfmul, selfmul) {
			selfmul = newselfmul;
		} else {
			// Okay, now we have to find out
			// whether this matters or not.
			// Check whether selfmul would have been needed.
			if i < 7 && (exponent >> (i + 1)) > 0 {
				panic!("Overflow when squaring for exp_fast, \
					precondition violated!");
			}
			return res;
		}
	}
	return res;
}

/// Returns, as defined in the vorbis spec:
/// "the greatest integer for which to `[return_value]` the power of `[codebook_dimensions]` is less than or equal to `[codebook_entries]`"
/// Essentially an "nth-root" algorithm.
/// About the speed:
/// Probably its super-optimized as it uses no floats,
/// probably smarter algorithms using floats would be faster here. No idea.
/// Either way, stackoverflow gave the (great) motivation for the algorithm:
/// http://stackoverflow.com/questions/7407752
fn lookup1_values(codebook_entries :u32, codebook_dimensions :u16) -> u32 {
	if codebook_dimensions >= 32 {
		// For codebook_dimensions >= 32 we'd already overflow the u32 range if
		// we computed 2 ^ codebook_dimensions.
		// Therefore, the result must be less than 2.
		return if codebook_entries == 0 { 0 } else { 1 };
	}
	// Now do a binary search.
	// We use two static helper arrays here. Both take the
	// exponent (codebook_dimensions here) as index.
	// The first array, MAX_BASES_WITHOUT_OVERFLOW contains
	// the base that doesn't generate an overflow for the
	// given exponent.
	// The second array MAX_BASE_MAX_BITS_WITHOUT_OVERFLOW
	// contains the number of the highest set bit in
	// the corresponding entry in MAX_BASES_WITHOUT_OVERFLOW.
	// This is the first bit that is "disputed" in the binary
	// search to follow: we check the bases to support the
	// claim by manual exponentiation.
	let max_base_bits = MAX_BASE_MAX_BITS_WITHOUT_OVERFLOW[
		codebook_dimensions as usize];
	let max_base = MAX_BASES_WITHOUT_OVERFLOW[codebook_dimensions as usize];
	let mut base_bits :u32 = 0;
	for i in 0 .. max_base_bits + 1 {
		let cur_disputed_bit :u32 = 1 << (max_base_bits - i);
		base_bits |= cur_disputed_bit;
		if max_base < base_bits ||
				exp_fast(base_bits, codebook_dimensions as u8) > codebook_entries {
			base_bits &= !cur_disputed_bit;
		}
	}
	return base_bits;
}

#[test]
fn test_lookup1_values() {
	// First, with base two:
	// 2 ^ 10 = 1024
	assert_eq!(lookup1_values(1025, 10), 2);
	assert_eq!(lookup1_values(1024, 10), 2);
	assert_eq!(lookup1_values(1023, 10), 1);

	// Now, the searched base is five:
	// 5 ^ 5 = 3125
	assert_eq!(lookup1_values(3126, 5), 5);
	assert_eq!(lookup1_values(3125, 5), 5);
	assert_eq!(lookup1_values(3124, 5), 4);

	// Now some exotic tests (edge cases :p):
	assert_eq!(lookup1_values(1, 1), 1);
	assert_eq!(lookup1_values(0, 15), 0);
	assert_eq!(lookup1_values(0, 0), 0);
	assert_eq!(lookup1_values(1, 0), std::u32::MAX);
	assert_eq!(lookup1_values(400, 0), std::u32::MAX);
}

/// Reads a codebook which is part of the setup header packet.
fn read_codebook(rdr :&mut BitpackCursor) -> Result<Codebook, HeaderReadError> {

	// 1. Read the sync pattern
	let sync_pattern = try!(rdr.read_u24());
	if sync_pattern != 0x564342 {
		try!(Err(HeaderReadError::HeaderBadFormat));
	}

	// 2. Read the _dimension, _entries fields and the ordered bitflag
	let codebook_dimensions = try!(rdr.read_u16());
	let codebook_entries = try!(rdr.read_u24());
	let ordered = try!(rdr.read_bit_flag());

	// 3. Read the codeword lengths
	let mut codebook_codeword_lengths = Vec::with_capacity(
		convert_to_usize!(codebook_entries, u32));
	if !ordered {
		let sparse = try!(rdr.read_bit_flag());
		for _ in 0 .. codebook_entries {
			let length = if sparse {
				let flag = try!(rdr.read_bit_flag());
				if flag {
					try!(rdr.read_u5()) + 1
				} else {
					/* The spec here asks that we should mark that the
					entry is unused. But 0 already fulfills this purpose,
					as everywhere else its guaranteed that the length is > 0.
					No messing with Option<T> needed here :) */
					0
				}
			} else {
				try!(rdr.read_u5()) + 1
			};
			codebook_codeword_lengths.push(length);
		}
	} else {
		let mut current_entry :u32 = 0;
		let mut current_length = try!(rdr.read_u5()) + 1;
		while current_entry < codebook_entries {
			let number = try!(rdr.read_dyn_u32(
				::ilog((codebook_entries - current_entry) as u64)));
			for _ in current_entry .. current_entry + number {
				codebook_codeword_lengths.push(current_length);
			}
			current_entry += number;
			current_length += 1;
			if current_entry as u32 > codebook_entries {
				try!(Err(HeaderReadError::HeaderBadFormat));
			}
		}
	}

	// 4. Read the vector lookup table
	let codebook_lookup_type = try!(rdr.read_u4());
	if codebook_lookup_type > 2 {
		// Not decodable per vorbis spec
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	let codebook_lookup :Option<CodebookVqLookup> =
	if codebook_lookup_type == 0 {
		None
	} else {
		let codebook_minimum_value = try!(rdr.read_f32());
		let codebook_delta_value = try!(rdr.read_f32());
		let codebook_value_bits = try!(rdr.read_u4()) + 1;
		let codebook_sequence_p = try!(rdr.read_bit_flag());
		let codebook_lookup_values :u64 = if codebook_lookup_type == 1 {
			 lookup1_values(codebook_entries, codebook_dimensions) as u64
		} else {
			codebook_entries as u64 * codebook_dimensions as u64
		};
		let mut codebook_multiplicands = Vec::with_capacity(
			convert_to_usize!(codebook_lookup_values, u64));
		for _ in 0 .. codebook_lookup_values {
			codebook_multiplicands.push(try!(rdr.read_dyn_u32(codebook_value_bits)));
		}
		Some(CodebookVqLookup {
			codebook_lookup_type,
			codebook_minimum_value,
			codebook_delta_value,
			codebook_sequence_p,
			codebook_multiplicands,
		})
	};
	let codebook_vq_lookup_vec = codebook_lookup.as_ref().map(|lup| {
		lookup_vec_val_decode(lup,
			codebook_entries, codebook_dimensions)
	});

	return Ok(Codebook {
		codebook_dimensions,
		codebook_entries,
		codebook_vq_lookup_vec,
		codebook_huffman_tree : try!(VorbisHuffmanTree::load_from_array(&codebook_codeword_lengths)),
	});
}

/// Reads a Floor which is part of the setup header packet.
/// The `codebook_cnt` param is required to check for compliant streams
fn read_floor(rdr :&mut BitpackCursor, codebook_cnt :u16, blocksizes :(u8, u8)) ->
		Result<Floor, HeaderReadError> {
	let floor_type = try!(rdr.read_u16());
	match floor_type {
		0 => {
			let floor0_order = try!(rdr.read_u8());
			let floor0_rate = try!(rdr.read_u16());
			let floor0_bark_map_size = try!(rdr.read_u16());
			let floor0_amplitude_bits = try!(rdr.read_u6());
			if floor0_amplitude_bits > 64 {
				// Unfortunately the audio decoder part
				// doesn't support values > 64 because rust has no
				// 128 bit integers yet.
				// TODO when support is added, remove this
				// check.
				try!(Err(HeaderReadError::HeaderBadFormat));
			}
			let floor0_amplitude_offset = try!(rdr.read_u8());
			let floor0_number_of_books = try!(rdr.read_u4()) + 1;
			let mut floor0_book_list = Vec::with_capacity(
				convert_to_usize!(floor0_number_of_books, u8));
			for _ in 0 .. floor0_number_of_books {
				let value = try!(rdr.read_u8());
				if value as u16 > codebook_cnt {
					try!(Err(HeaderReadError::HeaderBadFormat));
				}
				floor0_book_list.push(value);
			}
			Ok(Floor::TypeZero(FloorTypeZero {
				floor0_order,
				floor0_rate,
				floor0_bark_map_size,
				floor0_amplitude_bits,
				floor0_amplitude_offset,
				floor0_number_of_books,
				floor0_book_list,
				cached_bark_cos_omega : [
					compute_bark_map_cos_omega(1 << (blocksizes.0 - 1),
						floor0_rate, floor0_bark_map_size),
					compute_bark_map_cos_omega(1 << (blocksizes.1 - 1),
						floor0_rate, floor0_bark_map_size),
				]
			}))
		},
		1 => {
			let floor1_partitions = try!(rdr.read_u5());
			let mut maximum_class :i8 = -1;
			let mut floor1_partition_class_list = Vec::with_capacity(
				floor1_partitions as usize);
			for _ in 0 .. floor1_partitions {
				let cur_class = try!(rdr.read_u4());
				maximum_class = if cur_class as i8 > maximum_class
					{ cur_class as i8 } else { maximum_class };
				floor1_partition_class_list.push(cur_class);
			}

			// TODO one day try out whether its more performant
			// to have these two arrays in one, its wasteful to allocate
			// 16 bit so that one can store 5 bits.
			let mut floor1_class_dimensions = Vec::with_capacity((maximum_class + 1) as usize);
			let mut floor1_class_subclasses = Vec::with_capacity((maximum_class + 1) as usize);

			let mut floor1_subclass_books = Vec::with_capacity((maximum_class + 1) as usize);

			let mut floor1_class_masterbooks = Vec::with_capacity((maximum_class + 1) as usize);
			for _ in 0 .. maximum_class + 1 {
				floor1_class_dimensions.push(try!(rdr.read_u3()) + 1);
				let cur_subclass = try!(rdr.read_u2());
				floor1_class_subclasses.push(cur_subclass);
				if cur_subclass != 0 {
					let cur_masterbook = try!(rdr.read_u8());
					if cur_masterbook as u16 >= codebook_cnt {
						// undecodable as per spec
						try!(Err(HeaderReadError::HeaderBadFormat));
					}
					floor1_class_masterbooks.push(cur_masterbook);
				} else {
					// Some value... This never gets read,
					// but Rust requires everything to be initialized,
					// we can't increase the counter without initialisation.
					floor1_class_masterbooks.push(0);
				}
				let cur_books_cnt :u8 = 1 << cur_subclass;
				let mut cur_books = Vec::with_capacity(cur_books_cnt as usize);
				for _ in 0 .. cur_books_cnt {
					// The fact that we need i16 here (and shouldn't do
					// wrapping sub) is only revealed if you read the
					// "packet decode" part of the floor 1 spec...
					let cur_book = (try!(rdr.read_u8()) as i16) - 1;
					if cur_book >= codebook_cnt as i16 {
						// undecodable as per spec
						try!(Err(HeaderReadError::HeaderBadFormat));
					}
					cur_books.push(cur_book);
				}
				floor1_subclass_books.push(cur_books);
			}
			let floor1_multiplier = try!(rdr.read_u2()) + 1;
			let rangebits = try!(rdr.read_u4());
			let mut floor1_values :u16 = 2;
			// Calculate the count before doing anything else
			for cur_class_num in &floor1_partition_class_list {
				floor1_values += floor1_class_dimensions[*cur_class_num as usize] as u16;
			}
			if floor1_values > 65 {
				// undecodable as per spec
				try!(Err(HeaderReadError::HeaderBadFormat));
			}
			let mut floor1_x_list = Vec::with_capacity(floor1_values as usize);
			floor1_x_list.push(0);
			floor1_x_list.push(1u32 << rangebits);
			for cur_class_num in &floor1_partition_class_list {
				for _ in 0 .. floor1_class_dimensions[*cur_class_num as usize] {
					floor1_x_list.push(try!(rdr.read_dyn_u32(rangebits)));
				}
			}
			// Now do an uniqueness check on floor1_x_list
			// to check decodability.
			let mut floor1_x_list_sorted = floor1_x_list.iter().cloned()
				.enumerate().collect::<Vec<_>>();
			floor1_x_list_sorted.sort_by(|a, b| a.1.cmp(&b.1));
			// 0 is guaranteed to be in the list,
			// and due to sorting it will be first.
			let mut last = 1;
			for el in &floor1_x_list_sorted {
				if el.1 == last {
					// duplicate entry found
					// undecodable as per spec
					try!(Err(HeaderReadError::HeaderBadFormat));
				}
				last = el.1;
			}

			// Only now return the result
			Ok(Floor::TypeOne(FloorTypeOne {
				floor1_multiplier,
				floor1_partition_class : floor1_partition_class_list,
				floor1_class_dimensions,
				floor1_class_subclasses,
				floor1_subclass_books,
				floor1_class_masterbooks,
				floor1_x_list,
				floor1_x_list_sorted,

			}))
		},
		// Type greater than 1 is error condition per spec
		_ => Err(HeaderReadError::HeaderBadFormat),
	}
}

/// Reads a Residue which is part of the setup header packet.
/// The `codebook_cnt` param is required to check for compliant streams
fn read_residue(rdr :&mut BitpackCursor, codebooks :&[Codebook])
		-> Result<Residue, HeaderReadError> {
	let residue_type = try!(rdr.read_u16());
	if residue_type > 2 {
		// Undecodable by spec
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	let residue_begin = try!(rdr.read_u24());
	let residue_end = try!(rdr.read_u24());
	if residue_begin > residue_end {
		// If residue_begin < residue_end, we'll get
		// errors in audio parsing code.
		// As the idea of residue end being before begin
		// sounds quite wrong anyway, we already error
		// earlier, in header parsing code.
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	let residue_partition_size = try!(rdr.read_u24()) + 1;
	let residue_classifications = try!(rdr.read_u6()) + 1;
	let residue_classbook = try!(rdr.read_u8());
	// Read the bitmap pattern:
	let mut residue_cascade = Vec::with_capacity(residue_classifications as usize);
	for _ in 0 .. residue_classifications {
		let mut high_bits = 0;
		let low_bits = try!(rdr.read_u3());
		let bitflag = try!(rdr.read_bit_flag());
		if bitflag {
			high_bits = try!(rdr.read_u5());
		}
		residue_cascade.push((high_bits << 3) | low_bits);
	}

	let mut residue_books = Vec::with_capacity(residue_classifications as usize);
	// Read the list of book numbers:
	for cascade_entry in &residue_cascade {
		residue_books.push(try!(
			ResidueBook::read_book(rdr, *cascade_entry, codebooks)));
	}
	if residue_classbook as usize >= codebooks.len() {
		// Undecodable because residue_classbook must be valid index
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	/*
	// Currently we check below condition in audio decode, following the spec,
	// section 3.3., saying that it only renders the packet that wants to use the
	// invalid codebook invalid, but not the whole stream only because there is a
	// residue in the header (which may never be used).
	if codebooks[residue_classbook as usize].codebook_vq_lookup_vec.is_none() {
		// Undecodable because residue_classbook must be valid index
		try!(Err(HeaderReadError::HeaderBadFormat));
	}*/
	return Ok(Residue {
		residue_type : residue_type as u8,
		residue_begin,
		residue_end,
		residue_partition_size,
		residue_classifications,
		residue_classbook,
		residue_books,
	});
}

/// Reads a "Mapping" which is part of the setup header packet.
fn read_mapping(rdr :&mut BitpackCursor,
		audio_chan_ilog :u8, audio_channels :u8,
		floor_count :u8, residue_count :u8)
		-> Result<Mapping, HeaderReadError> {
	let mapping_type = try!(rdr.read_u16());
	if mapping_type > 0 {
		// Undecodable per spec
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	let mapping_submaps = match try!(rdr.read_bit_flag()) {
		true => try!(rdr.read_u4()) + 1,
		false => 1,
	};
	let mapping_coupling_steps = match try!(rdr.read_bit_flag()) {
		true => try!(rdr.read_u8()) as u16 + 1,
		false => 0,
	};
	let mut mapping_magnitudes = Vec::with_capacity(mapping_coupling_steps as usize);
	let mut mapping_angles = Vec::with_capacity(mapping_coupling_steps as usize);
	for _ in 0 .. mapping_coupling_steps {
		let cur_mag = try!(rdr.read_dyn_u8(audio_chan_ilog));
		let cur_angle = try!(rdr.read_dyn_u8(audio_chan_ilog));
		if (cur_angle == cur_mag) || (cur_mag >= audio_channels)
				|| (cur_angle >= audio_channels) {
			// Undecodable per spec
			try!(Err(HeaderReadError::HeaderBadFormat));
		}
		mapping_magnitudes.push(cur_mag);
		mapping_angles.push(cur_angle);
	}
	let reserved = try!(rdr.read_u2());
	if reserved != 0 {
		// Undecodable per spec
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	let mapping_mux = if mapping_submaps > 1 {
		let mut m = Vec::with_capacity(audio_channels as usize);
		for _ in 0 .. audio_channels {
			let val = try!(rdr.read_u4());
			if val >= mapping_submaps {
				// Undecodable per spec
				try!(Err(HeaderReadError::HeaderBadFormat));
			}
			m.push(val);
		};
		m
	} else {
		vec![0; audio_channels as usize]
	};
	let mut mapping_submap_floors = Vec::with_capacity(mapping_submaps as usize);
	let mut mapping_submap_residues = Vec::with_capacity(mapping_submaps as usize);
	for _ in 0 .. mapping_submaps {
		// To whom those reserved bits may concern.
		// I have discarded them!
		try!(rdr.read_u8());
		let cur_floor = try!(rdr.read_u8());
		let cur_residue = try!(rdr.read_u8());
		if cur_floor >= floor_count ||
				cur_residue >= residue_count {
			// Undecodable per spec
			try!(Err(HeaderReadError::HeaderBadFormat));
		}
		mapping_submap_floors.push(cur_floor);
		mapping_submap_residues.push(cur_residue);
	}
	return Ok(Mapping {
		mapping_submaps,
		mapping_magnitudes,
		mapping_angles,
		mapping_mux,
		mapping_submap_floors,
		mapping_submap_residues,
	});
}

/// Reads a ModeInfo which is part of the setup header packet.
fn read_mode_info(rdr :&mut BitpackCursor, mapping_count :u8) -> Result<ModeInfo, HeaderReadError> {
	let mode_blockflag = try!(rdr.read_bit_flag());
	let mode_windowtype = try!(rdr.read_u16());
	let mode_transformtype = try!(rdr.read_u16());
	let mode_mapping = try!(rdr.read_u8());
	// Verifying ranges
	if mode_windowtype != 0 ||
			mode_transformtype != 0 ||
			mode_mapping >= mapping_count {
		// Undecodable per spec
		try!(Err(HeaderReadError::HeaderBadFormat));
	}
	return Ok(ModeInfo {
		mode_blockflag,
		mode_mapping,
	});
}

/// Reading the setup header.
///
/// The audio channel and blocksize info needed by the function
/// can be obtained from the ident header.
pub fn read_header_setup(packet :&[u8], audio_channels :u8, blocksizes :(u8, u8)) ->
		Result<SetupHeader, HeaderReadError> {
	let mut rdr = BitpackCursor::new(packet);
	let hd_id = try!(read_header_begin(&mut rdr));
	if hd_id != 5 {
		try!(Err(HeaderReadError::HeaderBadType(hd_id)));
	}

	// Little preparation -- needed later
	let audio_chan_ilog = ::ilog((audio_channels - 1) as u64);

	//::print_u8_slice(packet);

	// 1. Read the codebooks
	let vorbis_codebook_count :u16 = try!(rdr.read_u8()) as u16 + 1;
	let mut codebooks = Vec::with_capacity(vorbis_codebook_count as usize);
	for _ in 0 .. vorbis_codebook_count {
		codebooks.push(try!(read_codebook(&mut rdr)));
	}

	// 2. Read the time domain transforms
	let vorbis_time_count :u8 = try!(rdr.read_u6()) + 1;
	for _ in 0 .. vorbis_time_count {
		if try!(rdr.read_u16()) != 0 {
			try!(Err(HeaderReadError::HeaderBadFormat));
		}
	}

	// 3. Read the floor values
	let vorbis_floor_count :u8 = try!(rdr.read_u6()) + 1;
	let mut floors = Vec::with_capacity(vorbis_floor_count as usize);
	for _ in 0 .. vorbis_floor_count {
		floors.push(try!(read_floor(&mut rdr, vorbis_codebook_count, blocksizes)));
	}

	// 4. Read the residue values
	let vorbis_residue_count :u8 = try!(rdr.read_u6()) + 1;
	let mut residues = Vec::with_capacity(vorbis_residue_count as usize);
	for _ in 0 .. vorbis_residue_count {
		residues.push(try!(read_residue(&mut rdr, &codebooks)));
	}

	// 5. Read the mappings
	let vorbis_mapping_count :u8 = try!(rdr.read_u6()) + 1;
	let mut mappings = Vec::with_capacity(vorbis_mapping_count as usize);
	for _ in 0 .. vorbis_mapping_count {
		mappings.push(try!(read_mapping(& mut rdr,
			audio_chan_ilog, audio_channels,
			vorbis_floor_count, vorbis_residue_count)));
	}

	// 6. Read the modes
	let vorbis_mode_count :u8 = try!(rdr.read_u6()) + 1;
	let mut modes = Vec::with_capacity(vorbis_mode_count as usize);
	for _ in 0 .. vorbis_mode_count {
		modes.push(try!(read_mode_info(& mut rdr, vorbis_mapping_count)));
	}

	// Now we only have to make sure the framing bit is set,
	// and we can successfully return the setup header!
	let framing :bool = try!(rdr.read_bit_flag());
	if !framing {
		try!(Err(HeaderReadError::HeaderBadFormat));
	}

	return Ok(SetupHeader {
		codebooks,
		floors,
		residues,
		mappings,
		modes,
	});
}
