// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

// This file is a very close translation of the
// implementation of the algorithm from stb_vorbis.

use ::header_cached::CachedBlocksizeDerived;

fn imdct_step3_iter0_loop(n :usize, e :&mut[f32], i_off :usize, k_off :isize, a :&[f32]) {
	let mut a_offs = 0;
	let mut i_offs = i_off;
	let mut k_offs = i_off as isize + k_off;

	macro_rules! ee0 {
		(-$x:expr) => {e[i_offs - ($x as usize)]};
		($x:expr) => {e[i_offs + ($x as usize)]}
	}

	macro_rules! ee2 {
		(-$x:expr) => {e[(k_offs - $x) as usize]};
		($x:expr) => {e[(k_offs + $x) as usize]}
	}

	macro_rules! aa {
		($x:expr) => {a[a_offs + ($x as usize)]}
	}

	assert_eq!((n & 3), 0);

	for _ in 0 .. n >> 2 {
		let mut k00_20 = ee0![ 0] - ee2![ 0];
		let mut k01_21 = ee0![-1] - ee2![-1];
		ee0![ 0] += ee2![ 0];
		ee0![-1] += ee2![-1];
		ee2![ 0] = k00_20 * aa![0] - k01_21 * aa![1];
		ee2![-1] = k01_21 * aa![0] + k00_20 * aa![1];
		a_offs += 8;

		k00_20  = ee0![-2] - ee2![-2];
		k01_21  = ee0![-3] - ee2![-3];
		ee0![-2] += ee2![-2];
		ee0![-3] += ee2![-3];
		ee2![-2] = k00_20 * aa![0] - k01_21 * aa![1];
		ee2![-3] = k01_21 * aa![0] + k00_20 * aa![1];
		a_offs += 8;

		k00_20  = ee0![-4] - ee2![-4];
		k01_21  = ee0![-5] - ee2![-5];
		ee0![-4] += ee2![-4];
		ee0![-5] += ee2![-5];
		ee2![-4] = k00_20 * aa![0] - k01_21 * aa![1];
		ee2![-5] = k01_21 * aa![0] + k00_20 * aa![1];
		a_offs += 8;

		k00_20  = ee0![-6] - ee2![-6];
		k01_21  = ee0![-7] - ee2![-7];
		ee0![-6] += ee2![-6];
		ee0![-7] += ee2![-7];
		ee2![-6] = k00_20 * aa![0] - k01_21 * aa![1];
		ee2![-7] = k01_21 * aa![0] + k00_20 * aa![1];

		a_offs += 8;
		i_offs -= 8;
		k_offs -= 8;
	}
}

fn imdct_step3_inner_r_loop(lim :usize, e :&mut [f32],
		d0 :usize, k_off :isize, a :&[f32], k1 :usize) {
	let mut a_offs = 0;
	let mut d0_offs = d0;
	let mut k_offs = d0 as isize + k_off;

	macro_rules! e0 {
		(-$x:expr) => {e[d0_offs - ($x as usize)]};
		($x:expr) => {e[d0_offs + ($x as usize)]}
	}

	macro_rules! e2 {
		(-$x:expr) => {e[(k_offs - $x) as usize]};
		($x:expr) => {e[(k_offs + $x) as usize]}
	}

	macro_rules! aa {
		($x:expr) => {a[a_offs + ($x as usize)]}
	}

	for _ in 0 .. lim >> 2 {
		let mut k00_20 = e0![-0] - e2![-0];
		let mut k01_21 = e0![-1] - e2![-1];
		e0![-0] += e2![-0];
		e0![-1] += e2![-1];
		e2![-0] = (k00_20) * aa![0] - (k01_21) * aa![1];
		e2![-1] = (k01_21) * aa![0] + (k00_20) * aa![1];

		a_offs += k1;

		k00_20 = e0![-2] - e2![-2];
		k01_21 = e0![-3] - e2![-3];
		e0![-2] += e2![-2];
		e0![-3] += e2![-3];
		e2![-2] = (k00_20) * aa![0] - (k01_21) * aa![1];
		e2![-3] = (k01_21) * aa![0] + (k00_20) * aa![1];

		a_offs += k1;

		k00_20 = e0![-4] - e2![-4];
		k01_21 = e0![-5] - e2![-5];
		e0![-4] += e2![-4];
		e0![-5] += e2![-5];
		e2![-4] = (k00_20) * aa![0] - (k01_21) * aa![1];
		e2![-5] = (k01_21) * aa![0] + (k00_20) * aa![1];

		a_offs += k1;

		k00_20 = e0![-6] - e2![-6];
		k01_21 = e0![-7] - e2![-7];
		e0![-6] += e2![-6];
		e0![-7] += e2![-7];
		e2![-6] = (k00_20) * aa![0] - (k01_21) * aa![1];
		e2![-7] = (k01_21) * aa![0] + (k00_20) * aa![1];

		d0_offs -= 8;
		k_offs -= 8;

		a_offs += k1;
	}
}

fn imdct_step3_inner_s_loop(n :usize, e :&mut [f32], i_off :usize, k_off :isize,
		a :&[f32], a_off :usize, k0 :usize) {
	let a0 = a[0];
	let a1 = a[0+1];
	let a2 = a[0+a_off];
	let a3 = a[0+a_off+1];
	let a4 = a[0+a_off*2+0];
	let a5 = a[0+a_off*2+1];
	let a6 = a[0+a_off*3+0];
	let a7 = a[0+a_off*3+1];

	let mut i_offs = i_off;
	let mut k_offs = (i_off as isize + k_off) as usize;

	macro_rules! ee0 {
		(-$x:expr) => {e[i_offs - ($x as usize)]};
		($x:expr) => {e[i_offs + ($x as usize)]}
	}

	macro_rules! ee2 {
		(-$x:expr) => {e[k_offs - ($x as usize)]};
		($x:expr) => {e[k_offs + ($x as usize)]}
	}

	let mut i = 0;
	loop {
		let mut k00 = ee0![ 0] - ee2![ 0];
		let mut k11 = ee0![-1] - ee2![-1];
		ee0![ 0] =  ee0![ 0] + ee2![ 0];
		ee0![-1] =  ee0![-1] + ee2![-1];
		ee2![ 0] = (k00) * a0 - (k11) * a1;
		ee2![-1] = (k11) * a0 + (k00) * a1;

		k00      = ee0![-2] - ee2![-2];
		k11      = ee0![-3] - ee2![-3];
		ee0![-2] =  ee0![-2] + ee2![-2];
		ee0![-3] =  ee0![-3] + ee2![-3];
		ee2![-2] = (k00) * a2 - (k11) * a3;
		ee2![-3] = (k11) * a2 + (k00) * a3;

		k00      = ee0![-4] - ee2![-4];
		k11      = ee0![-5] - ee2![-5];
		ee0![-4] =  ee0![-4] + ee2![-4];
		ee0![-5] =  ee0![-5] + ee2![-5];
		ee2![-4] = (k00) * a4 - (k11) * a5;
		ee2![-5] = (k11) * a4 + (k00) * a5;

		k00      = ee0![-6] - ee2![-6];
		k11      = ee0![-7] - ee2![-7];
		ee0![-6] =  ee0![-6] + ee2![-6];
		ee0![-7] =  ee0![-7] + ee2![-7];
		ee2![-6] = (k00) * a6 - (k11) * a7;
		ee2![-7] = (k11) * a6 + (k00) * a7;

		i += 1;
		// we have this check instead of a for loop
		// over an iterator because otherwise we
		// overflow.
		if i >= n {
			break;
		}
		i_offs -= k0;
		k_offs -= k0;
	}
}

#[inline]
fn iter_54(zm7 :&mut [f32]) {
	// difference from stb_vorbis implementation:
	// zm7 points to z minus 7
	// (Rust disallows negative indices)

	let k00  = zm7[7] - zm7[3];
	let y0   = zm7[7] + zm7[3];
	let y2   = zm7[5] + zm7[1];
	let k22  = zm7[5] - zm7[1];

	zm7[7] = y0 + y2;      // z0 + z4 + z2 + z6
	zm7[5] = y0 - y2;      // z0 + z4 - z2 - z6

	// done with y0,y2

	let k33  = zm7[4] - zm7[0];

	zm7[3] = k00 + k33;    // z0 - z4 + z3 - z7
	zm7[1] = k00 - k33;    // z0 - z4 - z3 + z7

	// done with k33

	let k11  = zm7[6] - zm7[2];
	let y1   = zm7[6] + zm7[2];
	let y3   = zm7[4] + zm7[0];

	zm7[6] = y1 + y3;      // z1 + z5 + z3 + z7
	zm7[4] = y1 - y3;      // z1 + z5 - z3 - z7
	zm7[2] = k11 - k22;    // z1 - z5 + z2 - z6
	zm7[0] = k11 + k22;    // z1 - z5 - z2 + z6
}

fn imdct_step3_inner_s_loop_ld654(n :usize, e :&mut [f32], i_off :usize,
	a :&[f32], base_n :usize)
{
	let a_off = base_n >> 3;
	let a2 = a[a_off];

	let mut z_offs = i_off;

	let basep16 = i_off - 16 * (n - 1 as usize);

	macro_rules! z {
		(-$x:expr) => {e[z_offs - ($x as usize)]}
	}

	loop {
		let mut k00 = z![-0] - z![-8];
		let mut k11 = z![-1] - z![-9];
		z![-0] = z![-0] + z![-8];
		z![-1] = z![-1] + z![-9];
		z![-8] =  k00;
		z![-9] =  k11;

		k00     = z![ -2] - z![-10];
		k11     = z![ -3] - z![-11];
		z![ -2] = z![ -2] + z![-10];
		z![ -3] = z![ -3] + z![-11];
		z![-10] = (k00+k11) * a2;
		z![-11] = (k11-k00) * a2;

		k00     = z![-12] - z![ -4];  // reverse to avoid a unary negation
		k11     = z![ -5] - z![-13];
		z![ -4] = z![ -4] + z![-12];
		z![ -5] = z![ -5] + z![-13];
		z![-12] = k11;
		z![-13] = k00;

		k00     = z![-14] - z![ -6];  // reverse to avoid a unary negation
		k11     = z![ -7] - z![-15];
		z![ -6] = z![ -6] + z![-14];
		z![ -7] = z![ -7] + z![-15];
		z![-14] = (k00+k11) * a2;
		z![-15] = (k00-k11) * a2;

		iter_54(e.split_at_mut(z_offs - 7).1);
		iter_54(e.split_at_mut(z_offs - 7 - 8).1);
		// We need to compare with basep16 here
		// in order to prevent a possible overflow
		// in calculation of base, and in calculation
		// of z_offs.
		if z_offs <= basep16 {
			break;
		}
		z_offs -= 16;
	}
}

#[allow(dead_code)]
pub fn inverse_mdct(cached_bd :&CachedBlocksizeDerived, buffer :&mut [f32], bs :u8) {
	let n = buffer.len();
	// Pre-condition.
	assert_eq!(n, 1 << bs);

	let n2 = n >> 1;
	let n4 = n >> 2;
	let n8 = n >> 3;

	// TODO later on we might want to do Vec::with_capacity here,
	// and use buf2.push everywhere...
	let mut buf2 :Vec<f32> = vec![0.0; n2];

	let ctf = &cached_bd.twiddle_factors;
	let a :&[f32] = &ctf.a;
	let b :&[f32] = &ctf.b;
	let c :&[f32] = &ctf.c;

	macro_rules! break_if_sub_overflows {
		($i:ident, $x:expr) => {
			$i = match $i.checked_sub($x) {
				Some(v) => v,
				None => break,
			};
		}
	}

	// IMDCT algorithm from "The use of multirate filter banks for coding of high quality digital audio"
	// See notes about bugs in that paper in less-optimal implementation 'inverse_mdct_old' in stb_vorbis original.

	// kernel from paper


	// merged:
	//   copy and reflect spectral data
	//   step 0

	// note that it turns out that the items added together during
	// this step are, in fact, being added to themselves (as reflected
	// by step 0). inexplicable inefficiency! this became obvious
	// once I combined the passes.

	// so there's a missing 'times 2' here (for adding X to itself).
	// this propogates through linearly to the end, where the numbers
	// are 1/2 too small, and need to be compensated for.

	{
		let mut a_offs = 0;
		let mut d_offs = n2 - 2;
		let mut e_offs = 0;
		let e_stop = n2;

		macro_rules! d {
			($x:expr) => {buf2[d_offs + ($x as usize)]}
		}
		macro_rules! aa {
			($x:expr) => {a[a_offs + ($x as usize)]}
		}
		macro_rules! e {
			($x:expr) => {buffer[e_offs + ($x as usize)]}
		}

		// TODO replace the while with a for once step_by on iterators
		// is stabilized
		while e_offs != e_stop {
			d![1] = e![0] * aa![0] - e![2]*aa![1];
			d![0] = e![0] * aa![1] + e![2]*aa![0];
			d_offs -= 2;
			a_offs += 2;
			e_offs += 4;
		}

		e_offs = n2 - 3;
		loop {
			d![1] = -e![2] * aa![0] - -e![0]*aa![1];
			d![0] = -e![2] * aa![1] + -e![0]*aa![0];
			break_if_sub_overflows!(d_offs, 2);
			a_offs += 2;
			e_offs -= 4;
		}
	}


	{
		// now we use symbolic names for these, so that we can
		// possibly swap their meaning as we change which operations
		// are in place

		let u = &mut *buffer;
		let v = &mut *buf2;

		// step 2    (paper output is w, now u)
		// this could be in place, but the data ends up in the wrong
		// place... _somebody_'s got to swap it, so this is nominated
		{
			let mut a_offs = n2 - 8;
			let mut d0_offs = n4;
			let mut d1_offs = 0;
			let mut e0_offs = n4;
			let mut e1_offs = 0;

			macro_rules! aa {
				($x:expr) => {a[a_offs + ($x as usize)]}
			}
			macro_rules! d0 {
				($x:expr) => {u[d0_offs + ($x as usize)]}
			}
			macro_rules! d1 {
				($x:expr) => {u[d1_offs + ($x as usize)]}
			}
			macro_rules! e0 {
				($x:expr) => {v[e0_offs + ($x as usize)]}
			}
			macro_rules! e1 {
				($x:expr) => {v[e1_offs + ($x as usize)]}
			}

			loop {
				let mut v41_21 = e0![1] - e1![1];
				let mut v40_20 = e0![0] - e1![0];
				d0![1]  = e0![1] + e1![1];
				d0![0]  = e0![0] + e1![0];
				d1![1]  = v41_21*aa![4] - v40_20*aa![5];
				d1![0]  = v40_20*aa![4] + v41_21*aa![5];

				v41_21 = e0![3] - e1![3];
				v40_20 = e0![2] - e1![2];
				d0![3]  = e0![3] + e1![3];
				d0![2]  = e0![2] + e1![2];
				d1![3]  = v41_21*aa![0] - v40_20*aa![1];
				d1![2]  = v40_20*aa![0] + v41_21*aa![1];

				break_if_sub_overflows!(a_offs, 8);

				d0_offs += 4;
				d1_offs += 4;
				e0_offs += 4;
				e1_offs += 4;
			}
		}


		// step 3

		let ld = bs as usize;

		// optimized step 3:

		// the original step3 loop can be nested r inside s or s inside r;
		// it's written originally as s inside r, but this is dumb when r
		// iterates many times, and s few. So I have two copies of it and
		// switch between them halfway.

		// this is iteration 0 of step 3
		imdct_step3_iter0_loop(n >> 4, u, n2-1-n4*0, -(n as isize >> 3), a);
		imdct_step3_iter0_loop(n >> 4, u, n2-1-n4*1, -(n as isize >> 3), a);

		// this is iteration 1 of step 3
		imdct_step3_inner_r_loop(n >> 5, u, n2-1 - n8*0, -(n as isize >> 4), a, 16);
		imdct_step3_inner_r_loop(n >> 5, u, n2-1 - n8*1, -(n as isize >> 4), a, 16);
		imdct_step3_inner_r_loop(n >> 5, u, n2-1 - n8*2, -(n as isize >> 4), a, 16);
		imdct_step3_inner_r_loop(n >> 5, u, n2-1 - n8*3, -(n as isize >> 4), a, 16);

		for l in 2 .. (ld - 3) >> 1 {
			let k0 = n >> (l + 2);
			let k0_2 = k0 as isize >> 1;
			let lim = 1 << (l+1);
			for i in 0 .. lim {
				imdct_step3_inner_r_loop(n >> (l + 4),
					u, n2-1 - k0*i, -k0_2, a, 1 << (l+3));
			}
		}
		for l in (ld - 3) >> 1 .. ld - 6 {
			let k0 = n >> (l + 2);
			let k1 = 1 << (l + 3);
			let k0_2 = k0 as isize >> 1;
			let rlim = n >> (l + 6);
			let lim = 1 << (l + 1);
			let mut i_off = n2 - 1;
			let mut a_off = 0;
			for _ in 0 .. rlim {
				let a0 = a.split_at(a_off).1;
				imdct_step3_inner_s_loop(lim, u, i_off, -k0_2, a0, k1, k0);
				a_off += k1 * 4;
				i_off -= 8;
			}
		}

		// iterations with count:
		//   ld-6,-5,-4 all interleaved together
		//       the big win comes from getting rid of needless flops
		//         due to the constants on pass 5 & 4 being all 1 and 0;
		//       combining them to be simultaneous to improve cache made little difference
		imdct_step3_inner_s_loop_ld654(n >> 5, u, n2 - 1, a, n);

		// output is u

		// step 4, 5, and 6
		// cannot be in-place because of step 5
		{
			let bitrev_vec = &cached_bd.bitrev;
			// weirdly, I'd have thought reading sequentially and writing
			// erratically would have been better than vice-versa, but in
			// fact that's not what my testing showed. (That is, with
			// j = bitreverse(i), do you read i and write j, or read j and write i.)

			let mut d0_offs = n4 - 4;
			let mut d1_offs = n2 - 4;
			let mut bitrev_offs = 0;

			macro_rules! d0 {
				($x:expr) => {v[d0_offs + ($x as usize)]}
			}
			macro_rules! d1 {
				($x:expr) => {v[d1_offs + ($x as usize)]}
			}
			macro_rules! bitrev {
				($x:expr) => {bitrev_vec[bitrev_offs + ($x as usize)]}
			}

			loop {
				let mut k4 = bitrev![0] as usize;
				d1![3] = u[k4 + 0];
				d1![2] = u[k4 + 1];
				d0![3] = u[k4 + 2];
				d0![2] = u[k4 + 3];

				k4 = bitrev![1] as usize;
				d1![1] = u[k4 + 0];
				d1![0] = u[k4 + 1];
				d0![1] = u[k4 + 2];
				d0![0] = u[k4 + 3];

				break_if_sub_overflows!(d0_offs, 4);
				d1_offs -= 4;
				bitrev_offs += 2;
			}
		}
		// (paper output is u, now v)

		// step 7   (paper output is v, now v)
		// this is now in place
		{
			let mut c_offs = 0;
			let mut d_offs = 0;
			let mut e_offs = n2 - 4;

			macro_rules! cc {
				($x:expr) => {c[c_offs + ($x as usize)]}
			}
			macro_rules! d {
				($x:expr) => {v[d_offs + ($x as usize)]}
			}
			macro_rules! e {
				($x:expr) => {v[e_offs + ($x as usize)]}
			}
			while d_offs < e_offs {
				let mut a02 = d![0] - e![2];
				let mut a11 = d![1] + e![3];

				let mut b0 = cc![1]*a02 + cc![0]*a11;
				let mut b1 = cc![1]*a11 - cc![0]*a02;

				let mut b2 = d![0] + e![ 2];
				let mut b3 = d![1] - e![ 3];

				d![0] = b2 + b0;
				d![1] = b3 + b1;
				e![2] = b2 - b0;
				e![3] = b1 - b3;

				a02 = d![2] - e![0];
				a11 = d![3] + e![1];

				b0 = cc![3]*a02 + cc![2]*a11;
				b1 = cc![3]*a11 - cc![2]*a02;

				b2 = d![2] + e![ 0];
				b3 = d![3] - e![ 1];

				d![2] = b2 + b0;
				d![3] = b3 + b1;
				e![0] = b2 - b0;
				e![1] = b1 - b3;

				c_offs += 4;
				d_offs += 4;
				e_offs -= 4;
			}
		}
	}

	// step 8+decode   (paper output is X, now buffer)
	// this generates pairs of data a la 8 and pushes them directly through
	// the decode kernel (pushing rather than pulling) to avoid having
	// to make another pass later

	// this cannot POSSIBLY be in place, so we refer to the buffers directly
	{
		let mut d0_offs = 0;
		let mut d1_offs = n2 - 4;
		let mut d2_offs = n2;
		let mut d3_offs = n - 4;

		let mut b_offs = n2 - 8;
		let mut e_offs = n2 - 8;

		macro_rules! d0 {
			($x:expr) => {buffer[d0_offs + ($x as usize)]}
		}
		macro_rules! d1 {
			($x:expr) => {buffer[d1_offs + ($x as usize)]}
		}
		macro_rules! d2 {
			($x:expr) => {buffer[d2_offs + ($x as usize)]}
		}
		macro_rules! d3 {
			($x:expr) => {buffer[d3_offs + ($x as usize)]}
		}

		macro_rules! b {
			($x:expr) => {b[b_offs + ($x as usize)]}
		}
		macro_rules! e {
			($x:expr) => {buf2[e_offs + ($x as usize)]}
		}

		loop {
			let mut p3 =  e![6]*b![7] - e![7]*b![6];
			let mut p2 = -e![6]*b![6] - e![7]*b![7];

			d0![0] =   p3;
			d1![3] = - p3;
			d2![0] =   p2;
			d3![3] =   p2;

			let mut p1 =  e![4]*b![5] - e![5]*b![4];
			let mut p0 = -e![4]*b![4] - e![5]*b![5];

			d0![1] =   p1;
			d1![2] = - p1;
			d2![1] =   p0;
			d3![2] =   p0;

			p3 =  e![2]*b![3] - e![3]*b![2];
			p2 = -e![2]*b![2] - e![3]*b![3];

			d0![2] =   p3;
			d1![1] = - p3;
			d2![2] =   p2;
			d3![1] =   p2;

			p1 =  e![0]*b![1] - e![1]*b![0];
			p0 = -e![0]*b![0] - e![1]*b![1];

			d0![3] =   p1;
			d1![0] = - p1;
			d2![3] =   p0;
			d3![0] =   p0;

			break_if_sub_overflows!(e_offs, 8);
			b_offs -= 8;
			d0_offs += 4;
			d2_offs += 4;
			d1_offs -= 4;
			d3_offs -= 4;
		}
	}
}

#[allow(dead_code)]
pub fn inverse_mdct_naive(cached_bd :&CachedBlocksizeDerived, buffer :&mut[f32]) {
	let n = buffer.len();
	let n2 = n >> 1;
	let n4 = n >> 2;
	let n8 = n >> 3;
	let n3_4 = n - n4;

	let mut u = [0.0; 1 << 13];
	let mut xa = [0.0; 1 << 13];
	let mut v = [0.0; 1 << 13];
	let mut w = [0.0; 1 << 13];

	// retrieve the cached twiddle factors
	let ctf = &cached_bd.twiddle_factors;
	let a :&[f32] = &ctf.a;
	let b :&[f32] = &ctf.b;
	let c :&[f32] = &ctf.c;

	// IMDCT algorithm from "The use of multirate filter banks for coding of high quality digital audio"
	// Note there are bugs in that pseudocode, presumably due to them attempting
	// to rename the arrays nicely rather than representing the way their actual
	// implementation bounces buffers back and forth. As a result, even in the
	// "some formulars corrected" version, a direct implementation fails. These
	// are noted below as "paper bug".

	// copy and reflect spectral data
	for k in 0 .. n2 {
		u[k] = buffer[k];
	}
	for k in n2 .. n {
		u[k] = -buffer[n - k - 1];
	}

	let mut k2 = 0;
	let mut k4 = 0;

	// kernel from paper
	// step 1
	while k2 < n2 { // n4 iterations
		v[n-k4-1] = (u[k4] - u[n-k4-1]) * a[k2]   - (u[k4+2] - u[n-k4-3])*a[k2+1];
		v[n-k4-3] = (u[k4] - u[n-k4-1]) * a[k2+1] + (u[k4+2] - u[n-k4-3])*a[k2];
		k2 += 2;
		k4 += 4;
	}
	// step 2
	k4 = 0;
	while k4 < n2 { // n8 iterations
		w[n2+3+k4] = v[n2+3+k4] + v[k4+3];
		w[n2+1+k4] = v[n2+1+k4] + v[k4+1];
		w[k4+3]    = (v[n2+3+k4] - v[k4+3])*a[n2-4-k4] - (v[n2+1+k4]-v[k4+1])*a[n2-3-k4];
		w[k4+1]    = (v[n2+1+k4] - v[k4+1])*a[n2-4-k4] + (v[n2+3+k4]-v[k4+3])*a[n2-3-k4];
		k4 += 4;
	}

	// step 3
	let ld :usize = (::ilog(n as u64) - 1) as usize;
	for l in 0 .. ld - 3 {
		let k0 = n >> (l+2);
		let k1 = 1 << (l+3);
		let rlim = n >> (l+4);
		let slim = 1 << (l+1);
		let mut r4 = 0;
		for r in 0 .. rlim {
			let mut s2 = 0;
			for _ in 0 .. slim {
				u[n-1-k0*s2-r4] = w[n-1-k0*s2-r4] + w[n-1-k0*(s2+1)-r4];
				u[n-3-k0*s2-r4] = w[n-3-k0*s2-r4] + w[n-3-k0*(s2+1)-r4];
				u[n-1-k0*(s2+1)-r4] = (w[n-1-k0*s2-r4] - w[n-1-k0*(s2+1)-r4]) * a[r*k1]
					- (w[n-3-k0*s2-r4] - w[n-3-k0*(s2+1)-r4]) * a[r*k1+1];
				u[n-3-k0*(s2+1)-r4] = (w[n-3-k0*s2-r4] - w[n-3-k0*(s2+1)-r4]) * a[r*k1]
					+ (w[n-1-k0*s2-r4] - w[n-1-k0*(s2+1)-r4]) * a[r*k1+1];
				s2 += 2;
			}
			r4 += 4;
		}
		if l+1 < ld-3 {
			// paper bug: ping-ponging of u&w here is omitted
			w.copy_from_slice(&u);
		}
	}

	// step 4
	for i in 0 .. n8 {
		let j = (::bit_reverse(i as u32) >> (32-ld+3)) as usize;
		assert!(j < n8);
		if i == j {
			// paper bug: original code probably swapped in place; if copying,
			//            need to directly copy in this case
			let ii = i << 3;
			v[ii+1] = u[ii+1];
			v[ii+3] = u[ii+3];
			v[ii+5] = u[ii+5];
			v[ii+7] = u[ii+7];
		} else if i < j {
			let ii = i << 3;
			let j8 = j << 3;
			v[j8+1] = u[ii+1];
			v[ii+1] = u[j8 + 1];
			v[j8+3] = u[ii+3];
			v[ii+3] = u[j8 + 3];
			v[j8+5] = u[ii+5];
			v[ii+5] = u[j8 + 5];
			v[j8+7] = u[ii+7];
			v[ii+7] = u[j8 + 7];
		}
	}

	// step 5
	for k in 0 .. n2 {
		w[k] = v[k*2+1];
	}
	// step 6
	let mut k2 = 0;
	let mut k4 = 0;
	while k2 < n4 { // n8 iterations
		u[n-1-k2] = w[k4];
		u[n-2-k2] = w[k4+1];
		u[n3_4 - 1 - k2] = w[k4+2];
		u[n3_4 - 2 - k2] = w[k4+3];
		k2 += 2;
		k4 += 4;
	}
	// step 7
	k2 = 0;
	while k2 < n4 { // n8 iterations
		v[n2 + k2 ] = ( u[n2 + k2] + u[n-2-k2] + c[k2+1]*(u[n2+k2]-u[n-2-k2]) + c[k2]*(u[n2+k2+1]+u[n-2-k2+1]))/2.0;
		v[n-2 - k2] = ( u[n2 + k2] + u[n-2-k2] - c[k2+1]*(u[n2+k2]-u[n-2-k2]) - c[k2]*(u[n2+k2+1]+u[n-2-k2+1]))/2.0;
		v[n2+1+ k2] = ( u[n2+1+k2] - u[n-1-k2] + c[k2+1]*(u[n2+1+k2]+u[n-1-k2]) - c[k2]*(u[n2+k2]-u[n-2-k2]))/2.0;
		v[n-1 - k2] = (-u[n2+1+k2] + u[n-1-k2] + c[k2+1]*(u[n2+1+k2]+u[n-1-k2]) - c[k2]*(u[n2+k2]-u[n-2-k2]))/2.0;
		k2 += 2;
	}
	// step 8
	k2 = 0;
	for k in 0 .. n4 {
		xa[k]      = v[k2+n2]*b[k2  ] + v[k2+1+n2]*b[k2+1];
		xa[n2-1-k] = v[k2+n2]*b[k2+1] - v[k2+1+n2]*b[k2  ];
		k2 += 2;
	}

	// decode kernel to output

	for i in 0 .. n4 {
		buffer[i] = xa[i + n4];
	}
	for i in n4 .. n3_4 {
		buffer[i] = -xa[n3_4 - i - 1];
	}
	for i in n3_4 .. n {
		buffer[i] = -xa[i - n3_4];
	}
}

#[cfg(test)]
#[test]
fn test_imdct_naive() {
	use imdct_test::*;
	let mut arr_1 = imdct_prepare(&IMDCT_INPUT_TEST_ARR_1);
	let cbd = CachedBlocksizeDerived::from_blocksize(8);
	inverse_mdct_naive(&cbd, &mut arr_1);
	let mismatches = fuzzy_compare_array(
		&arr_1, &IMDCT_OUTPUT_TEST_ARR_1,
		0.00005, true);
	let mismatches_limit = 0;
	if mismatches > mismatches_limit {
		panic!("Numer of mismatches {} was larger than limit of {}",
			mismatches, mismatches_limit);
	}
}

#[cfg(test)]
#[test]
fn test_imdct() {
	use imdct_test::*;
	let mut arr_1 = imdct_prepare(&IMDCT_INPUT_TEST_ARR_1);
	let blocksize = 8;
	let cbd = CachedBlocksizeDerived::from_blocksize(blocksize);
	inverse_mdct(&cbd, &mut arr_1, blocksize);
	let mismatches = fuzzy_compare_array(
		&arr_1, &IMDCT_OUTPUT_TEST_ARR_1,
		0.00005, true);
	let mismatches_limit = 0;
	if mismatches > mismatches_limit {
		panic!("Numer of mismatches {} was larger than limit of {}",
			mismatches, mismatches_limit);
	}
}
