use std::io::Cursor;

use afl::fuzz;

fn main() {
	fuzz!(|data: &[u8]| {
		match paw_classfile::ClassFile::read(&mut Cursor::new(data)) {
			Ok(cf) => match paw_lir::class::LIRClass::parse(cf) {
				Ok(cf) => {}
				Err(err) => {}
			},
			Err(err) => {}
		}
	});
}
