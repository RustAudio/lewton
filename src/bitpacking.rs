// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

/*!
Vorbis bitpacking layer

Functionality to read content from the bitpacking layer.

Implements vorbis spec, section 2.

The most important struct of this mod is the `BitpackCursor` struct.
It can be instantiated using `BitpackCursor::new()`.

Note that this implementation doesn't fully align with the spec in the regard that it assumes a byte is an octet.
This is no problem on most architectures.
This non-alignment to the spec is due to the fact that the rust language is highly leaned towards byte == u8,
and doesn't even have a builtin single byte type.
*/


use ::huffman_tree::VorbisHuffmanTree;

/// A Cursor on slices to read numbers and bitflags, bit aligned.
pub struct BitpackCursor <'a> {
    bit_cursor :u8,
    byte_cursor :usize,
    inner :&'a[u8],
}

macro_rules! sign_extend {
( $num:expr, $desttype:ident, $bit_cnt_large:expr, $bit_cnt_small:expr) => { {
    let n = $num;
    let res :$desttype = n as $desttype;
    let k :u8 = $bit_cnt_large - $bit_cnt_small;
    res << k >> k
} }
}

#[test]
fn test_sign_extend() {
    assert_eq!(sign_extend!(0b00,  i8,  8,  2),  0);
    assert_eq!(sign_extend!(0b01,  i8,  8,  2),  1);
    assert_eq!(sign_extend!(0b11,  i8,  8,  2), -1);
    assert_eq!(sign_extend!(0b111, i8,  8,  3), -1);
    assert_eq!(sign_extend!(0b101, i8,  8,  3), -3);
    assert_eq!(sign_extend!(0b01111110, i16, 16, 8),  126);
    assert_eq!(sign_extend!(0b10000010, i16, 16, 8), -126);
}

/// Returns `num` bits of 1 (but never more than 8).
fn mask_bits(num : u8) -> u8 {
    !((!0u8).wrapping_shl(num as u32)) | if (num >= 8) { 0xff } else { 0 }
}

// Same as mask_bits but different in a special case: for num % 8 == 0
// Make sure that 0 <= num <= 8.
fn bmask_bits(num : u8) -> u8 {
    (!0u8).wrapping_shr(8 - num as u32)
}

#[test]
fn test_mask_bits() {
    assert_eq!(mask_bits(0), 0b00000000);
    assert_eq!(mask_bits(1), 0b00000001);
    assert_eq!(mask_bits(2), 0b00000011);
    assert_eq!(mask_bits(3), 0b00000111);
    assert_eq!(mask_bits(4), 0b00001111);
    assert_eq!(mask_bits(5), 0b00011111);
    assert_eq!(mask_bits(6), 0b00111111);
    assert_eq!(mask_bits(7), 0b01111111);
    assert_eq!(mask_bits(8), 0b11111111);
}

#[test]
fn test_bmask_bits() {
    assert_eq!(bmask_bits(0), 0b11111111);
    assert_eq!(bmask_bits(1), 0b00000001);
    assert_eq!(bmask_bits(2), 0b00000011);
    assert_eq!(bmask_bits(3), 0b00000111);
    assert_eq!(bmask_bits(4), 0b00001111);
    assert_eq!(bmask_bits(5), 0b00011111);
    assert_eq!(bmask_bits(6), 0b00111111);
    assert_eq!(bmask_bits(7), 0b01111111);
    assert_eq!(bmask_bits(8), 0b11111111);
}

// The main macro to read bit aligned
// Note that `$octetnum` is the number of octets in $bitnum ($bitnum / 8 rounded down)
macro_rules! bpc_read_body {
( $rettype:ident, $bitnum:expr, $octetnum:expr, $selfarg:expr ) => { {
    let last_octet_partial :usize = ($bitnum as i8 - $octetnum as i8 * 8 > 0) as usize;
    let octetnum_rounded_up :usize = last_octet_partial + $octetnum;
    let bit_cursor_after = ($selfarg.bit_cursor + $bitnum) % 8;

    if ($selfarg.bit_cursor + $bitnum) as usize > 8 * octetnum_rounded_up {
        /*println!("Reading {} bits (octetnum={}, last_partial={}, total_touched={}+1)",
            $bitnum, $octetnum, last_octet_partial, $octetnum + last_octet_partial);
        println!("    byte_c={}; bit_c={}", $selfarg.byte_cursor, $selfarg.bit_cursor);// */
        /*print!("Reading {} bits (byte_c={}; bit_c={}) [] = {:?}", $bitnum,
            $selfarg.byte_cursor, $selfarg.bit_cursor,
            &$selfarg.inner[$selfarg.byte_cursor .. $selfarg.byte_cursor +
            1 + octetnum_rounded_up]);// */
        if $selfarg.byte_cursor + 1 + octetnum_rounded_up > $selfarg.inner.len() {
            //println!(" => Out of bounds :\\");
            return Err(());
        }
        let buf = &$selfarg.inner[$selfarg.byte_cursor
            .. $selfarg.byte_cursor + 1 + octetnum_rounded_up];
        let mut res :$rettype = buf[0] as $rettype;
        res >>= $selfarg.bit_cursor;
        let mut cur_bit_cursor = 8 - $selfarg.bit_cursor;
        for i in 1 .. octetnum_rounded_up {
            res |= (buf[i] as $rettype) << cur_bit_cursor;
            cur_bit_cursor += 8;
        }
        let last_bits = buf[octetnum_rounded_up] & mask_bits(bit_cursor_after);
        res |= (last_bits as $rettype) << cur_bit_cursor;
        $selfarg.byte_cursor += octetnum_rounded_up;
        $selfarg.bit_cursor = bit_cursor_after;
        //println!(" => {:?}", res);
        Ok(res)
    } else {
        /*println!("Reading {} bits (octetnum={}, last_partial={}, total_touched={})",
            $bitnum, $octetnum, last_octet_partial, $octetnum + last_octet_partial);
        println!("    byte_c={}; bit_c={}", $selfarg.byte_cursor, $selfarg.bit_cursor);// */
        /*print!("Reading {} bits (byte_c={}; bit_c={}) [] = {:?}", $bitnum,
            $selfarg.byte_cursor, $selfarg.bit_cursor,
            &$selfarg.inner[$selfarg.byte_cursor .. $selfarg.byte_cursor +
            octetnum_rounded_up]);// */
        if $selfarg.byte_cursor + octetnum_rounded_up > $selfarg.inner.len() {
            //println!(" => Out of bounds :\\");
            return Err(());
        }
        let buf = &$selfarg.inner[$selfarg.byte_cursor ..
            $selfarg.byte_cursor + octetnum_rounded_up];
        let mut res :$rettype = buf[0] as $rettype;
        res >>= $selfarg.bit_cursor;
        if $bitnum <= 8 {
            res &= mask_bits($bitnum) as $rettype;
        }
        let mut cur_bit_cursor = 8 - $selfarg.bit_cursor;
        for i in 1 .. octetnum_rounded_up - 1 {
            res |= (buf[i] as $rettype) << cur_bit_cursor;
            cur_bit_cursor += 8;
        }
        if $bitnum > 8 {
            let last_bits = buf[octetnum_rounded_up - 1] & bmask_bits(bit_cursor_after);
            res |= (last_bits as $rettype) << cur_bit_cursor;
        }
        $selfarg.byte_cursor += $octetnum;
        $selfarg.byte_cursor += ($selfarg.bit_cursor == 8 - ($bitnum % 8)) as usize;
        $selfarg.bit_cursor = bit_cursor_after;
        //println!(" => {:?}", res);
        Ok(res)
    }
} }
}

macro_rules! uk_reader {
( $fnname:ident, $rettype:ident, $bitnum:expr, $octetnum:expr) => {
    #[inline]
    pub fn $fnname(&mut self) -> Result<$rettype, ()> {
        bpc_read_body!($rettype, $bitnum, $octetnum, self)
    }
}
}

macro_rules! ik_reader {
( $fnname:ident, $rettype:ident, $bitnum_of_rettype:expr, $bitnum:expr, $octetnum:expr) => {
    #[inline]
    pub fn $fnname(&mut self) -> Result<$rettype, ()> {
        Ok(sign_extend!(try!(
            bpc_read_body!($rettype, $bitnum, $octetnum, self)),
            $rettype, $bitnum_of_rettype, $bitnum))
    }
}
}

macro_rules! ik_dynamic_reader {
( $fnname:ident, $rettype:ident, $bitnum_of_rettype:expr) => {
    #[inline]
    pub fn $fnname(&mut self, bit_num :u8) -> Result<$rettype, ()> {
        let octet_num :usize = (bit_num / 8) as usize;
        assert!(bit_num <= $bitnum_of_rettype);
        Ok(sign_extend!(try!(
            bpc_read_body!($rettype, bit_num, octet_num, self)),
            $rettype, $bitnum_of_rettype, bit_num))
    }
}
}

macro_rules! uk_dynamic_reader {
( $fnname:ident, $rettype:ident, $bit_num_max:expr) => {
    #[inline]
    pub fn $fnname(&mut self, bit_num :u8) -> Result<$rettype, ()> {
        let octet_num :usize = (bit_num / 8) as usize;
        if bit_num == 0 {
            // TODO: one day let bpc_read_body handle this,
            // if its smartly doable in there.
            // For why it is required, see comment in the
            // test_bitpacking_reader_empty function.
            return Ok(0);
        }
        assert!(bit_num <= $bit_num_max);
        bpc_read_body!($rettype, bit_num, octet_num, self)
    }
}
}

fn float32_unpack(val :u32) -> f64 {
    let sgn = (val & 0x80000000) as u64;
    let mut exp = (val & 0x7fe00000) as u64 >> 21;
    exp += 1023 - 768;
    // We & with 0x000fffff and not with 0x001fffff here as the spec says
    // because the IEE754 representation has an implicit leading bit.
    let mantissa = (val & 0x000fffff) as u64;
    let v = (sgn << 32) | (exp << 52) | (mantissa << 32);
    ::transmution_stuff::f64_transmute(v)
}

fn float32_unpack_to_32_directly(val :u32) -> f32 {
    let sgn = (val & 0x80000000) as u32;
    let mut exp = (val & 0x7fe00000) as u32 >> 21;
    // If this overflows, we are in trouble:
    // The number can't be represented with our f32 number system.
    exp -= 768 - 127;
    // We & with 0x000fffff and not with 0x001fffff here as the spec says
    // because the IEE754 representation has an implicit leading bit.
    let mantissa = (val & 0x000fffff) as u32;
    let v = sgn | (exp << 23) | (mantissa << 3);
    ::transmution_stuff::f32_transmute(v)
}

#[test]
fn test_float_32_unpack() {
    // Values were printed out from what stb_vorbis
    // calculated for this function from a test file.
    assert_eq!(float32_unpack(1611661312),      1.000000);
    assert_eq!(float32_unpack(1616117760),      5.000000);
    assert_eq!(float32_unpack(1618345984),     11.000000);
    assert_eq!(float32_unpack(1620115456),     17.000000);
    assert_eq!(float32_unpack(1627381760),    255.000000);
    assert_eq!(float32_unpack(3759144960),     -1.000000);
    assert_eq!(float32_unpack(3761242112),     -2.000000);
    assert_eq!(float32_unpack(3763339264),     -4.000000);
    assert_eq!(float32_unpack(3763601408),     -5.000000);
    assert_eq!(float32_unpack(3765436416),     -8.000000);
    assert_eq!(float32_unpack(3765829632),    -11.000000);
    assert_eq!(float32_unpack(3768451072),    -30.000000);
    assert_eq!(float32_unpack(3772628992),   -119.000000);
    assert_eq!(float32_unpack(3780634624),  -1530.000000);
}

#[test]
fn test_float32_unpack_to_32_directly() {
    // Values were printed out from what stb_vorbis
    // calculated for this function from a test file.
    assert_eq!(float32_unpack_to_32_directly(1611661312),      1.000000);
    assert_eq!(float32_unpack_to_32_directly(1616117760),      5.000000);
    assert_eq!(float32_unpack_to_32_directly(1618345984),     11.000000);
    assert_eq!(float32_unpack_to_32_directly(1620115456),     17.000000);
    assert_eq!(float32_unpack_to_32_directly(1627381760),    255.000000);
    assert_eq!(float32_unpack_to_32_directly(3759144960),     -1.000000);
    assert_eq!(float32_unpack_to_32_directly(3761242112),     -2.000000);
    assert_eq!(float32_unpack_to_32_directly(3763339264),     -4.000000);
    assert_eq!(float32_unpack_to_32_directly(3763601408),     -5.000000);
    assert_eq!(float32_unpack_to_32_directly(3765436416),     -8.000000);
    assert_eq!(float32_unpack_to_32_directly(3765829632),    -11.000000);
    assert_eq!(float32_unpack_to_32_directly(3768451072),    -30.000000);
    assert_eq!(float32_unpack_to_32_directly(3772628992),   -119.000000);
    assert_eq!(float32_unpack_to_32_directly(3780634624),  -1530.000000);
}

impl <'a> BitpackCursor <'a> {

    /// Creates a new `BitpackCursor` for the given data array
    pub fn new(arr : &'a[u8]) -> BitpackCursor {
        BitpackCursor::<'a> { bit_cursor: 0, byte_cursor: 0, inner: arr }
    }

    // Unsigned, non-dynamic reader methods

    // u32 based

    // TODO add here if needed
    uk_reader!(read_u32, u32, 32, 4);
    // TODO add here if needed
    uk_reader!(read_u24, u32, 24, 3);
    // TODO add here if needed

    // u16 based

    uk_reader!(read_u16, u16, 16, 2);

    // TODO add here if needed
    uk_reader!(read_u13, u16, 13, 1);
    // TODO add here if needed

    // u8 based
    uk_reader!(read_u8, u8, 8, 1);
    uk_reader!(read_u7, u8, 7, 0);
    uk_reader!(read_u6, u8, 6, 0);
    uk_reader!(read_u5, u8, 5, 0);
    uk_reader!(read_u4, u8, 4, 0);
    uk_reader!(read_u3, u8, 3, 0);
    uk_reader!(read_u2, u8, 2, 0);
    uk_reader!(read_u1, u8, 1, 0);

    // Returning bool:
    #[inline]
    pub fn read_bit_flag(&mut self) -> Result<bool, ()> {
        Ok(try!(self.read_u1()) == 1)
    }

    // Unsigned dynamic reader methods
    // They panic if you give them invalid params
    // (bit_num larger than maximum allowed bit number for the type)
    uk_dynamic_reader!(read_dyn_u8,  u8,  8);
    uk_dynamic_reader!(read_dyn_u16, u16, 16);
    uk_dynamic_reader!(read_dyn_u32, u32, 32);

    // Signed non-dynamic reader methods

    ik_reader!(read_i32, i32, 32, 32, 4);
    // TODO add here if needed

    ik_reader!(read_i8, i8, 8, 8, 1);
    ik_reader!(read_i7, i8, 8, 7, 0);
    // TODO add here if needed

    // Signed dynamic reader methods
    // They panic if you give them invalid params
    // (bit_num larger than maximum allowed bit number for the type)
    ik_dynamic_reader!(read_dyn_i8,  i8,  8);
    ik_dynamic_reader!(read_dyn_i16, i16, 16);
    ik_dynamic_reader!(read_dyn_i32, i32, 32);

    // Float reading methods

    /// Reads single float in the vorbis-float32 format
    ///
    /// This function will read 32 bits, but its return type is `f64`,
    /// as only rust's `f64` type can fully contain numbers
    /// converted from the vorbis-float32 format.
    /// If you prefer conciseness (and speed) over correctness,
    /// then you should use the `read_f32_lossy` method.
    pub fn read_f32(&mut self) -> Result<f64, ()> {
        let val = try!(self.read_u32());
        Ok(float32_unpack(val))
    }

    /// Reads single float in the vorbis-float32 format
    ///
    /// This function will read 32 bits, and its return type is `f64`.
    /// Some information from the exponent had to be discarded
    /// in order to be compatible with the native
    /// as only rust's `f64` type can fully contain numbers
    /// converted from the vorbis-float32 format.
    /// If you prefer conciseness (and speed) over correctness,
    /// then you should use the `read_f32_lossy` method.
    pub fn read_f32_lossy(&mut self) -> Result<f32, ()> {
        let val = try!(self.read_u32());
        let exp = val & 0x7fe00000;
        let exp_val :i16 = (exp >> 21) as i16 - 768;
        // Probably its not very performant to ask the FPU to
        // find the closest f32 to a given f64 value, therefore
        // we only do this if its actually needed (if the exponent
        // can't be directly translated into f32 representation)
        if (exp_val > 128) || exp_val < -127 {
            Ok(float32_unpack(val) as f32)
        } else {
            Ok(float32_unpack_to_32_directly(val))
        }
    }

    /// Reads a huffman word using the codebook abstraction
    pub fn read_huffman(&mut self, tree :&VorbisHuffmanTree) -> Result<u32, ()> {
        let mut iter = tree.iter();
        //let mut c :usize = 0;
        //let mut w :usize = 0;
        loop {
            let b = try!(self.read_bit_flag());
            /*
            c +=1;
            w >>= 1;
            w |= (b as usize) << 63;
            // Put this into the Some arm of the match below in order to debug:
            {print!("({}:{}:{}) ", w >> (64 - c), v, c); }
            // */
            if let Some(v) = iter.next(b) {
                return Ok(v)
            }
        }
    }
}

#[test]
fn test_bitpacking_reader_static() {
    // Test vectors taken from Vorbis I spec, section 2.1.6
    let test_arr = &[0b11111100, 0b01001000, 0b11001110, 0b00000110];
    let mut cur = BitpackCursor::new(test_arr);
    assert_eq!(cur.read_u4().unwrap(),  12);
    assert_eq!(cur.read_u3().unwrap(),  7);
    assert_eq!(cur.read_u7().unwrap(),  17);
    assert_eq!(cur.read_u13().unwrap(), 6969);
}

#[test]
fn test_bitpacking_reader_dynamic() {
    // Test vectors taken from Vorbis I spec, section 2.1.6
    let test_arr = &[0b11111100, 0b01001000, 0b11001110, 0b00000110];
    let mut cur = BitpackCursor::new(test_arr);
    assert_eq!(cur.read_dyn_u8(4).unwrap(),   12);
    assert_eq!(cur.read_dyn_u8(3).unwrap(),   7);
    assert_eq!(cur.read_dyn_u16(7).unwrap(),  17);
    assert_eq!(cur.read_dyn_u16(13).unwrap(), 6969);

    // Regression test for bug
    let test_arr = &[93, 92];
    let mut cur = BitpackCursor::new(test_arr);
    assert_eq!(cur.read_dyn_u32(10).unwrap(), 93);
}

#[test]
fn test_bitpacking_reader_empty() {
    // Same as the normal bitpacking test
    // but with some additional empty reads.
    //
    // This is expected to happen by the vorbis spec.
    // For example, the mode_number read in the audio packet
    // decode at first position may be 0 bit long (if there
    // is only one mode, ilog([vorbis_mode_count] - 1) is zero).

    let test_arr = &[0b11111100, 0b01001000, 0b11001110, 0b00000110];
    let mut cur = BitpackCursor::new(test_arr);
    assert_eq!(cur.read_dyn_u8(4).unwrap(),   12);
    assert_eq!(cur.read_dyn_u8(0).unwrap(),   0);
    assert_eq!(cur.read_dyn_u8(0).unwrap(),   0);
    assert_eq!(cur.read_dyn_u8(3).unwrap(),   7);
    assert_eq!(cur.read_dyn_u8(0).unwrap(),   0);
    assert_eq!(cur.read_dyn_u16(7).unwrap(),  17);
    assert_eq!(cur.read_dyn_u16(0).unwrap(),   0);
    assert_eq!(cur.read_dyn_u16(0).unwrap(),   0);
    assert_eq!(cur.read_dyn_u16(13).unwrap(), 6969);
    assert_eq!(cur.read_dyn_u16(0).unwrap(),   0);
}

#[test]
fn test_bitpacking_reader_byte_aligned() {
    // Check that bitpacking readers work with "normal" byte aligned types:
    let test_arr = &[0x00, 0x00, 0x00, 0x00, 0x01];
    let mut cur = BitpackCursor::new(test_arr);
    assert_eq!(cur.read_dyn_u32(32).unwrap(), 0);
    assert_eq!(cur.read_dyn_u8(8).unwrap(),   1);

    // We not just check here whether it works for byte aligned
    // "normal" (non-dynamic) reader methods, we also check
    // whether, after reading first one, then seven bits,
    // it "gets back" to byte alignment (and increases the byte ctr)
    let test_arr = &[0x09, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    let mut cur = BitpackCursor::new(test_arr);
    assert_eq!(cur.read_u1().unwrap(), 1);
    assert_eq!(cur.read_u7().unwrap(), 4);
    assert_eq!(cur.read_i8().unwrap(), 2);
    assert_eq!(cur.read_u32().unwrap(), 0);
    assert_eq!(cur.read_u8().unwrap(), 1);
}

#[test]
fn test_capture_pattern_nonaligned() {
    // Regression test from test OGG file
    // Tests for proper codebook capture
    // pattern reading.
    //
    // The OGG vorbis capture pattern
    // is a three octet (24 bits) value.
    //
    // The first block tests capture pattern
    // reading in a byte aligned scenario.
    // The actually problematic part was
    // the second block: it tests capture
    // pattern reading in a non-aligned
    // situation.

    let capture_pattern_arr = &[0x42, 0x43, 0x56];
    let mut cur = BitpackCursor::new(capture_pattern_arr);
    assert_eq!(cur.read_u24().unwrap(), 0x564342);

    let test_arr = &[0x28, 0x81, 0xd0, 0x90, 0x55, 0x00, 0x00];
    let mut cur = BitpackCursor::new(test_arr);
    cur.read_u5().unwrap(); // some value we are not interested in
    cur.read_u5().unwrap(); // some value we are not interested in
    assert_eq!(cur.read_u4().unwrap(), 0);
    assert_eq!(cur.read_u24().unwrap(), 0x564342);
    // Ensure that we incremented by only three bytes, not four
    assert_eq!(cur.read_u16().unwrap(), 1);
}
