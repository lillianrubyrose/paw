use std::io::Cursor;

use afl::fuzz;

fn main() {
	fuzz!(|data: &[u8]| {
		let _ = paw_classfile_format::ClassFile::read(&mut Cursor::new(data));
	});
}
