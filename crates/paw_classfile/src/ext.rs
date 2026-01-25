pub trait ReadBytesExt: byteorder::ReadBytesExt {
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

impl<T: byteorder::ReadBytesExt> ReadBytesExt for T {}
