use bytemuck::AnyBitPattern;
use byteorder::ByteOrder;

pub trait ReadBytesExt: byteorder::ReadBytesExt {
	/// reads a vector from a reader
	/// `len_t` is in units of T, **not** in units of bytes
	fn read_vec<T: AnyBitPattern + std::fmt::Debug, O: ByteOrder>(&mut self, len_t: usize) -> std::io::Result<Vec<T>> {
		let mut v = Vec::with_capacity(len_t);
		let mut buf = vec![0_u8; const { core::mem::size_of::<T>() }];
		for _ in 0..len_t {
			self.read_exact(&mut buf)?;
			// FIXME: maybe just remove this method lol
			#[cfg(not(target_endian = "big"))]
			{
				buf.reverse();
			}
			v.push(bytemuck::pod_read_unaligned(&buf));
		}

		Ok(v)
	}
}

impl<T: byteorder::ReadBytesExt> ReadBytesExt for T {}
