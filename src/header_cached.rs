// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

/*!
Cached header info

This mod contains logic to generate and deal with
data derived from header information
that's used later in the decode process.

The caching is done to speed up decoding.
*/

pub struct TwiddleFactors {
	pub a :Vec<f32>,
	pub b :Vec<f32>,
	pub c :Vec<f32>,
}

pub struct CachedBlocksizeDerived {
	pub twiddle_factors : TwiddleFactors,
	pub window_slope : Vec<f32>,
	pub bitrev : Vec<u32>,
}

impl CachedBlocksizeDerived {
	pub fn from_blocksize(bs :u8) -> Self {
		CachedBlocksizeDerived {
			window_slope : generate_window((1 << (bs as u16)) >> 1),
			twiddle_factors : compute_twiddle_factors(bs),
			bitrev : compute_bitreverse(bs),
		}
	}
}

fn win_slope(x :u16, n :u16) -> f32 {
	// please note that there might be a MISTAKE
	// in how the spec specifies the right window slope
	// function. See "4.3.1. packet type, mode and window decode"
	// step 7 where it adds an "extra" pi/2.
	// The left slope doesn't have it, only the right one.
	// as stb_vorbis shares the window slope generation function,
	// The *other* possible reason is that we don't need the right
	// window for anything. TODO investigate this more.
	let v = (0.5 * ::std::f32::consts::PI * (x as f32 + 0.5) / n as f32).sin();
	return (0.5 * ::std::f32::consts::PI * v * v ).sin();
}

fn generate_window(n :u16) -> Vec<f32> {
	let mut window = Vec::with_capacity(n as usize);
	for i in 0 .. n {
		window.push(win_slope(i, n));
	}
	return window;
}

fn compute_twiddle_factors(blocksize :u8) -> TwiddleFactors {
	let n = 1 << (blocksize as u16);

	let n2 = n >> 1;
	let n4 = n >> 2;
	let n8 = n >> 3;

	let mut a = Vec::with_capacity(n2);
	let mut b = Vec::with_capacity(n2);
	let mut c = Vec::with_capacity(n4);

	let mut k2 = 0;

	let pi_4_n = 4.0 * ::std::f32::consts::PI / (n as f32);
	let pi_05_n = 0.5 * ::std::f32::consts::PI / (n as f32);
	let pi_2_n = 2.0 * ::std::f32::consts::PI / (n as f32);

	for k in 0..n4 {
		a.push( f32::cos((k as f32)      * pi_4_n));
		a.push(-f32::sin((k as f32)      * pi_4_n));
		b.push( f32::cos(((k2+1) as f32) * pi_05_n) * 0.5);
		b.push( f32::sin(((k2+1) as f32) * pi_05_n) * 0.5);
		k2 += 2;
	}
	k2 = 0;
	for _ in 0..n8 {
		c.push( f32::cos(((k2 + 1) as f32) * pi_2_n));
		c.push(-f32::sin(((k2 + 1) as f32) * pi_2_n));
		k2 += 2;
	}
	return TwiddleFactors {
		a : a,
		b : b,
		c : c,
	};
}

fn compute_bitreverse(blocksize :u8) -> Vec<u32> {
	let ld = blocksize as u16;
	let n = 1 << blocksize;
	let n8 = n >> 3;
	let mut rev = Vec::with_capacity(n8);
	for i in 0 .. n8 {
		rev.push((::bit_reverse(i as u32) as u32 >> (32 - ld + 3)) << 2);
	}
	return rev;
}

#[test]
fn test_compute_bitreverse() {
	let br = compute_bitreverse(8);
	// The output was generated from the output of the
	// original stb_vorbis function.
	let cmp_arr = &[
		0,   64,  32,  96,
		16,  80,  48, 112,
		8,   72,  40, 104,
		24,  88,  56, 120,
		4,   68,  36, 100,
		20,  84,  52, 116,
		12,  76,  44, 108,
		28,  92,  60, 124];
	assert_eq!(br, cmp_arr);
}

#[inline]
fn bark(x :f32) -> f32 {
	13.1 * (0.00074 * x).atan() + 2.24 * (0.0000000185*x*x).atan() + 0.0001 * x
}

/// Precomputes bark map values used by floor type 0 packets
///
/// Precomputes the cos(omega) values for use by floor type 0 computation.
///
/// Note that there is one small difference to the spec: the output
/// vec is n elements long, not n+1. The last element (at index n)
/// is -1 in the spec, we lack it. Users of the result of this function
/// implementation should use it "virtually".
pub fn compute_bark_map_cos_omega(n :u16, floor0_rate :u16,
		floor0_bark_map_size :u16) -> Vec<f32> {
	let mut res = Vec::with_capacity(n as usize);
	let hfl = floor0_rate as f32 / 2.0;
	let hfl_dn = hfl / n as f32;
	let foobar_const_part = floor0_bark_map_size as f32 / bark(hfl);
	// Bark map size minus 1:
	let bms_m1 = floor0_bark_map_size as f32 - 1.0;
	let omega_factor = ::std::f32::consts::PI / floor0_bark_map_size as f32;
	for i in 0 .. n {
		let foobar = (bark(i as f32 * hfl_dn) * foobar_const_part).floor();
		let map_elem = foobar.min(bms_m1);
		let cos_omega = (map_elem * omega_factor).cos();
		res.push(cos_omega);
	}
	return res;
}
