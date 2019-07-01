use std::os::raw::c_int;
use std::slice::from_raw_parts;
use std::ptr::null_mut;

use ::header::{read_header_setup, //read_header_comment,
	read_header_ident, IdentHeader, //CommentHeader,
	SetupHeader};
use ::audio::{PreviousWindowRight, read_audio_packet_generic};

/// Main Decoder State
///
/// It is created by `lewton_context_from_extradata` by passing a xiph-laced extradate bundle
pub struct LewtonContext {
	pwr :PreviousWindowRight,

	ident_hdr :IdentHeader,
	//comment_hdr :CommentHeader,
	setup_hdr :SetupHeader,
}

fn read_xiph_lacing(arr :&mut &[u8]) -> Option<u64> {
	let mut r = 0;
	loop {
		if arr.len() == 0 {
			return None;
		}
		let v = arr[0] as u64;
		*arr = &arr[1..];
		r += v;
		if v < 255 {
			return Some(r);
		}
	}
}

impl LewtonContext {
	fn from_extradata(mut extradata :&[u8]) -> Option<Self> {
		// We must start with a 2 as per matroska encapsulation spec
		if extradata.len() == 0 || extradata[0] != 2 {
			return None
		}
		extradata = &extradata[1..];
		let ident_len = read_xiph_lacing(&mut extradata)? as usize;
		let comment_len = read_xiph_lacing(&mut extradata)? as usize;

		let ident_hdr = read_header_ident(&extradata[0..ident_len]).ok()?;
		extradata = &extradata[ident_len..];
		//let comment_hdr = read_header_comment(&extradata[0..comment_len]).ok()?;
		extradata = &extradata[comment_len..];
		let setup_hdr = read_header_setup(extradata, ident_hdr.audio_channels,
			(ident_hdr.blocksize_0, ident_hdr.blocksize_1))
			.ok()?;
		Some(LewtonContext {
			pwr : PreviousWindowRight::new(),

			ident_hdr,
			//comment_hdr,
			setup_hdr,
		})
	}
}

/// A multichannel vector of samples
///
/// It is produced by `lewton_decode_packet`
///
/// Use `lewton_samples_count` to retrieve the number of samples available in each channel
/// Use `lewton_samples_channels` to retrieve the number of channels
/// Use `lewton_samples_for_channel_f32` to retrieve a reference to the data present in the
/// channel
///
/// use `lewton_samples_drop()` to deallocate the memory
pub struct LewtonSamples(Vec<Vec<f32>>);

/// Create a LewtonContext from an extradata buffer
///
/// Returns either NULL or a newly allocated LewtonContext
#[no_mangle]
pub unsafe extern fn lewton_context_from_extradata(
		data :*const u8, len :usize) -> *mut LewtonContext {
	if data.is_null() {
		return null_mut();
	}
	let extradata = from_raw_parts(data, len);
	if let Some(cx) = LewtonContext::from_extradata(extradata) {
		let boxed = Box::new(cx);
		Box::into_raw(boxed)
	} else {
		null_mut()
	}
}

/// Reset the Decoder to support seeking.
#[no_mangle]
pub unsafe extern fn lewton_context_reset(ctx :*mut LewtonContext) {
	(*ctx).pwr = PreviousWindowRight::new();
}

/// Decode a packet to LewtonSamples when possible
///
/// Returns 0 on success, non-zero if no samples can be produced
#[no_mangle]
pub unsafe extern fn lewton_decode_packet(ctx :*mut LewtonContext,
		pkt :*const u8, len: usize,
		sample_out :*mut *mut LewtonSamples) -> c_int {
	if pkt.is_null() || ctx.is_null() || sample_out.is_null() {
		return 1;
	}
	let pkt = from_raw_parts(pkt, len);
	let decoded = read_audio_packet_generic(&(*ctx).ident_hdr,
			&(*ctx).setup_hdr, &pkt, &mut (*ctx).pwr);
	let decoded = if let Ok(v) = decoded {
		v
	} else {
		return 2;
	};
	let boxed = Box::new(LewtonSamples(decoded));
	*sample_out = Box::into_raw(boxed);
	return 0;
}

/// Provide the number of samples present in each channel
#[no_mangle]
pub unsafe extern fn lewton_samples_count(samples :*const LewtonSamples) -> usize {
	(*samples).0
		.get(0)
		.map(|v| v.len())
		.unwrap_or(0)
}

/// Provide a reference to the channel sample data
pub unsafe extern fn lewton_samples_f32(samples :*const LewtonSamples, channel :usize) -> *const f32 {
	(*samples).0
		.get(channel)
		.map(|v| v.as_ptr())
		.unwrap_or(std::ptr::null())
}

#[no_mangle]
pub unsafe extern fn lewton_samples_drop(samples :*mut LewtonSamples) {
	std::mem::drop(Box::from_raw(samples));
}

#[no_mangle]
pub unsafe extern fn lewton_context_drop(ctx :*mut LewtonContext) {
	std::mem::drop(Box::from_raw(ctx));
}
