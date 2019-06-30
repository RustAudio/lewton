use std::os::raw::c_int;

/// Main Decoder State
///
/// It is created by `lewton_context_from_extradata` by passing a xiph-laced extradate bundle
pub struct LewtonContext {

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
pub struct LewtonSamples {

}

/// Create a LewtonContext from an extradata buffer
///
/// Returns either NULL or a newly allocated LewtonContext
#[no_mangle]
pub unsafe extern "C" fn lewton_context_from_extradata(
		data :*const u8, len :usize) -> *mut LewtonContext {
	unimplemented!()
}

/// Reset the Decoder to support seeking.
#[no_mangle]
pub unsafe extern "C" fn lewton_context_reset(ctx :*mut LewtonContext) {
	unimplemented!()
}

/// Decode a packet to LewtonSamples when possible
///
/// Returns 0 on success, non-zero if no samples can be produced
#[no_mangle]
pub unsafe extern "C" fn lewton_decode_packet(ctx :*mut LewtonContext,
		pkt :*const u8, len: usize,
		sample_out :*mut *mut LewtonSamples) -> c_int {
	unimplemented!()
}

/// Provide the number of samples present in each channel
#[no_mangle]
pub unsafe extern "C" fn lewton_samples_count(ctx :*const LewtonSamples) -> usize {
	unimplemented!()
}

/// Provide a reference to the channel sample data
pub unsafe extern "C" fn lewton_samples_f32(samples :*mut LewtonSamples, channel :usize) -> *const f32 {
	unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn lewton_samples_drop(samples :*mut *mut LewtonSamples) {
	unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn lewton_context_drop(ctx :*mut *mut LewtonContext) {
	unimplemented!()
}
