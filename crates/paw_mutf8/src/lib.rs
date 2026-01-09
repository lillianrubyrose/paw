//! Fast and simple MUTF-8 encoder/decoder.
//!
//! <https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.7>

use std::borrow::Cow;

use num_conv::Truncate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Mutf8DecodeError {
	#[error("Invalid sequence")]
	InvalidSequence,
	#[error("Unexpected end")]
	UnexpectedEnd,
	#[error("Invalid surrogate")]
	InvalidSurrogate,
}

fn encoded_len(s: &str) -> usize {
	let bytes = s.as_bytes();
	let mut len = bytes.len();

	for &b in bytes {
		if b == 0 {
			len += 1;
		} else if b >= 0xF0 {
			len += 2;
		}
	}

	len
}

/// Encodes a Rust string to the MUTF-8 format.
pub fn encode<S: AsRef<str> + ?Sized>(input: &S) -> Cow<'_, [u8]> {
	let input = input.as_ref();

	let encoded_len = encoded_len(input);
	if encoded_len == input.len() {
		return Cow::Borrowed(input.as_bytes());
	}

	encode_slow(input.as_bytes(), encoded_len)
}

#[cold]
fn encode_slow(input: &[u8], capacity: usize) -> Cow<'_, [u8]> {
	let mut output = Vec::with_capacity(capacity);
	let mut i = 0;
	let len = input.len();

	while i < len {
		// SAFETY: Loop ensures this condition
		let b = unsafe { *input.get_unchecked(i) };
		if b == 0x00 {
			// 0x00 -> 0xC0 0x80
			output.extend_from_slice(&[0xC0, 0x80]);
			i += 1;
		} else if b < 0xF0 {
			// Just push if it's a 1-3 byte sequence
			output.push(b);
			i += 1;
		} else {
			// 4-byte sequence -> surrogate pair
			debug_assert!(i + 3 < len);

			// SAFETY: The passed string slice should always be valid UTF-8,
			//         ensuring that 4 bytes exist starting at 0xF0
			let b1 = b;
			let b2 = unsafe { *input.get_unchecked(i + 1) };
			let b3 = unsafe { *input.get_unchecked(i + 2) };
			let b4 = unsafe { *input.get_unchecked(i + 3) };

			let codepoint = ((u32::from(b1) & 0x07) << 18)
				| ((u32::from(b2) & 0x3F) << 12)
				| ((u32::from(b3) & 0x3F) << 6)
				| (u32::from(b4) & 0x3F);

			// UTF-16 surrogate pair
			let offset = codepoint - 0x10000;
			let high = 0xD800 | ((offset >> 10).truncate::<u16>());
			let low = 0xDC00 | ((offset & 0x3FF).truncate::<u16>());

			output.push(0xE0 | ((high >> 12) as u8));
			output.push(0x80 | (((high >> 6) & 0x3F) as u8));
			output.push(0x80 | ((high & 0x3F) as u8));

			output.push(0xE0 | ((low >> 12) as u8));
			output.push(0x80 | (((low >> 6) & 0x3F) as u8));
			output.push(0x80 | ((low & 0x3F) as u8));

			i += 4;
		}
	}

	// This would indicate a bug in our encode_len function
	debug_assert_eq!(output.capacity(), capacity);

	Cow::Owned(output)
}

/// Decodes MUTF-8 to a Rust string.
pub fn decode(input: &[u8]) -> Result<Cow<'_, str>, Mutf8DecodeError> {
	// The std from_utf8 will fail on MUTF-8 null or surrogate headers
	if let Ok(s) = std::str::from_utf8(input) {
		return Ok(Cow::Borrowed(s));
	}

	decode_slow(input).map(Cow::Owned)
}

#[cold]
fn decode_slow(input: &[u8]) -> Result<String, Mutf8DecodeError> {
	let mut out = String::with_capacity(input.len());

	// By pushing directly to the vector we avoid an unnecessary UTF-8 check on every call
	let out_vec = unsafe { out.as_mut_vec() };

	let mut i = 0;
	let len = input.len();

	while i < len {
		// SAFETY: Loop ensures this condition
		let b1 = unsafe { *input.get_unchecked(i) };
		i += 1;

		if b1 < 0x80 {
			out_vec.push(b1);
		} else if b1 < 0xE0 {
			// 2-byte sequence
			if i >= len {
				return Err(Mutf8DecodeError::UnexpectedEnd);
			}

			// SAFETY: i < len checked above
			let b2 = unsafe { *input.get_unchecked(i) };
			i += 1;

			if b1 == 0xC0 && b2 == 0x80 {
				out_vec.push(0x00);
			} else {
				if b2 & 0xC0 != 0x80 {
					return Err(Mutf8DecodeError::InvalidSequence);
				}

				out_vec.push(b1);
				out_vec.push(b2);
			}
		} else if b1 < 0xF0 {
			// 3-byte sequence
			if i + 1 >= len {
				return Err(Mutf8DecodeError::UnexpectedEnd);
			}

			// SAFETY: i + 1 < len checked above
			let b2 = unsafe { *input.get_unchecked(i) };
			let b3 = unsafe { *input.get_unchecked(i + 1) };
			i += 2;

			if b1 == 0xED && (0xA0..=0xAF).contains(&b2) {
				// A low surrogate should follow a high surrogate
				if i + 2 >= len {
					return Err(Mutf8DecodeError::UnexpectedEnd);
				}

				// SAFETY: i + 2 < len checked above
				let b4 = unsafe { *input.get_unchecked(i) };
				let b5 = unsafe { *input.get_unchecked(i + 1) };
				let b6 = unsafe { *input.get_unchecked(i + 2) };
				i += 3;

				// Low surrgate should be 0xED or 0xB0..0xBF
				if b4 != 0xED || !(0xB0..=0xBF).contains(&b5) {
					return Err(Mutf8DecodeError::InvalidSurrogate);
				}

				let high = ((u32::from(b1) & 0x0F) << 12) | ((u32::from(b2) & 0x3F) << 6) | (u32::from(b3) & 0x3F);
				let low = ((u32::from(b4) & 0x0F) << 12) | ((u32::from(b5) & 0x3F) << 6) | (u32::from(b6) & 0x3F);

				let codepoint = 0x10000 + (((high & 0x3FF) << 10) | (low & 0x3FF));

				out_vec.push(0xF0 | ((codepoint >> 18).truncate::<u8>()));
				out_vec.push(0x80 | (((codepoint >> 12) & 0x3F) as u8));
				out_vec.push(0x80 | (((codepoint >> 6) & 0x3F) as u8));
				out_vec.push(0x80 | ((codepoint & 0x3F) as u8));
			} else {
				// Standard 3-byte sequence
				if (b2 & 0xC0 != 0x80) || (b3 & 0xC0 != 0x80) {
					return Err(Mutf8DecodeError::InvalidSequence);
				}

				out_vec.push(b1);
				out_vec.push(b2);
				out_vec.push(b3);
			}
		} else {
			// 4-byte sequences aren't valid MUTF-8
			return Err(Mutf8DecodeError::InvalidSequence);
		}
	}

	Ok(out)
}

#[cfg(test)]
mod tests {
	use super::{decode, encode};

	#[test]
	fn test() {
		let cases = ["Hello, World!", "Hello\0World", "Hello 世界!", "🚀"];
		for input in cases {
			let encoded = encode(input);
			let decoded = decode(&encoded).expect("to not fail");
			assert_eq!(decoded, input);
		}
	}

	#[test]
	fn test_null_encoding() {
		let input = "\0";
		let encoded = encode(input);
		assert_eq!(encoded.as_ref(), &[0xC0, 0x80]);
	}

	#[test]
	fn test_surrogate_encoding() {
		let input = "\u{1F680}";
		let encoded = encode(input);

		assert_eq!(encoded.len(), 6);
		assert_eq!(encoded[0], 0xED);
		assert_eq!(encoded[3], 0xED);
	}
}
