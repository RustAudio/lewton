// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

/*!
Audio packet decoding

This module decodes the audio packets given to it.
*/

#[allow(unused_imports)]
use imdct;
use std::error;
use std::fmt;
use std::cmp::min;
use std::iter;
use tinyvec::TinyVec;
use ::ilog;
use ::bitpacking::BitpackCursor;
use ::header::{Codebook, Floor, FloorTypeZero, FloorTypeOne,
	HuffmanVqReadErr, IdentHeader, Mapping, Residue, SetupHeader};
use samples::Samples;

#[derive(Debug, PartialEq, Eq)]
pub enum AudioReadError {
	EndOfPacket,
	AudioBadFormat,
	AudioIsHeader,
	/// If the needed memory isn't addressable by us
	///
	/// This error is returned if a calculation yielded a higher value for
	/// an internal buffer size that doesn't fit into the platform's address range.
	/// Note that if we "simply" encounter an allocation failure (OOM, etc),
	/// we do what libstd does in these cases: crash.
	///
	/// This error is not automatically an error of the format,
	/// but rather is due to insufficient decoder hardware.
	BufferNotAddressable,
}

// For the () error type returned by the bitpacking layer
// TODO that type choice was a bit unfortunate,
// perhaps one day fix this
impl From<()> for AudioReadError {
	fn from(_ :()) -> AudioReadError {
		AudioReadError::EndOfPacket
	}
}

impl error::Error for AudioReadError {}

impl fmt::Display for AudioReadError {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		let description = match self {
			AudioReadError::EndOfPacket => "End of packet reached.",
			AudioReadError::AudioBadFormat => "Invalid audio packet",
			AudioReadError::AudioIsHeader => "The vorbis version is not supported",
			AudioReadError::BufferNotAddressable => "Requested to create buffer of non-addressable size",
		};
		write!(fmt, "{}", description)
	}
}

enum DecodedFloor<'a> {
	TypeZero(Vec<f32>, u64, &'a FloorTypeZero),
	TypeOne(Vec<u32>, &'a FloorTypeOne),
	Unused,
}

impl <'a> DecodedFloor<'a> {
	fn is_unused(&self) -> bool {
		match self {
			&DecodedFloor::Unused => true,
			_ => false,
		}
	}
}

enum FloorSpecialCase {
	Unused,
	PacketUndecodable,
}

impl From<()> for FloorSpecialCase {
	fn from(_ :()) -> Self {
		// () always means end of packet condition in the places
		// the conversion is used.
		return FloorSpecialCase::Unused;
	}
}

impl From<HuffmanVqReadErr> for FloorSpecialCase {
	fn from(e :HuffmanVqReadErr) -> Self {
		use ::header::HuffmanVqReadErr::*;
		use self::FloorSpecialCase::*;
		match e {
			EndOfPacket => Unused,
			// Undecodable per spec, see paragraph about
			// VQ lookup type zero in section 3.3.
			NoVqLookupForCodebook => PacketUndecodable,
		}
	}
}

// Note that the output vector contains the cosine values of the coefficients,
// not the bare values like in the spec. This is in order to optimize.
fn floor_zero_decode(rdr :&mut BitpackCursor, codebooks :&[Codebook],
		fl :&FloorTypeZero) -> Result<(Vec<f32>, u64), FloorSpecialCase> {
	// TODO this needs to become 128 bits wide, not just 64,
	// as floor0_amplitude_bits can be up to 127.
	let amplitude = try!(rdr.read_dyn_u64(fl.floor0_amplitude_bits));
	if amplitude <= 0 {
		// This channel is unused in this frame,
		// its all zeros.
		return Err(FloorSpecialCase::Unused);
	}

	let booknumber = try!(rdr.read_dyn_u32(
		::ilog(fl.floor0_number_of_books as u64)));
	match fl.floor0_book_list.get(booknumber as usize) {
		// Undecodable per spec
		None => try!(Err(FloorSpecialCase::PacketUndecodable)),
		Some(codebook_idx) => {
			let mut coefficients = Vec::with_capacity(fl.floor0_order as usize);
			let mut last = 0.0;
			let codebook = &codebooks[*codebook_idx as usize];
			loop {
				let mut last_new = last;
				let temp_vector = try!(rdr.read_huffman_vq(codebook));
				if temp_vector.len() + coefficients.len() < fl.floor0_order as usize {
					// Little optimisation: we don't have to care about the >= case here
					for &e in temp_vector {
						coefficients.push((last + e as f32).cos());
						last_new = e as f32;
					}
				} else {
					for &e in temp_vector {
						coefficients.push((last + e as f32).cos());
						last_new = e as f32;
						// This rule makes sure that coefficients doesn't get
						// larger than floor0_order and saves an allocation
						// in this case
						if coefficients.len() == fl.floor0_order as usize {
							return Ok((coefficients, amplitude));
						}
					}
				}
				last += last_new;
				if coefficients.len() >= fl.floor0_order as usize {
					return Ok((coefficients, amplitude));
				}
			}
		},
	}
	unreachable!();
}

fn floor_zero_compute_curve(cos_coefficients :&[f32], amplitude :u64,
		fl :&FloorTypeZero, blockflag :bool, n :u16) -> Vec<f32> {
	let cached_bark_cos_omega =
		&fl.cached_bark_cos_omega[blockflag as usize];
	let mut i = 0;
	let mut output = Vec::with_capacity(n as usize);
	let lfv_common_term = amplitude as f32 * fl.floor0_amplitude_offset as f32 /
		((1 << fl.floor0_amplitude_bits) - 1) as f32;
	while i < n as usize {
		let cos_omega = cached_bark_cos_omega[i];

		// Compute p and q
		let (p_upper_border, q_upper_border) =
		if fl.floor0_order & 1 == 1 {
			((fl.floor0_order as usize - 3) / 2,
				(fl.floor0_order as usize - 1) / 2)
		} else {
			let v = (fl.floor0_order as usize - 2) / 2;
			(v, v)
		};
		let (mut p, mut q) =
		if fl.floor0_order & 1 == 1 {
			(1.0 - cos_omega * cos_omega, 0.25)
		} else {
			((1.0 - cos_omega) / 2.0, (1.0 + cos_omega) / 2.0)
		};
		for j in 0 .. p_upper_border + 1 {
			let pm = cos_coefficients[2 * j + 1] - cos_omega;
			p *= 4.0 * pm * pm;
		}
		for j in 0 .. q_upper_border + 1 {
			let qm = cos_coefficients[2 * j] - cos_omega;
			q *= 4.0 * qm * qm;
		}

		// Compute linear_floor_value
		let linear_floor_value = (0.11512925 *
			(lfv_common_term / (p+q).sqrt() - fl.floor0_amplitude_offset as f32)
		).exp();

		// Write into output
		let mut iteration_condition = cos_omega;
		while cos_omega == iteration_condition {
			output.push(linear_floor_value);
			i += 1;
			iteration_condition = match cached_bark_cos_omega.get(i) {
				Some(v) => *v,
				None => break,
			};
		}
	}
	return output;
}

// Returns Err if the floor is "unused"
fn floor_one_decode(rdr :&mut BitpackCursor, codebooks :&[Codebook],
		fl :&FloorTypeOne) -> Result<Vec<u32>, FloorSpecialCase> {
	// TODO perhaps it means invalid audio packet if reading the nonzero
	// flag doesn't succeed bc end of packet. Perhaps it does not.
	if !try!(rdr.read_bit_flag()) {
		try!(Err(()));
	}
	let mut floor1_y = Vec::new();
	let v = &[256, 128, 86, 64];
	let range = v[(fl.floor1_multiplier - 1) as usize];
	let b = ::ilog(range - 1);
	floor1_y.push(try!(rdr.read_dyn_u8(b)) as u32);
	floor1_y.push(try!(rdr.read_dyn_u8(b)) as u32);

	for class in &fl.floor1_partition_class {
		let uclass = *class as usize;
		let cdim = fl.floor1_class_dimensions[uclass];
		let cbits = fl.floor1_class_subclasses[uclass];
		let csub = (1 << cbits) - 1;
		let mut cval = 0;
		if cbits > 0 {
			let cbook = fl.floor1_class_masterbooks[uclass] as usize;
			cval = try!(rdr.read_huffman(&codebooks[cbook].codebook_huffman_tree));
		}
		for _ in 0 .. cdim {
			let book = fl.floor1_subclass_books[uclass][(cval & csub) as usize];
			cval >>= cbits;
			if book >= 0 {
				let tree = &codebooks[book as usize].codebook_huffman_tree;
				floor1_y.push(try!(rdr.read_huffman(tree)));
			} else {
				floor1_y.push(0);
			}
		}
	}
	return Ok(floor1_y);
}

fn extr_neighbor<F>(v :&[u32], max_idx :usize,
		compare :F, relation :&str) -> (usize, u32)
		where F :Fn(u32, u32) -> std::cmp::Ordering {
	use std::cmp::Ordering;

	let bound = v[max_idx];
	let prefix = &v[..max_idx];
	let smaller = |a, b| compare(a, b) == Ordering::Less;

	// First find a first index that fulfills
	// the criterion of being "smaller" than bound
	let min_idx = prefix.iter()
		.position(|&val| smaller(val, bound))
		.unwrap_or_else(||
			panic!("No index y < {0} found where v[y] is {1} than v[{0}] = 0x{2:08x}!",
				max_idx, relation, bound));

	// Now search for "bigger" entries
	let (offset, max_neighbor) = prefix[min_idx..].iter().cloned()
		.enumerate()
		// According to documentation of Iterator::max_by,
		// "If several elements are equally maximum, the last element is returned".
		// Thus, in order to find the *first* maximum element,
		// we need to search from the end of `prefix`
		.rev()
		.filter(|&(_i, val)| smaller(val, bound))
		.max_by(|&(_, a), &(_, b)| compare(a, b))
		.unwrap_or((0, v[min_idx]));

	(min_idx + offset, max_neighbor)
}

fn low_neighbor(v :&[u32], x :usize) -> (usize, u32) {
	extr_neighbor(v, x, |a, b| a.cmp(&b), "smaller")
}


fn high_neighbor(v :&[u32], x :usize) -> (usize, u32) {
	extr_neighbor(v, x, |a, b| b.cmp(&a), "bigger")
}

#[test]
fn test_low_neighbor() {
	let v = [1, 4, 2, 3, 6, 5];
	// 0 will panic
	assert_eq!(low_neighbor(&v, 1), (0, 1));
	assert_eq!(low_neighbor(&v, 2), (0, 1));
	assert_eq!(low_neighbor(&v, 3), (2, 2));
	assert_eq!(low_neighbor(&v, 4), (1, 4));
	assert_eq!(low_neighbor(&v, 5), (1, 4));
}


#[test]
fn test_high_neighbor() {
	let v = [1, 4, 2, 3, 6, 5];
	// 0, 1 will panic
	assert_eq!(high_neighbor(&v, 2), (1, 4));
	assert_eq!(high_neighbor(&v, 3), (1, 4));
	// 4 will panic
	assert_eq!(high_neighbor(&v, 5), (4, 6));
}

#[test]
fn test_high_neighbor_ex() {
	// Data extracted from example file
	let v = [0, 128, 12, 46, 4, 8, 16, 23,
		33, 70, 2, 6, 10, 14, 19, 28, 39, 58, 90];

	// 0, 1 will panic
	assert_eq!(high_neighbor(&v, 2), (1, 128));
	assert_eq!(high_neighbor(&v, 3), (1, 128));
	assert_eq!(high_neighbor(&v, 4), (2, 12));
	assert_eq!(high_neighbor(&v, 5), (2, 12));
	assert_eq!(high_neighbor(&v, 6), (3, 46));
	assert_eq!(high_neighbor(&v, 7), (3, 46));
	assert_eq!(high_neighbor(&v, 8), (3, 46));
	assert_eq!(high_neighbor(&v, 9), (1, 128));
	assert_eq!(high_neighbor(&v, 10), (4, 4));
	assert_eq!(high_neighbor(&v, 11), (5, 8));
	assert_eq!(high_neighbor(&v, 12), (2, 12));
	assert_eq!(high_neighbor(&v, 13), (6, 16));
	assert_eq!(high_neighbor(&v, 14), (7, 23));
	assert_eq!(high_neighbor(&v, 15), (8, 33));
	assert_eq!(high_neighbor(&v, 16), (3, 46));
	assert_eq!(high_neighbor(&v, 17), (9, 70));
	assert_eq!(high_neighbor(&v, 18), (1, 128));
}

#[test]
#[should_panic]
fn test_high_neighbor_panic() {
	high_neighbor(&[1, 4, 3, 2, 6, 5], 4);
}

#[test]
#[should_panic]
fn test_low_neighbor_panic() {
	low_neighbor(&[2, 4, 3, 1, 6, 5], 3);
}

fn render_point(x0 :u32, y0 :u32, x1 :u32, y1 :u32, x :u32) -> u32 {
	// TODO find out whether the type choices in this method are okay
	// (esp. the i32 choice).
	let dy = y1 as i32 - y0 as i32;
	let adx = x1 - x0;
	let ady = dy.abs() as u32;
	let err = ady * (x - x0);
	let off = err / adx;
	if dy < 0 {
		return y0 - off;
	} else {
		return y0 + off;
	}
}

#[test]
fn test_render_point() {
	// Test data taken from real life ogg/vorbis file.
	assert_eq!(render_point(0, 28, 128, 67, 12), 31);
	assert_eq!(render_point(12, 38, 128, 67, 46), 46);
	assert_eq!(render_point(0, 28, 12, 38, 4), 31);
	assert_eq!(render_point(4, 33, 12, 38, 8), 35);
	assert_eq!(render_point(12, 38, 46, 31, 16), 38);
	assert_eq!(render_point(16, 30, 46, 31, 23), 30);
	assert_eq!(render_point(23, 40, 46, 31, 33), 37);
	assert_eq!(render_point(46, 31, 128, 67, 70), 41);
	assert_eq!(render_point(0, 28, 4, 33, 2), 30);
	assert_eq!(render_point(4, 33, 8, 43, 6), 38);
	assert_eq!(render_point(8, 43, 12, 38, 10), 41);
	assert_eq!(render_point(12, 38, 16, 30, 14), 34);
	assert_eq!(render_point(16, 30, 23, 40, 19), 34);
	assert_eq!(render_point(23, 40, 33, 26, 28), 33);
	assert_eq!(render_point(33, 26, 46, 31, 39), 28);
	assert_eq!(render_point(46, 31, 70, 20, 58), 26);
	assert_eq!(render_point(70, 20, 128, 67, 90), 36);
}

fn floor_one_curve_compute_amplitude(floor1_y :&[u32], fl :&FloorTypeOne) -> (Vec<u32>, Vec<bool>) {
	let v = &[256, 128, 86, 64];
	let range = v[(fl.floor1_multiplier - 1) as usize] as i32;
	let mut floor1_step2_flag = Vec::new();
	floor1_step2_flag.push(true);
	floor1_step2_flag.push(true);
	let mut floor1_final_y = Vec::new();
	floor1_final_y.push(floor1_y[0]);
	floor1_final_y.push(floor1_y[1]);

	for (i, el) in fl.floor1_x_list.iter().enumerate().skip(2) {
		let cur_low_neighbor = low_neighbor(&fl.floor1_x_list, i);
		let cur_high_neighbor = high_neighbor(&fl.floor1_x_list, i);
		let predicted = render_point(
			cur_low_neighbor.1, floor1_final_y[cur_low_neighbor.0],
			cur_high_neighbor.1, floor1_final_y[cur_high_neighbor.0], *el) as i32;
		let val = floor1_y[i] as i32;
		let highroom = range - predicted;
		let lowroom = predicted;
		let room = min(highroom, lowroom) * 2;
		if val > 0 {
			floor1_step2_flag[cur_low_neighbor.0] = true;
			floor1_step2_flag[cur_high_neighbor.0] = true;
			floor1_step2_flag.push(true);
			floor1_final_y.push(if val >= room {
				if highroom > lowroom {
					predicted + val - lowroom
				} else {
					predicted - val + highroom - 1
				}
			} else {
				predicted + (if val % 2 == 1 {
					- val - 1 } else { val } >> 1)
			} as u32);
		} else {
			floor1_final_y.push(predicted as u32);
			floor1_step2_flag.push(false);
		}
	}
	// Clamp all entries of floor1_final_y to range
	for el in &mut floor1_final_y {
		*el = min(range as u32 - 1, *el);
	}
	return (floor1_final_y, floor1_step2_flag);
}

static FLOOR1_INVERSE_DB_TABLE :&[f32] = &[
	1.0649863e-07, 1.1341951e-07, 1.2079015e-07, 1.2863978e-07,
	1.3699951e-07, 1.4590251e-07, 1.5538408e-07, 1.6548181e-07,
	1.7623575e-07, 1.8768855e-07, 1.9988561e-07, 2.1287530e-07,
	2.2670913e-07, 2.4144197e-07, 2.5713223e-07, 2.7384213e-07,
	2.9163793e-07, 3.1059021e-07, 3.3077411e-07, 3.5226968e-07,
	3.7516214e-07, 3.9954229e-07, 4.2550680e-07, 4.5315863e-07,
	4.8260743e-07, 5.1396998e-07, 5.4737065e-07, 5.8294187e-07,
	6.2082472e-07, 6.6116941e-07, 7.0413592e-07, 7.4989464e-07,
	7.9862701e-07, 8.5052630e-07, 9.0579828e-07, 9.6466216e-07,
	1.0273513e-06, 1.0941144e-06, 1.1652161e-06, 1.2409384e-06,
	1.3215816e-06, 1.4074654e-06, 1.4989305e-06, 1.5963394e-06,
	1.7000785e-06, 1.8105592e-06, 1.9282195e-06, 2.0535261e-06,
	2.1869758e-06, 2.3290978e-06, 2.4804557e-06, 2.6416497e-06,
	2.8133190e-06, 2.9961443e-06, 3.1908506e-06, 3.3982101e-06,
	3.6190449e-06, 3.8542308e-06, 4.1047004e-06, 4.3714470e-06,
	4.6555282e-06, 4.9580707e-06, 5.2802740e-06, 5.6234160e-06,
	5.9888572e-06, 6.3780469e-06, 6.7925283e-06, 7.2339451e-06,
	7.7040476e-06, 8.2047000e-06, 8.7378876e-06, 9.3057248e-06,
	9.9104632e-06, 1.0554501e-05, 1.1240392e-05, 1.1970856e-05,
	1.2748789e-05, 1.3577278e-05, 1.4459606e-05, 1.5399272e-05,
	1.6400004e-05, 1.7465768e-05, 1.8600792e-05, 1.9809576e-05,
	2.1096914e-05, 2.2467911e-05, 2.3928002e-05, 2.5482978e-05,
	2.7139006e-05, 2.8902651e-05, 3.0780908e-05, 3.2781225e-05,
	3.4911534e-05, 3.7180282e-05, 3.9596466e-05, 4.2169667e-05,
	4.4910090e-05, 4.7828601e-05, 5.0936773e-05, 5.4246931e-05,
	5.7772202e-05, 6.1526565e-05, 6.5524908e-05, 6.9783085e-05,
	7.4317983e-05, 7.9147585e-05, 8.4291040e-05, 8.9768747e-05,
	9.5602426e-05, 0.00010181521, 0.00010843174, 0.00011547824,
	0.00012298267, 0.00013097477, 0.00013948625, 0.00014855085,
	0.00015820453, 0.00016848555, 0.00017943469, 0.00019109536,
	0.00020351382, 0.00021673929, 0.00023082423, 0.00024582449,
	0.00026179955, 0.00027881276, 0.00029693158, 0.00031622787,
	0.00033677814, 0.00035866388, 0.00038197188, 0.00040679456,
	0.00043323036, 0.00046138411, 0.00049136745, 0.00052329927,
	0.00055730621, 0.00059352311, 0.00063209358, 0.00067317058,
	0.00071691700, 0.00076350630, 0.00081312324, 0.00086596457,
	0.00092223983, 0.00098217216, 0.0010459992,  0.0011139742,
	0.0011863665,  0.0012634633,  0.0013455702,  0.0014330129,
	0.0015261382,  0.0016253153,  0.0017309374,  0.0018434235,
	0.0019632195,  0.0020908006,  0.0022266726,  0.0023713743,
	0.0025254795,  0.0026895994,  0.0028643847,  0.0030505286,
	0.0032487691,  0.0034598925,  0.0036847358,  0.0039241906,
	0.0041792066,  0.0044507950,  0.0047400328,  0.0050480668,
	0.0053761186,  0.0057254891,  0.0060975636,  0.0064938176,
	0.0069158225,  0.0073652516,  0.0078438871,  0.0083536271,
	0.0088964928,  0.009474637,   0.010090352,   0.010746080,
	0.011444421,   0.012188144,   0.012980198,   0.013823725,
	0.014722068,   0.015678791,   0.016697687,   0.017782797,
	0.018938423,   0.020169149,   0.021479854,   0.022875735,
	0.024362330,   0.025945531,   0.027631618,   0.029427276,
	0.031339626,   0.033376252,   0.035545228,   0.037855157,
	0.040315199,   0.042935108,   0.045725273,   0.048696758,
	0.051861348,   0.055231591,   0.058820850,   0.062643361,
	0.066714279,   0.071049749,   0.075666962,   0.080584227,
	0.085821044,   0.091398179,   0.097337747,   0.10366330,
	0.11039993,    0.11757434,    0.12521498,    0.13335215,
	0.14201813,    0.15124727,    0.16107617,    0.17154380,
	0.18269168,    0.19456402,    0.20720788,    0.22067342,
	0.23501402,    0.25028656,    0.26655159,    0.28387361,
	0.30232132,    0.32196786,    0.34289114,    0.36517414,
	0.38890521,    0.41417847,    0.44109412,    0.46975890,
	0.50028648,    0.53279791,    0.56742212,    0.60429640,
	0.64356699,    0.68538959,    0.72993007,    0.77736504,
	0.82788260,    0.88168307,    0.9389798,     1.];

fn render_line(x0 :u32, y0 :u32, x1 :u32, y1 :u32, v :&mut Vec<u32>) {
	// TODO find out whether the type choices in this method are okay
	let dy = y1 as i32 - y0 as i32;
	let adx = x1 as i32 - x0 as i32;
	let ady = dy.abs();
	let base = dy / adx;
	let mut y = y0 as i32;
	let mut err = 0;
	let sy = base + (if dy < 0 { -1 } else { 1 });
	let ady = ady  - base.abs() * adx;
	v.push(y as u32);
	for _ in (x0 + 1) .. x1 {
		err += ady;
		if err >= adx {
			err -= adx;
			y += sy;
		} else {
			y += base;
		}
		v.push(y as u32);
	}
}

fn floor_one_curve_synthesis(floor1_final_y :Vec<u32>,
		floor1_step2_flag :Vec<bool>, fl :&FloorTypeOne, n :u16) -> Vec<f32> {
	let floor1_final_y_s = |i :usize| { floor1_final_y[fl.floor1_x_list_sorted[i].0] };
	let floor1_x_list_s = |i :usize| { fl.floor1_x_list_sorted[i].1 };
	let floor1_step2_flag_s = |i :usize| {
		floor1_step2_flag[fl.floor1_x_list_sorted[i].0] };
	let mut hx = 0;
	let mut lx = 0;
	let mut hy = 0;
	let mut floor = Vec::with_capacity(n as usize);
	let mut ly = floor1_final_y_s(0) * fl.floor1_multiplier as u32;
	for i in 1 .. fl.floor1_x_list.len() {
		if floor1_step2_flag_s(i) {
			hy = floor1_final_y_s(i) * fl.floor1_multiplier as u32;
			hx = floor1_x_list_s(i);
			render_line(lx, ly, hx, hy, &mut floor);
			lx = hx;
			ly = hy;
		}
	}
	if hx < n as u32 {
		render_line(hx, hy, n as u32, hy, &mut floor);
	} else if hx > n as u32 {
		floor.truncate(n as usize);
	}

	floor.into_iter()
		.map(|idx| FLOOR1_INVERSE_DB_TABLE[idx as usize])
		.collect()
}

fn floor_decode<'a>(rdr :&mut BitpackCursor,
		ident :&IdentHeader, mapping :&Mapping, codebooks :&[Codebook],
		floors :&'a [Floor]) -> Result<Vec<DecodedFloor<'a>>, ()> {
	let mut decoded_floor_infos = Vec::with_capacity(ident.audio_channels as usize);
	for i in 0 .. ident.audio_channels as usize {
		let submap_number = mapping.mapping_mux[i] as usize;
		let floor_number = mapping.mapping_submap_floors[submap_number];
		let floor = &floors[floor_number as usize];
		use self::FloorSpecialCase::*;
		let floor_res = match floor {
			&Floor::TypeZero(ref fl) => {
				match floor_zero_decode(rdr, codebooks, fl) {
					Ok((coeff, amp)) => DecodedFloor::TypeZero(coeff, amp, fl),
					Err(Unused) => DecodedFloor::Unused,
					Err(PacketUndecodable) => try!(Err(())),
				}
			},
			&Floor::TypeOne(ref fl) => {
				match floor_one_decode(rdr, codebooks, fl) {
					Ok(dfl) => DecodedFloor::TypeOne(dfl, fl),
					Err(Unused) => DecodedFloor::Unused,
					Err(PacketUndecodable) => try!(Err(())),
				}
			},
		};
		decoded_floor_infos.push(floor_res);
	}
	return Ok(decoded_floor_infos);
}

fn residue_packet_read_partition(rdr :&mut BitpackCursor, codebook :&Codebook,
		resid :&Residue, vec_v :&mut [f32]) -> Result<(), HuffmanVqReadErr> {
	if resid.residue_type == 0 {
		let codebook_dimensions = codebook.codebook_dimensions as usize;
		let step = resid.residue_partition_size as usize / codebook_dimensions;
		for i in 0 .. step {
			let entry_temp = try!(rdr.read_huffman_vq(codebook));
			for (j, e) in entry_temp.iter().enumerate() {
				vec_v[i + j * step] += *e;
			}
		}
	} else {
		// Common for both format 1 and 2
		let partition_size = resid.residue_partition_size as usize;
		let mut i = 0;
		while i < partition_size {
			let entries = try!(rdr.read_huffman_vq(codebook));
			let vs = if let Some(vs) = vec_v.get_mut(i..(i + entries.len())) {
				vs
			} else {
				break;
			};

			for (v, e) in vs.iter_mut().zip(entries.iter()) {
				*v += *e;
			}

			i += entries.len();
		}
	}
	Ok(())
}

fn residue_packet_decode_inner(rdr :&mut BitpackCursor, cur_blocksize :u16,
		do_not_decode_flag :&[bool], resid :&Residue, codebooks :&[Codebook]) -> Result<Vec<f32>, ()> {

	let ch = do_not_decode_flag.len();
	let actual_size = (cur_blocksize / 2) as usize;

	// Older versions of the spec say max() here,
	// but there's been a bug in the spec.
	// It's been fixed since:
	// https://github.com/xiph/vorbis/pull/35
	let limit_residue_begin = min(resid.residue_begin as usize, actual_size);
	let limit_residue_end = min(resid.residue_end as usize, actual_size);

	let cur_codebook = &codebooks[resid.residue_classbook as usize];
	let classwords_per_codeword = cur_codebook.codebook_dimensions as usize;
	let n_to_read = limit_residue_end - limit_residue_begin;
	let partitions_to_read = n_to_read / resid.residue_partition_size as usize;
	let residue_classbok_ht = &cur_codebook.codebook_huffman_tree;

	// Allocate and zero all vectors that will be returned
	let mut vectors = vec![0.; ch * actual_size];

	if n_to_read == 0 {
		// No residue to decode
		return Ok(vectors);
	}

	if classwords_per_codeword == 0 {
		// A value of 0 would create an infinite loop.
		// Therefore, throw an error in this case.
		try!(Err(()));
	}

	'pseudo_return: loop {
		// ENdofpacketisnOrmal macro. Local replacement for try.
		macro_rules! eno {
			($expr:expr) => (match $expr {
				$crate::std::result::Result::Ok(val) => val,
				$crate::std::result::Result::Err(_) => break 'pseudo_return,
			})
		}
		let cl_stride :usize = partitions_to_read + classwords_per_codeword;
		let mut classifications = vec![0; ch as usize * cl_stride];
		for pass in 0 .. 8 {
			let mut partition_count = 0;
			while partition_count < partitions_to_read {
				if pass == 0 {
					for (j, do_not_decode) in do_not_decode_flag.iter().enumerate() {
						if *do_not_decode {
							continue;
						}
						let mut temp = eno!(rdr.read_huffman(residue_classbok_ht));
						for i in (0 .. classwords_per_codeword).rev() {
							classifications[j * cl_stride + i + partition_count] =
							temp % resid.residue_classifications as u32;
							temp = temp / resid.residue_classifications as u32;
						}
					}
				}
				for _ in 0 .. classwords_per_codeword {
					if partition_count >= partitions_to_read {
						break;
					}
					for (j, do_not_decode) in do_not_decode_flag.iter().enumerate() {
						if *do_not_decode {
							continue;
						}
						let offs = limit_residue_begin + partition_count * resid.residue_partition_size as usize;
						let vec_j_offs = &mut vectors[(j * actual_size + offs) .. ((j + 1) * actual_size)];
						let vqclass = classifications[j * cl_stride + partition_count] as usize;
						let vqbook_opt = resid.residue_books[vqclass].get_val(pass);
						if let Some(vqbook) = vqbook_opt {
							let codebook = &codebooks[vqbook as usize];
							// codebook is checked by header decode to have a value mapping
							// Decode the partition into output vector number j (vec_j).
							match residue_packet_read_partition(rdr,
									codebook, resid, vec_j_offs) {
								Ok(_) => (),
								Err(err) => {
									use ::header::HuffmanVqReadErr::*;
									match err {
										EndOfPacket => break 'pseudo_return,
										NoVqLookupForCodebook =>
											panic!("Codebook must have a value mapping"),
									}
								},
							}
						}
					}
					partition_count += 1;
				}
			}
		}
		break;
	}

	return Ok(vectors);
}


// Ok means "fine" (or end of packet, but thats "fine" too!),
// Err means "not fine" -- the whole packet must be discarded
fn residue_packet_decode(rdr :&mut BitpackCursor, cur_blocksize :u16,
		do_not_decode_flag :&[bool], resid :&Residue, codebooks :&[Codebook]) -> Result<Vec<f32>, ()> {

	let ch = do_not_decode_flag.len();
	let vec_size = (cur_blocksize / 2) as usize;

	if resid.residue_type == 2 {
		let mut to_decode_found = false;
		for do_not_decode in do_not_decode_flag {
			if !do_not_decode {
				to_decode_found = true;
				break;
			}
		}
		if !to_decode_found {
			// Don't attempt to decode, but return vectors,
			// as required per spec only residue 2 has this.
			return Ok(vec![0.; ch * vec_size]);
		} else {
			// Construct a do_not_decode flag array
			let c_do_not_decode_flag = [false];

			let vectors = try!(residue_packet_decode_inner(rdr,
				cur_blocksize * ch as u16, &c_do_not_decode_flag,
				resid, codebooks));

			// Post decode step
			let mut vectors_deinterleaved = Vec::with_capacity(ch * vec_size);
			for j in 0 .. ch {
				let iter = vectors.chunks(ch).map(|chunk| chunk[j]);
				vectors_deinterleaved.extend(iter);
			}
			return Ok(vectors_deinterleaved);
		}
	} else {
		return residue_packet_decode_inner(rdr, cur_blocksize,
			do_not_decode_flag, resid, codebooks);
	}
}

#[inline]
fn inverse_couple(m :f32, a :f32) -> (f32, f32) {
	if m > 0. {
		if a > 0. {
			(m, m - a)
		} else {
			(m + a, m)
		}
	} else {
		if a > 0. {
			(m, m + a)
		} else {
			(m - a, m)
		}
	}
}

// TODO this is probably slower than a replacement of
// this function in unsafe code, no idea
fn dual_mut_idx<T>(v :&mut [T], idx_a :usize, idx_b :usize)
		-> (&mut T, &mut T) {
	assert_ne!(idx_a, idx_b, "not allowed, indices must be different!");

	let range = if idx_a < idx_b { idx_a..idx_b+1 } else { idx_b..idx_a+1 };
	let segment = &mut v[range];
	let (first, rest) = segment.split_first_mut().unwrap();
	let (last, _) = rest.split_last_mut().unwrap();
	(first, last)
}

fn dct_iv_slow(buffer :&mut [f32]) {
	let x = buffer.to_vec();
	let n = buffer.len();
	let nmask = (n << 3) - 1;
	let mcos = (0 .. 8 * n)
		.map(|i| (std::f32::consts::FRAC_PI_4 * (i as f32) / (n as f32)).cos())
		.collect::<Vec<_>>();
	for i in 0 .. n {
		let mut acc = 0.;
		for j in 0 .. n {
			acc += x[j] * mcos[((2 * i + 1)*(2*j+1)) & nmask];
		}
		buffer[i] = acc;
	}
}

#[allow(dead_code)]
fn inverse_mdct_slow(buffer :&mut [f32]) {
	let n = buffer.len();
	let n4 = n >> 2;
	let n2 = n >> 1;
	let n3_4 = n - n4;
	let mut temp = buffer[0 .. n2].to_vec();
	dct_iv_slow(&mut temp); // returns -c'-d, a-b'
	for i in 0 .. n4 {
		buffer[i] = temp[i + n4]; // a-b'
	}
	for i in n4 .. n3_4 {
		buffer[i] = -temp[n3_4 - i - 1]; // b-a', c+d'
	}
	for i in n3_4 .. n {
		buffer[i] = -temp[i - n3_4]; // c'+d
	}
}

#[cfg(test)]
#[test]
fn test_imdct_slow() {
	use imdct_test::*;
	let mut arr_1 = imdct_prepare(&IMDCT_INPUT_TEST_ARR_1);
	inverse_mdct_slow(&mut arr_1);
	let mismatches = fuzzy_compare_array(
		&arr_1, &IMDCT_OUTPUT_TEST_ARR_1,
		0.00005, true);
	let mismatches_limit = 0;
	if mismatches > mismatches_limit {
		panic!("Numer of mismatches {} was larger than limit of {}",
			mismatches, mismatches_limit);
	}
}

/// The right part of the previous window
///
/// This is the only state that needs to be changed
/// once the headers are read.
pub struct PreviousWindowRight {
	data :Option<Vec<Vec<f32>>>,
}

impl PreviousWindowRight {
	/// Initialisation for new streams
	pub fn new() -> Self {
		return PreviousWindowRight{ data : None };
	}
	/// If the state is still uninitialized
	pub fn is_empty(&self) -> bool {
		self.data.is_none()
	}
}

/**
Returns the per-channel sample count of a packet if it were decoded.

This operation is very cheap and doesn't involve actual decoding of the packet.

Note: for the first packet in a stream, or in other instances when
the `PreviousWindowRight` is reset, the decoding functions will return
0 samples for that packet, while this function returns a different number.
Please use the `PreviousWindowRight::is_empty` function or other methods
to check for this case.
*/
pub fn get_decoded_sample_count(ident :&IdentHeader, setup :&SetupHeader, packet :&[u8])
		-> Result<usize, AudioReadError> {
	let mut rdr = BitpackCursor::new(packet);
	if try!(rdr.read_bit_flag()) {
		try!(Err(AudioReadError::AudioIsHeader));
	}
	let mode_number = try!(rdr.read_dyn_u8(ilog(setup.modes.len() as u64 - 1)));
	let mode = &setup.modes[mode_number as usize];
	let bs = if mode.mode_blockflag { ident.blocksize_1 } else { ident.blocksize_0 };
	let n :u16 = 1 << bs;
	let previous_next_window_flag = if mode.mode_blockflag {
		Some((try!(rdr.read_bit_flag()), try!(rdr.read_bit_flag())))
	} else {
		None
	};
	// Compute windowing info for left window
	let window_center = n >> 1;
	let (left_win_start, _left_win_end, _left_n, _left_n_use_bs1) =
		if previous_next_window_flag.map_or(true, |(prev_win_flag, _)| prev_win_flag) {
			(0, window_center, n >> 1, mode.mode_blockflag)
		} else {
			let bs_0_exp = 1 << ident.blocksize_0;
			((n - bs_0_exp) >> 2, (n + bs_0_exp) >> 2, bs_0_exp >> 1, false)
		};

	// Compute windowing info for right window
	let (right_win_start, _right_win_end) =
		if previous_next_window_flag.map_or(true, |(_, next_win_flag)| next_win_flag) {
			(window_center, n)
		} else {
			let bs_0_exp = 1 << ident.blocksize_0;
			((n * 3 - bs_0_exp) >> 2, (n * 3 + bs_0_exp) >> 2)
		};

	Ok((right_win_start - left_win_start) as usize)
}

#[allow(unused_variables)]
/**
Main audio packet decoding function

Pass your info to this function to get your raw packet data decoded.

Panics if the passed PreviousWindowRight struct doesn't match the info
from the ident header.
*/
pub fn read_audio_packet_generic<S :Samples>(ident :&IdentHeader, setup :&SetupHeader, packet :&[u8], pwr :&mut PreviousWindowRight)
		-> Result<S, AudioReadError> {
	let mut rdr = BitpackCursor::new(packet);
	if try!(rdr.read_bit_flag()) {
		try!(Err(AudioReadError::AudioIsHeader));
	}
	let mode_number = try!(rdr.read_dyn_u8(ilog(setup.modes.len() as u64 - 1)));
	let mode = if let Some(mode) = setup.modes.get(mode_number as usize) {
		mode
	} else {
		try!(Err(AudioReadError::AudioBadFormat))
	};
	let mapping = &setup.mappings[mode.mode_mapping as usize];
	let bs = if mode.mode_blockflag { ident.blocksize_1 } else { ident.blocksize_0 };
	let n :u16 = 1 << bs;
	let previous_next_window_flag = if mode.mode_blockflag {
		Some((try!(rdr.read_bit_flag()), try!(rdr.read_bit_flag())))
	} else {
		None
	};
	// Decode the floors
	let decoded_floor_infos = try!(floor_decode(&mut rdr, ident, mapping,
		&setup.codebooks, &setup.floors));

	// Now calculate the no_residue vector
	let mut no_residue = TinyVec::<[bool; 32]>::new();
	for fl in &decoded_floor_infos {
		no_residue.push(fl.is_unused());
	}
	// and also propagate
	for (&mag, &angle) in
			mapping.mapping_magnitudes.iter().zip(mapping.mapping_angles.iter()) {
		if ! (no_residue[mag as usize] && no_residue[angle as usize]) {
			no_residue[mag as usize] = false;
			no_residue[angle as usize] = false;
		}
	}

	// Residue decode.
	let mut residue_vectors = vec![vec![]; mapping.mapping_mux.len()];
	// Helper variable
	let resid_vec_len = (n / 2) as usize;
	for (i, &residue_number) in mapping.mapping_submap_residues.iter().enumerate() {
		let mut do_not_decode_flag = TinyVec::<[bool; 32]>::new();
		for (j, &mapping_mux_j) in mapping.mapping_mux.iter().enumerate() {
			if mapping_mux_j as usize == i {
				do_not_decode_flag.push(no_residue[j]);
			}
		}
		let cur_residue = &setup.residues[residue_number as usize];
		let vectors = match residue_packet_decode(&mut rdr, n,
				&do_not_decode_flag, cur_residue, &setup.codebooks) {
			Ok(v) => v,
			Err(_) => return Err(AudioReadError::AudioBadFormat),
		};
		// The vectors Vec<f32> now contains the do_not_decode_flag.len()
		// many decoded residue vectors, each vector occupying n/2 scalars.
		let mut ch = 0;
		for (j, &mapping_mux_j) in mapping.mapping_mux.iter().enumerate() {
			if mapping_mux_j as usize == i {
				// TODO get rid of this copy somehow...
				let vec_at_ch = &vectors[resid_vec_len * ch .. resid_vec_len * (ch + 1)];
				residue_vectors[j].clear();
				residue_vectors[j].extend_from_slice(vec_at_ch);
				ch += 1;
			}
		}
	}

	record_residue_pre_inverse!(residue_vectors);

	// Inverse coupling
	for (&mag, &angle) in
			mapping.mapping_magnitudes.iter().rev().zip(mapping.mapping_angles.iter().rev()) {
		let (mag_vector, angle_vector) = dual_mut_idx(&mut residue_vectors,
			mag as usize, angle as usize);
		for (m, a) in mag_vector.iter_mut().zip(angle_vector.iter_mut()) {
			// https://github.com/rust-lang/rfcs/issues/372
			// grumble grumble...
			let (new_m, new_a) = inverse_couple(*m, *a);
			*m = new_m;
			*a = new_a;
		}
	}

	record_residue_post_inverse!(residue_vectors);

	// Dot product
	let mut audio_spectri = Vec::with_capacity(ident.audio_channels as usize);
	for (residue_vector, chan_decoded_floor) in
			residue_vectors.iter().zip(decoded_floor_infos.iter()) {
		let mut floor_decoded :Vec<f32> = match chan_decoded_floor {
			&DecodedFloor::TypeZero(ref coefficients, amplitude, ref fl) => {
				floor_zero_compute_curve(coefficients, amplitude,
					fl, mode.mode_blockflag, n / 2)
			},
			&DecodedFloor::TypeOne(ref floor_y, ref fl) => {
				let (floor1_final_y, floor1_step2_flag) =
					floor_one_curve_compute_amplitude(floor_y, fl);
				floor_one_curve_synthesis(floor1_final_y,
					floor1_step2_flag, fl, n / 2)
			},
			&DecodedFloor::Unused => {
				// Generate zero'd floor of length n/2
				vec![0.; (n / 2) as usize]
			},
		};

		// The only legal length is n/2.
		// The implementation should ensure this,
		// but its good for debugging to have this
		// confirmed.
		debug_assert_eq!(residue_vector.len(), (n / 2) as usize);
		debug_assert_eq!(floor_decoded.len(), (n / 2) as usize);

		// Now do the multiplication
		for (fl_sc, r_sc) in floor_decoded.iter_mut().zip(residue_vector.iter()) {
			*fl_sc *= *r_sc;
		}
		audio_spectri.push(floor_decoded);
	}

	record_pre_mdct!(audio_spectri);

	// Inverse MDCT
	for ref mut spectrum in audio_spectri.iter_mut() {
		let size = (n / 2) as usize;
		let ext = iter::repeat(0.).take(size);
		spectrum.extend(ext);
		let cached_bd = &ident.cached_bs_derived[mode.mode_blockflag as usize];
		//imdct::inverse_mdct_naive(cached_bd, &mut spectrum[..]);
		imdct::inverse_mdct(cached_bd, &mut spectrum[..], bs);
		//inverse_mdct_slow(&mut spectrum[..]);
	}

	record_post_mdct!(audio_spectri);

	// Compute windowing info for left window
	let window_center = n >> 1;
	let (left_win_start, left_win_end, left_n, left_n_use_bs1) =
		if previous_next_window_flag.map_or(true, |(prev_win_flag, _)| prev_win_flag) {
			(0, window_center, n >> 1, mode.mode_blockflag)
		} else {
			let bs_0_exp = 1 << ident.blocksize_0;
			((n - bs_0_exp) >> 2, (n + bs_0_exp) >> 2, bs_0_exp >> 1, false)
		};

	// Compute windowing info for right window
	let (right_win_start, right_win_end) =
		if previous_next_window_flag.map_or(true, |(_, next_win_flag)| next_win_flag) {
			(window_center, n)
		} else {
			let bs_0_exp = 1 << ident.blocksize_0;
			((n * 3 - bs_0_exp) >> 2, (n * 3 + bs_0_exp) >> 2)
		};

	/*println!("n={} prev_win_flag={:?} left_win_(start={}, end={}, n={}) right_win(start={}, end={})",
		n, previous_next_window_flag, left_win_start, left_win_end, left_n,
		right_win_start, right_win_end); // */

	// Overlap add and store last half
	// in PreviousWindowRight
	// Only add if prev has elements.
	let mut future_prev_halves = Vec::with_capacity(ident.audio_channels as usize);
	if let Some(prev_data) = pwr.data.take() {
		// TODO maybe check if prev_n matches blocksize_0 or blocksize_1,
		// and the channel number. Panic if no match of either.
		assert_eq!(audio_spectri.len(), prev_data.len());

		let win_slope = &ident.cached_bs_derived[left_n_use_bs1 as usize].window_slope;

		for (prev_chan, chan) in prev_data.into_iter().zip(audio_spectri.iter_mut()) {
			let plen = prev_chan.len();
			let left_win_start = left_win_start as usize;
			let right_win_start = right_win_start as usize;
			let right_win_end = right_win_end as usize;

			// Then do the actual overlap_add
			// Set up iterators for all the variables
			let range = {
				let start = left_win_start;
				let end = left_win_start + plen;
				start..end
			};

			let prev = prev_chan[0..plen].iter();

			let (lhs, rhs) = {
				if win_slope.len() < plen {
					// According to fuzzing, code can trigger this case,
					// so let's error gracefully instead of panicing.
					try!(Err(AudioReadError::AudioBadFormat));
				}
				let win_slope = &win_slope[0..plen];
				(win_slope.iter(), win_slope.iter().rev())
			};

			for (((v, lhs), prev), rhs) in chan[range].iter_mut().zip(lhs).zip(prev).zip(rhs) {
				*v = (*v * lhs) + (prev * rhs);
			}

  			// and populate the future previous half
			let future_prev_half = chan[right_win_start..right_win_end].into();

			future_prev_halves.push(future_prev_half);

			// Remove everything left of the left window start,
			// by moving the the stuff right to it to the left.
			if left_win_start > 0 {
				for i in 0 .. right_win_start - left_win_start {
					chan[i] = chan[i + left_win_start];
				}
			}

			// Now the last step: truncate the decoded packet
			// to cut off the right part.
			chan.truncate(right_win_start - left_win_start);
			// TODO stb_vorbis doesn't use right_win_start
			// in the calculation above but sth like
			// if len < right_win_start { len } else { right_win_start }
		}
	} else {
		for chan in audio_spectri.iter_mut() {
			let mut future_prev_half = Vec::with_capacity(
				(right_win_end - right_win_start) as usize);
			for i in right_win_start as usize .. right_win_end as usize {
				future_prev_half.push(chan[i]);
			}
			future_prev_halves.push(future_prev_half);
			// If there is no previous window right, we have to discard
			// the whole packet.
			chan.truncate(0);
		}
	}

	pwr.data = Some(future_prev_halves);

	// Generate final integer samples
	let final_i16_samples = S::from_floats(audio_spectri);

	Ok(final_i16_samples)
}

/**
Main audio packet decoding function

Pass your info to this function to get your raw packet data decoded.

Panics if the passed PreviousWindowRight struct doesn't match the info
from the ident header.
*/
pub fn read_audio_packet(ident :&IdentHeader, setup :&SetupHeader, packet :&[u8], pwr :&mut PreviousWindowRight)
		-> Result<Vec<Vec<i16>>, AudioReadError> {
	read_audio_packet_generic(ident, setup, packet, pwr)
}
