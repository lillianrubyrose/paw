use eyre::Result;
use paw_classfile_format::{ClassAccessFlags, ClassFile, ClassFileVersion};

use crate::{
	attribute::{LIRClassAttribute, LIRFieldAttribute, LIRMethodAttribute},
	field::LIRField,
	method::LIRMethod,
};

#[derive(Debug)]
pub struct LIRClass {
	pub version: ClassFileVersion,
	pub access_flags: ClassAccessFlags,
	pub this_class: String,          // ClassRef
	pub super_class: Option<String>, // ClassRef
	pub interfaces: Vec<String>,     // ClassRef
	pub fields: Vec<LIRField>,
	pub methods: Vec<LIRMethod>,
	pub attributes: Vec<LIRClassAttribute>,
}

impl LIRClass {
	pub fn parse(raw: ClassFile) -> Result<Self> {
		let version = raw.version;
		let cp = raw.cp;
		let access_flags = raw.access_flags;
		let this_class = cp.resolve_class_name(cp.get_class(raw.this_class)?)?;
		let super_class = if raw.super_class != 0 {
			Some(cp.resolve_class_name(cp.get_class(raw.super_class)?)?)
		} else {
			None
		};
		let interfaces = raw
			.interfaces
			.iter()
			.map(|idx| -> Result<String> { Ok(cp.resolve_class_name(cp.get_class(*idx)?)?) })
			.collect::<Result<Vec<String>>>()?;

		let class_attrs = raw
			.attributes
			.into_iter()
			.map(|attr| LIRClassAttribute::parse(attr, &cp))
			.collect::<Result<Vec<_>>>()
			.unwrap();
		dbg!(&class_attrs);

		let fields = raw
			.fields
			.into_iter()
			.map(|fi| -> Result<LIRField> {
				let access_flags = fi.access_flags;
				let name = cp.get_utf8(fi.name_index)?;
				let descriptor = cp.get_utf8(fi.descriptor_index)?.parse()?;
				let attributes = fi
					.attributes
					.into_iter()
					.map(|attr| LIRFieldAttribute::parse(attr, &cp))
					.collect::<Result<Vec<_>>>()
					.unwrap();
				Ok(LIRField {
					access_flags,
					name,
					descriptor,
					attributes,
				})
			})
			.collect::<Result<Vec<LIRField>>>()?;
		dbg!(&fields);

		let methods = raw
			.methods
			.into_iter()
			.map(|mi| {
				let access_flags = mi.access_flags;
				let name = cp.get_utf8(mi.name_index)?;
				let descriptor = cp.get_utf8(mi.descriptor_index)?.parse()?;
				dbg!(&descriptor);
				let attributes = mi
					.attributes
					.into_iter()
					.map(|attr| LIRMethodAttribute::parse(attr, &cp, &class_attrs))
					.collect::<Result<Vec<_>>>()
					.unwrap();
				Ok(LIRMethod {
					access_flags,
					name,
					descriptor,
					attributes,
				})
			})
			.collect::<Result<Vec<LIRMethod>>>()?;
		dbg!(&methods);

		// FIXME: visit all attributes to validate
		for attr in class_attrs.iter() {
			if let LIRClassAttribute::Unknown(val) = attr {
				panic!("unknown attribute `{}` in class `{}`", val, this_class)
			}
		}

		Ok(LIRClass {
			version,
			access_flags,
			this_class,
			super_class,
			interfaces,
			fields,
			methods,
			attributes: class_attrs,
		})
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, io::Cursor};

	use paw_classfile_format::ClassFile;

	use crate::class::LIRClass;

	#[test]
	fn parse_hello_world() -> eyre::Result<()> {
		let hello_world_class = include_bytes!("../../../test_data/hello_world/HelloWorld.class");
		let mut cursor = Cursor::new(hello_world_class);
		let cf = ClassFile::read(&mut cursor).unwrap();
		let lir_cf = LIRClass::parse(cf);
		println!("{lir_cf:#?}");
		Ok(())
	}

	#[test]
	fn parse_enterprise_hello_world() -> eyre::Result<()> {
		println!("{}", std::env::current_dir().unwrap().display());
		for entry in fs::read_dir("../../test_data/enterprise_hello_world/").unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path).unwrap();
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor).unwrap();
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			};
		}
		Ok(())
	}

	#[test]
	fn parse_annos() -> eyre::Result<()> {
		println!("{}", std::env::current_dir().unwrap().display());
		for entry in fs::read_dir("../../test_data/hello_world2/").unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path).unwrap();
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor).unwrap();
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			};
		}
		Ok(())
	}

	#[test]
	fn parse_module() -> eyre::Result<()> {
		println!("{}", std::env::current_dir().unwrap().display());
		for entry in fs::read_dir("../../test_data/modules/out/test.module").unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path).unwrap();
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor).unwrap();
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			};
		}
		for entry in fs::read_dir("../../test_data/modules/out/test.module/test").unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path).unwrap();
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor).unwrap();
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			};
		}
		Ok(())
	}

	#[test]
	fn parse_indy() -> eyre::Result<()> {
		println!("{}", std::env::current_dir().unwrap().display());
		for entry in fs::read_dir("../../test_data/indy").unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path).unwrap();
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor).unwrap();
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			};
		}
		Ok(())
	}
}
