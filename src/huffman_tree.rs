// Vorbis decoder written in Rust
//
// Copyright (c) 2016 est31 <MTest31@outlook.com>
// and contributors. All rights reserved.
// Licensed under MIT license, or Apache 2 license,
// at your option. Please see the LICENSE file
// attached to this source distribution for details.

/*!
Huffman tree unpacking and traversal

This mod contains the `VorbisHuffmanTree` struct which
can be loaded from the `codebook_codeword_lengths` array
specified for each codebook in the vorbis setup header.

Once decoding is happening, you are more interested in
the `VorbisHuffmanIter` struct which provides you with
facilities to load a value bit by bit.
*/

struct HuffTree {
	// True iff every sub-tree in this tree
	// either has two direct children or none
	even_childs :bool,
	payload :Option<u32>,
	l :Option<Box<HuffTree>>,
	r :Option<Box<HuffTree>>,
}

/*
use std::fmt;
impl fmt::Debug for HuffTree {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fn fmt_rec(s :&HuffTree, f: &mut fmt::Formatter, depth :u32) -> fmt::Result {
			macro_rules! depth_print {
			($f:ident, $depth:ident) => {
				for _ in 0..$depth {
					try!(write!($f, "| "));
				}
			}}
			if s.l.is_some() || s.r.is_some() {
				try!(writeln!(f, "ec: {:?}, pl: {:?}, LIS {:?} RIS {:?}",
					s.even_childs, s.payload, s.l.is_some(), s.r.is_some()));
			} else {
				try!(writeln!(f, "ec: {:?}, pl: {:?}", s.even_childs, s.payload));
			}
			if let Some(ref v) = s.l {
				depth_print!(f, depth);
				try!(write!(f, "LEFT "));
				try!(fmt_rec(&*v, f, depth + 1));
			}
			if let Some(ref v) = s.r {
				depth_print!(f, depth);
				try!(write!(f, "RIGT "));
				try!(fmt_rec(&*v, f, depth + 1));
			}
			return Ok(());
		}
		try!(fmt_rec(self, f, 1));
		return Ok(());
	}
} // */

impl HuffTree {
	/// Returns whether the addition was successful
	pub fn insert_rec(&mut self, payload :u32, depth :u8) -> bool {
		//print!("INSERT payload {:?} depth {:?} ", payload, depth);
		if self.payload.is_some() {
			//println!(" => OCCUPIED AS LEAF");
			return false;
		}
		if depth == 0 {
			if !(self.l.is_none() && self.r.is_none()) {
				//println!(" => INNER NODE");
				return false;
			}
			self.payload = Some(payload);
			//println!(" => ADDED");
			return true;
		}
		if self.even_childs {
			//println!(" => HAS EVEN CHILDS");
			match &mut self.l {
				&mut Some(_) => return false,
				&mut None => {
					let mut new_node = HuffTree { even_childs :true, payload :None, l :None, r :None };
					new_node.insert_rec(payload, depth - 1);
					self.l = Some(Box::new(new_node));
					self.even_childs = false;
					return true;
				}
			}
		} else {
			//println!(" => HAS NOT EVEN CHILDS");
			// First try left branch
			let left = self.l.as_mut().unwrap();
			if !left.even_childs {
				if left.insert_rec(payload, depth - 1) {
					self.even_childs = left.even_childs &&
						if let &mut Some(ref mut right) = &mut self.r.as_mut() { right.even_childs } else { false };
					return true;
				}
			}
			// Left sub tree was either full or leaf
			// Therefore, put it in the right branch now
			// As left has even_childs == true, right causes
			// us to have even_childs == false.
			return match self.r {
				Some(ref mut right) => {
					let success = right.insert_rec(payload, depth - 1);
					self.even_childs = left.even_childs && right.even_childs;
					success
				},
				None => {
					let mut new_node = HuffTree { even_childs :true, payload :None, l :None, r :None };
					let success = new_node.insert_rec(payload, depth - 1);
					self.even_childs = left.even_childs && new_node.even_childs;
					self.r = Some(Box::new(new_node));
					success
				}
			};
		}
	}
}

#[derive(Debug)]
pub enum HuffmanError {
	Overspecified,
	Underpopulated,
	InvalidSingleEntry,
}

#[derive(Clone, Copy)]
enum UnrolledLookupEntry {
	/// The specified entry was found in the lookup array
	///
	/// First param: offset by which to advance the reader
	/// Second param: the payload
	HasEntry(u8, u32),
	/// Seems the given input is inconclusive and not complete yet.
	///
	/// The argument contains a hint that is an offset inside desc_prog
	/// to help to advance the reader.
	InconclusiveWithHint(u32),
	/// Seems the given input is inconclusive and not complete yet.
	Inconclusive,
}

pub enum PeekedDataLookupResult<'l> {
	/// The supplied info is not enough to result in a payload directly.
	///
	/// First param is the number of bits to advance.
	///
	/// The returned iterator has state up to the count of bits that could be used.
	Iter(u8, VorbisHuffmanIter<'l>),
	/// The supplied info is enough to map to a payload
	///
	/// First param is the number of bits to advance. Second is payload.
	PayloadFound(u8, u32),
}

/// Huffman tree representation
pub struct VorbisHuffmanTree {
	// Format: three bytes per non leaf node, one byte per leaf node.
	// First byte is the payload container,
	// second and third point to the indices inside the vector that
	// have left and right children.
	// If the node is a leaf the highest bit of the payload container 0,
	// if it has children the bit is 1. If its a leaf the lower 31 bits of the
	// payload container form the actual payload.
	desc_prog :Vec<u32>,

	unrolled_entries :[UnrolledLookupEntry; 256],
}

impl VorbisHuffmanTree {
	/// Constructs a new `VorbisHuffmanTree` instance from the passed array,
	/// like the vorbis spec demands.
	///
	/// Returns the resulting tree if the array results in a valid (neither
	/// underspecified nor overspecified) tree.
	pub fn load_from_array(codebook_codeword_lengths :&[u8]) -> Result<VorbisHuffmanTree, HuffmanError> {
		// First step: generate a simple tree representing the
		// Huffman tree
		let mut simple_tree = HuffTree { even_childs :true, payload :None, l :None, r :None };
		let mut cnt :usize = 0;
		let mut last_valid_idx = None;
		for (i, &codeword_length) in codebook_codeword_lengths.iter().enumerate() {
			if codeword_length == 0 {
				continue;
			}
			cnt += 1;
			last_valid_idx = Some(i);
			if !simple_tree.insert_rec(i as u32, codeword_length) {
				try!(Err(HuffmanError::Overspecified)) /* Overspecified, can't be put into tree */
			}
		}
		//println!("The tree:\n{:?}", simple_tree);

		// Single entry codebook special handling
		if cnt == 1 {
			let decoded = last_valid_idx.unwrap();
			let encoded_len = codebook_codeword_lengths[decoded];
			if encoded_len == 1 {
				// Return a vorbis tree that returns decoded for any single bit input
				return Ok(VorbisHuffmanTree {
					desc_prog :vec![1u32 << 31, 3, 3, decoded as u32],
					unrolled_entries :[
						UnrolledLookupEntry::HasEntry(1, decoded as u32); 256
					],
				});
			} else {
				// Single entry codebooks must have 1 as their only length entry
				try!(Err(HuffmanError::InvalidSingleEntry))
			}
		}

		if !simple_tree.even_childs {
			try!(Err(HuffmanError::Underpopulated)); /* Underpopulated */
		}

		// Second step: generate the actual desc_prog
		// by pre_order traversal of the tree.
		//
		// The general advantage of this approach over one with only the simple tree
		// is better cache locality and less memory requirements (at least after the
		// setup with the simple tree).
		let mut desc_prog = Vec::with_capacity(cnt);
		fn traverse(tree :& HuffTree, desc_prog :&mut Vec<u32>) -> u32 {
			let cur_pos = desc_prog.len() as u32;
			let has_children = tree.l.is_some() || tree.r.is_some();

			let entry = ((has_children as u32) << 31) | tree.payload.unwrap_or(0);
			//println!("push node (w_children : {:?}) at {:?} : {:?}", has_children, cur_pos, entry);
			desc_prog.push(entry);

			if has_children {
				desc_prog.push(0);
				desc_prog.push(0);
				desc_prog[cur_pos as usize + 1] =
					traverse(tree.l.as_ref().unwrap(), desc_prog);
				/*println!("left child of node {:?}: at {:?}", cur_pos,
					desc_prog[cur_pos as usize + 1]);// */
				desc_prog[cur_pos as usize + 2] =
					traverse(tree.r.as_ref().unwrap(), desc_prog);
				/*println!("right child of node {:?}: at {:?}", cur_pos,
					desc_prog[cur_pos as usize + 2]);// */
			}
			return cur_pos;
		}
		assert_eq!(traverse(&simple_tree, &mut desc_prog), 0);

		// Third step: generate unrolled entries array
		// Also by pre_order traversal.
		//
		// This gives us a speedup over desc_prog as reading the unrolled
		// entries should involve less branching and less lookups overall.
		let mut unrolled_entries = [UnrolledLookupEntry::Inconclusive; 256];
		fn uroll_traverse(tree :& HuffTree,
				unrolled_entries :&mut [UnrolledLookupEntry; 256],
				prefix :u32, prefix_idx :u8,
				desc_prog :&[u32], desc_prog_idx :u32) {
			let has_children = tree.l.is_some() || tree.r.is_some();

			if has_children {
				// There are children.
				// We'd like to recurse deeper. Can we?
				if prefix_idx == 8 {
					// No we can't.
					// The tree is too deep.
					unrolled_entries[prefix as usize] =
						UnrolledLookupEntry::InconclusiveWithHint(desc_prog_idx);
				} else {
					// Recurse deeper.
					uroll_traverse(tree.l.as_ref().unwrap(),
						unrolled_entries,
						prefix + (0 << prefix_idx), prefix_idx + 1,
						desc_prog, desc_prog[desc_prog_idx as usize + 1]);
					uroll_traverse(tree.r.as_ref().unwrap(),
						unrolled_entries,
						prefix + (1 << prefix_idx), prefix_idx + 1,
						desc_prog, desc_prog[desc_prog_idx as usize + 2]);
				}
			} else {
				// No children, fill the entries in the range according to
				// the prefix we have.
				let payload = tree.payload.unwrap();
				let it = 1 << prefix_idx;
				let mut i = prefix as usize;
				for _ in 1 .. (1u16 << (8 - prefix_idx)) {
					unrolled_entries[i] =
						UnrolledLookupEntry::HasEntry(prefix_idx, payload);
					i += it;
				}
			}
		}
		if cnt > 0 {
			uroll_traverse(&simple_tree,
				&mut unrolled_entries, 0, 0, &desc_prog, 0);
		}

		// Now we are done, return the result
		return Ok(VorbisHuffmanTree {
			desc_prog,
			unrolled_entries,
		});
	}

	/// Returns an iterator over this tree.
	pub fn iter<'l>(&'l self) -> VorbisHuffmanIter<'l> {
		return VorbisHuffmanIter { desc_prog :&self.desc_prog, pos :0 };
	}

	/// Resolves a given number of peeked bits.
	///
	/// Returns whether the data given is enough to uniquely identify a
	/// tree element, or whether only an iterator that's progressed by
	/// a given amount can be returned. Also, info is returned about how
	/// far the reader can be advanced.
	pub fn lookup_peeked_data<'l>(&'l self, bit_count :u8, peeked_data :u32)
			-> PeekedDataLookupResult<'l> {
		if bit_count > 8 {
			panic!("Bit count {} larger than allowed 8", bit_count);
		}
		use self::UnrolledLookupEntry::*;
		use self::PeekedDataLookupResult::*;
		return match self.unrolled_entries[peeked_data as usize] {
			// If cnt_to_remove is bigger than bit_count the result is inconclusive.
			// Return in this case.
			HasEntry(cnt_to_remove, payload) if cnt_to_remove <= bit_count
				=> PayloadFound(cnt_to_remove, payload),
			InconclusiveWithHint(hint)
				=> Iter(8, VorbisHuffmanIter { desc_prog : &self.desc_prog, pos : hint }),
			_
				=> Iter(0, VorbisHuffmanIter { desc_prog : &self.desc_prog, pos : 0 }),
		};
	}
}

/// Iterator on the Huffman tree
pub struct VorbisHuffmanIter<'a> {
	desc_prog :&'a Vec<u32>,
	pos :u32,
}

impl<'a> VorbisHuffmanIter<'a> {
	/// Iterate one level deeper inside the tree.
	/// Returns `Some(p)` if it encounters a leaf with a payload p,
	/// None if it only processed an inner node.
	///
	/// Inner nodes don't carry payloads in huffman trees.
	///
	/// If this function encounters a leaf, it automatically resets
	/// the iterator to its starting state.
	///
	/// # Panics
	///
	/// Panics if the vorbis huffman treee is empty. It has to be found out
	/// what to do if the huffman tree is empty, whether to reject the stream,
	/// or whether to do sth else. Finding this out is a TODO.
	pub fn next(&mut self, bit :bool) -> Option<u32> {
		// Assertion test for the paranoid and testing, comment out if you are:
		/*let cur_entry = self.desc_prog[self.pos as usize];
		assert!((cur_entry & (1u32 << 31)) != 0);*/

		//print!("With bit {:?}, pos {:?} becomes pos ", bit, self.pos);
		self.pos = self.desc_prog[self.pos as usize + 1 + bit as usize];
		//print!("{:?}", self.pos);
		let child = self.desc_prog[self.pos as usize];
		if (child & (1u32 << 31)) != 0 {
			//println!(" => None");
			// child has children
			return None;
		} else {
			//println!(" => Some({:?})", child);
			// child has no children, it's a leaf
			self.pos = 0;
			return Some(child);
		}
	}
}

#[cfg(test)]
impl VorbisHuffmanTree {
	fn iter_test(&self, path :u32, path_len :u8, expected_val :u32) {
		let mut itr = self.iter();
		for i in 1 .. path_len {
			assert_eq!(itr.next((path & (1 << (path_len - i))) != 0), None);
		}
		assert_eq!(itr.next((path & 1) != 0), Some(expected_val));
	}
}

#[test]
fn test_huffman_tree() {
	// Official example from the vorbis spec section 3.2.1
	let tree = VorbisHuffmanTree::load_from_array(&[2, 4, 4, 4, 4, 2, 3, 3]).unwrap();

	tree.iter_test(0b00, 2, 0);
	tree.iter_test(0b0100, 4, 1);
	tree.iter_test(0b0101, 4, 2);
	tree.iter_test(0b0110, 4, 3);
	tree.iter_test(0b0111, 4, 4);
	tree.iter_test(0b10, 2, 5);
	tree.iter_test(0b110, 3, 6);
	tree.iter_test(0b111, 3, 7);

	// Some other example
	// we mostly test the length (max 32) here
	VorbisHuffmanTree::load_from_array(&[
		1,   2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15, 16,
		17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 32]).unwrap();
}

#[test]
fn test_issue_8() {
	// regression test for issue 8
	// make sure that it doesn't panic.
	let _ = VorbisHuffmanTree::load_from_array(&[0; 625]);
}

#[test]
fn test_under_over_spec() {
	// All trees base on the official example from the vorbis spec section 3.2.1
	// but with modifications to under- or overspecify them

	// underspecified
	let tree = VorbisHuffmanTree::load_from_array(&[2, 4, 4, 4, 4, 2, 3/*, 3*/]);
	assert!(tree.is_err());

	// underspecified
	let tree = VorbisHuffmanTree::load_from_array(&[2, 4, 4, 4, /*4,*/ 2, 3, 3]);
	assert!(tree.is_err());

	// overspecified
	let tree = VorbisHuffmanTree::load_from_array(&[2, 4, 4, 4, 4, 2, 3, 3/*]*/,3]);
	assert!(tree.is_err());
}

#[test]
fn test_single_entry_huffman_tree() {
	// Special testing for single entry codebooks, as required by the vorbis spec
	let tree = VorbisHuffmanTree::load_from_array(&[1]).unwrap();
	tree.iter_test(0b0, 1, 0);
	tree.iter_test(0b1, 1, 0);

	let tree = VorbisHuffmanTree::load_from_array(&[0, 0, 1, 0]).unwrap();
	tree.iter_test(0b0, 1, 2);
	tree.iter_test(0b1, 1, 2);

	let tree = VorbisHuffmanTree::load_from_array(&[2]);
	assert!(tree.is_err());
}

#[test]
fn test_unordered_huffman_tree() {
	// Reordered the official example from the vorbis spec section 3.2.1
	//
	// Ensuring that unordered huffman trees work as well is important
	// because the spec does not disallow them, and unordered
	// huffman trees appear in "the wild".
	let tree = VorbisHuffmanTree::load_from_array(&[2, 4, 4, 2, 4, 4, 3, 3]).unwrap();

	tree.iter_test(0b00, 2, 0);
	tree.iter_test(0b0100, 4, 1);
	tree.iter_test(0b0101, 4, 2);
	tree.iter_test(0b10, 2, 3);
	tree.iter_test(0b0110, 4, 4);
	tree.iter_test(0b0111, 4, 5);
	tree.iter_test(0b110, 3, 6);
	tree.iter_test(0b111, 3, 7);
}

#[test]
fn test_extracted_huffman_tree() {
	// Extracted from a real-life vorbis file.
	VorbisHuffmanTree::load_from_array(&[
	5,  6, 11, 11, 11, 11, 10, 10, 12, 11,  5,  2, 11,  5,  6,  6,
	7,  9, 11, 13, 13, 10,  7, 11,  6,  7,  8,  9, 10, 12, 11,  5,
	11, 6,  8,  7,  9, 11, 14, 15, 11,  6,  6,  8,  4,  5,  7,  8,
	10,13, 10,  5,  7,  7,  5,  5,  6,  8, 10, 11, 10,  7,  7,  8,
	6,  5,  5,  7,  9,  9, 11,  8,  8, 11,  8,  7,  6,  6,  7,  9,
	12,11, 10, 13,  9,  9,  7,  7,  7,  9, 11, 13, 12, 15, 12, 11,
	9,  8,  8,  8]).unwrap();
}
