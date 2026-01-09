use std::io::Cursor;

use afl::fuzz;

fn main() {
	fuzz!(|data: &[u8]| {
		match paw_classfile_format::ClassFile::read(&mut Cursor::new(data)) {
			Ok(cf) => match paw_classfile_lir::class::LIRClass::parse(cf) {
				Ok(cf) => {}
				Err(err) => {}
			},
			Err(err) => {}
		}
	});
}
