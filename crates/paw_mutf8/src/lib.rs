//! Fast and simple MUTF-8 encoder/decoder.
//!
//! <https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.7>

#![forbid(unsafe_code)] // prevent finding fork in kitchen

use std::borrow::Cow;

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

	let mut output = Vec::with_capacity(encoded_len);
	for c in input.chars() {
		let cp = c as u32;
		match cp {
			0x0001..=0x007F => {
				output.push(cp as u8);
			}
			0x0000 | 0x0080..=0x07FF => {
				let x = 0b110_00000 | ((cp >> 6) & 0x1F) as u8;
				let y = 0b10_000000 | (cp & 0x3F) as u8;
				output.push(x);
				output.push(y);
			}
			0x0800..=0xFFFF => {
				let x = 0b1110_0000 | ((cp >> 12) & 0x0F) as u8;
				let y = 0b10_000000 | ((cp >> 6) & 0x3F) as u8;
				let z = 0b10_000000 | (cp & 0x3F) as u8;
				output.push(x);
				output.push(y);
				output.push(z);
			}

			_ => {
				let u = 0b1110_1101;
				let v = 0b1010_0000 | (((cp >> 16) & 0x1F) - 1) as u8;
				let w = 0b10_000000 | ((cp >> 10) & 0x3F) as u8;
				let x = 0b1110_1101;
				let y = 0b1011_0000 | ((cp >> 6) & 0x0F) as u8;
				let z = 0b10_000000 | (cp & 0x3F) as u8;
				output.push(u);
				output.push(v);
				output.push(w);
				output.push(x);
				output.push(y);
				output.push(z);
			}
		}
	}

	Cow::Owned(output)
}

/// Decodes MUTF-8 to a Rust string.
pub fn decode(input: &[u8]) -> Result<Cow<'_, str>, Mutf8DecodeError> {
	// The std from_utf8 will fail on MUTF-8 null or surrogate headers
	if let Ok(s) = std::str::from_utf8(input) {
		return Ok(Cow::Borrowed(s));
	}

	let mut output = String::with_capacity(input.len());
	let mut i = 0;
	while i < input.len() {
		let b1 = input[i];

		if (b1 & 0b1000_0000) == 0b0000_0000 {
			output.push(b1 as char);
			i += 1;
		} else if (b1 & 0b1110_0000) == 0b1100_0000 {
			let b2 = *input.get(i + 1).ok_or(Mutf8DecodeError::UnexpectedEnd)?;
			if (b2 & 0b1100_0000) != 0b1000_0000 {
				return Err(Mutf8DecodeError::InvalidSequence);
			}
			let x = (b1 & 0x1F) as u32;
			let y = (b2 & 0x3F) as u32;
			output.push(char::from_u32((x << 6) | y).ok_or(Mutf8DecodeError::InvalidSequence)?);
			i += 2;
		} else if (b1 & 0b1111_0000) == 0b1110_0000 {
			let b2 = *input.get(i + 1).ok_or(Mutf8DecodeError::UnexpectedEnd)?;
			if (b2 & 0b1100_0000) != 0b1000_0000 {
				return Err(Mutf8DecodeError::InvalidSequence);
			}
			let b3 = *input.get(i + 2).ok_or(Mutf8DecodeError::UnexpectedEnd)?;
			if (b3 & 0b1100_0000) != 0b1000_0000 {
				return Err(Mutf8DecodeError::InvalidSequence);
			}

			// surrogate pair
			if b1 == 0b1110_1101 {
				// additional requirement on b2 for surrogate pair
				if (b2 & 0b1111_0000) != 0b1010_0000 {
					return Err(Mutf8DecodeError::InvalidSequence);
				}

				let b4 = *input.get(i + 3).ok_or(Mutf8DecodeError::UnexpectedEnd)?;
				if b4 != 0b1110_1101 {
					return Err(Mutf8DecodeError::InvalidSequence);
				}
				let b5 = *input.get(i + 4).ok_or(Mutf8DecodeError::UnexpectedEnd)?;
				if (b5 & 0b1111_0000) != 0b1011_0000 {
					return Err(Mutf8DecodeError::InvalidSequence);
				}
				let b6 = *input.get(i + 5).ok_or(Mutf8DecodeError::UnexpectedEnd)?;
				if (b6 & 0b1100_0000) != 0b1000_0000 {
					return Err(Mutf8DecodeError::InvalidSequence);
				}

				let w1 = (b2 as u32 & 0x0F) << 6;
				let w2 = b3 as u32 & 0x3F;
				let w = w1 | w2;

				let z1 = (b5 as u32 & 0x0F) << 6;
				let z2 = b6 as u32 & 0x3F;
				let z = z1 | z2;

				let cp = 0x10000 + ((w << 10) | z);
				output.push(char::from_u32(cp).unwrap());

				i += 6;
			} else {
				let x = (b1 & 0x0F) as u32;
				let y = (b2 & 0x3F) as u32;
				let z = (b3 & 0x3F) as u32;
				output.push(char::from_u32((x << 12) | (y << 6) | z).ok_or(Mutf8DecodeError::InvalidSequence)?);
				i += 3;
			}
		} else {
			return Err(Mutf8DecodeError::InvalidSequence);
		}
	}

	Ok(Cow::Owned(output))
}

#[cfg(test)]
mod tests {
	use super::{decode, encode};

	#[test]
	fn test_mutf8() {
		let cases = ["Hello, World!", "Hello\0World", "Hello 世界!", "🚀"];
		for input in cases {
			let encoded = encode(input);
			let decoded = decode(&encoded)
				.unwrap_or_else(|e| panic!("err {:?} when round tripping {:?} encoded {:?}", e, input, encoded));
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
