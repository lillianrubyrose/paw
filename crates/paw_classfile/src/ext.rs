use std::io::{Read, Result, Write};

pub trait BytesReadExt: Read {
	fn read_u8(&mut self) -> Result<u8>;
	fn read_u16(&mut self) -> Result<u16>;
	fn read_u32(&mut self) -> Result<u32>;
	fn read_u64(&mut self) -> Result<u64>;
	fn read_u128(&mut self) -> Result<u128>;

	fn read_i8(&mut self) -> Result<i8>;
	fn read_i16(&mut self) -> Result<i16>;
	fn read_i32(&mut self) -> Result<i32>;
	fn read_i64(&mut self) -> Result<i64>;
	fn read_i128(&mut self) -> Result<i128>;

	fn read_f32(&mut self) -> Result<f32>;
	fn read_f64(&mut self) -> Result<f64>;

	/// reads a vector from a reader
	/// `len_t` is in units of T, **not** in units of bytes
	fn read_vec_with<T, F>(&mut self, len_t: usize, mut parser: F) -> eyre::Result<Vec<T>>
	where
		F: FnMut(&mut Self) -> eyre::Result<T>,
	{
		let mut v = Vec::with_capacity(len_t);
		for _ in 0..len_t {
			v.push(parser(self)?);
		}
		Ok(v)
	}
}

pub trait BytesWriteExt: Write {
	fn write_u8(&mut self, value: u8) -> Result<()>;
	fn write_u16(&mut self, value: u16) -> Result<()>;
	fn write_u32(&mut self, value: u32) -> Result<()>;
	fn write_u64(&mut self, value: u64) -> Result<()>;
	fn write_u128(&mut self, value: u128) -> Result<()>;

	fn write_i8(&mut self, value: i8) -> Result<()>;
	fn write_i16(&mut self, value: i16) -> Result<()>;
	fn write_i32(&mut self, value: i32) -> Result<()>;
	fn write_i64(&mut self, value: i64) -> Result<()>;
	fn write_i128(&mut self, value: i128) -> Result<()>;

	fn write_f32(&mut self, value: f32) -> Result<()>;
	fn write_f64(&mut self, value: f64) -> Result<()>;
}

impl<R: Read> BytesReadExt for R {
	fn read_u8(&mut self) -> Result<u8> {
		let mut buf = [0u8; 1];
		self.read_exact(&mut buf)?;
		Ok(buf[0])
	}

	fn read_u16(&mut self) -> Result<u16> {
		let mut buf = [0u8; 2];
		self.read_exact(&mut buf)?;
		Ok(u16::from_be_bytes(buf))
	}

	fn read_u32(&mut self) -> Result<u32> {
		let mut buf = [0u8; 4];
		self.read_exact(&mut buf)?;
		Ok(u32::from_be_bytes(buf))
	}

	fn read_u64(&mut self) -> Result<u64> {
		let mut buf = [0u8; 8];
		self.read_exact(&mut buf)?;
		Ok(u64::from_be_bytes(buf))
	}

	fn read_u128(&mut self) -> Result<u128> {
		let mut buf = [0u8; 16];
		self.read_exact(&mut buf)?;
		Ok(u128::from_be_bytes(buf))
	}

	fn read_i8(&mut self) -> Result<i8> {
		let mut buf = [0u8; 1];
		self.read_exact(&mut buf)?;
		Ok(i8::from_be_bytes(buf))
	}

	fn read_i16(&mut self) -> Result<i16> {
		let mut buf = [0u8; 2];
		self.read_exact(&mut buf)?;
		Ok(i16::from_be_bytes(buf))
	}

	fn read_i32(&mut self) -> Result<i32> {
		let mut buf = [0u8; 4];
		self.read_exact(&mut buf)?;
		Ok(i32::from_be_bytes(buf))
	}

	fn read_i64(&mut self) -> Result<i64> {
		let mut buf = [0u8; 8];
		self.read_exact(&mut buf)?;
		Ok(i64::from_be_bytes(buf))
	}

	fn read_i128(&mut self) -> Result<i128> {
		let mut buf = [0u8; 16];
		self.read_exact(&mut buf)?;
		Ok(i128::from_be_bytes(buf))
	}

	fn read_f32(&mut self) -> Result<f32> {
		let mut buf = [0u8; 4];
		self.read_exact(&mut buf)?;
		Ok(f32::from_be_bytes(buf))
	}

	fn read_f64(&mut self) -> Result<f64> {
		let mut buf = [0u8; 8];
		self.read_exact(&mut buf)?;
		Ok(f64::from_be_bytes(buf))
	}
}

impl<W: Write> BytesWriteExt for W {
	fn write_u8(&mut self, value: u8) -> Result<()> {
		self.write_all(&[value])
	}

	fn write_u16(&mut self, value: u16) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_u32(&mut self, value: u32) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_u64(&mut self, value: u64) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_u128(&mut self, value: u128) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_i8(&mut self, value: i8) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_i16(&mut self, value: i16) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_i32(&mut self, value: i32) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_i64(&mut self, value: i64) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_i128(&mut self, value: i128) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_f32(&mut self, value: f32) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}

	fn write_f64(&mut self, value: f64) -> Result<()> {
		self.write_all(&value.to_be_bytes())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Cursor;

	#[test]
	fn test_write_and_read_unsigned() {
		let mut buf = Vec::new();

		buf.write_u8(0x12).unwrap();
		buf.write_u16(0x1234).unwrap();
		buf.write_u32(0x12345678).unwrap();
		buf.write_u64(0x123456789ABCDEF0).unwrap();
		buf.write_u128(0x123456789ABCDEF0FEDCBA9876543210).unwrap();

		let mut slice = buf.as_slice();
		assert_eq!(slice.read_u8().unwrap(), 0x12);
		assert_eq!(slice.read_u16().unwrap(), 0x1234);
		assert_eq!(slice.read_u32().unwrap(), 0x12345678);
		assert_eq!(slice.read_u64().unwrap(), 0x123456789ABCDEF0);
		assert_eq!(slice.read_u128().unwrap(), 0x123456789ABCDEF0FEDCBA9876543210);
		assert_eq!(slice.len(), 0);
	}

	#[test]
	fn test_write_and_read_signed() {
		let mut buf = Vec::new();

		buf.write_i8(-42).unwrap();
		buf.write_i16(-1234).unwrap();
		buf.write_i32(-123456).unwrap();
		buf.write_i64(-123456789).unwrap();
		buf.write_i128(-123456789123456789).unwrap();

		let mut slice = buf.as_slice();
		assert_eq!(slice.read_i8().unwrap(), -42);
		assert_eq!(slice.read_i16().unwrap(), -1234);
		assert_eq!(slice.read_i32().unwrap(), -123456);
		assert_eq!(slice.read_i64().unwrap(), -123456789);
		assert_eq!(slice.read_i128().unwrap(), -123456789123456789);
		assert_eq!(slice.len(), 0);
	}

	#[test]
	fn test_write_and_read_floats() {
		let mut buf = Vec::new();

		buf.write_f32(3.14159).unwrap();
		buf.write_f64(2.718281828459045).unwrap();

		let mut slice = buf.as_slice();
		assert!((slice.read_f32().unwrap() - 3.14159).abs() < 0.00001);
		assert!((slice.read_f64().unwrap() - 2.718281828459045).abs() < 0.00000000001);
		assert_eq!(slice.len(), 0);
	}

	#[test]
	fn test_read_with_cursor() {
		let data = vec![0x12, 0x34, 0x56, 0x78];
		let mut cursor = Cursor::new(data);

		assert_eq!(cursor.read_u16().unwrap(), 0x1234);
		assert_eq!(cursor.read_u16().unwrap(), 0x5678);
	}

	#[test]
	fn test_insufficient_bytes() {
		let data = vec![0x12];
		let mut slice = data.as_slice();

		assert!(slice.read_u16().is_err());
	}
}
