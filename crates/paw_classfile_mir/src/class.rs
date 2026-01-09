use eyre::Result;
use paw_classfile_lir::{attribute::LIRClassAttribute, class::LIRClass};

use crate::ty::{NamedTypeParam, Ty};

#[derive(Debug)]
pub struct MIRClass {
	pub name: String,
	pub super_class: Ty,
	pub implements_interfaces: Vec<Ty>,
	pub type_params: Vec<NamedTypeParam>,
	// fields
	// methods
}

impl MIRClass {
	pub fn from_lir(lir: LIRClass) -> Result<Self> {
		let signature = lir.attributes.iter().find_map(|attr| match attr {
			LIRClassAttribute::Signature(sig) => Some(sig),
			_ => None,
		});

		dbg!(signature);

		todo!("mirclass")
	}
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use eyre::Result;
	use paw_classfile_format::ClassFile;
	use paw_classfile_lir::class::LIRClass;

	use crate::class::MIRClass;

	#[test]
	fn load_generic_class() -> Result<()> {
		let data = include_bytes!("../../../test_data/generic/Generic$Inner.class");
		let cf = ClassFile::read(&mut Cursor::new(data))?;
		let lir = LIRClass::parse(cf)?;
		let mir = MIRClass::from_lir(lir)?;

		dbg!(mir);
		Ok(())
	}
}
