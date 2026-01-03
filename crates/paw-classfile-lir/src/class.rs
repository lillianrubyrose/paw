use eyre::Result;
use paw_classfile_format::{
	ClassAccessFlags, ClassFile, ClassFileVersion, FieldInfo, MethodInfo, class_pool::ConstantPool,
};

use crate::{
	attribute::{BootstrapMethod, BootstrapMethodsAttribute, LIRClassAttribute, LIRFieldAttribute, LIRMethodAttribute},
	field::LIRField,
	instruction::Instruction,
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
			.map(|attr| LIRClassAttribute::parse(&attr, &cp))
			.collect::<Result<Vec<_>>>()
			.unwrap();

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
					.map(|attr| LIRFieldAttribute::parse(&attr, &cp))
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

		let methods = raw
			.methods
			.into_iter()
			.map(|mi| {
				let access_flags = mi.access_flags;
				let name = cp.get_utf8(mi.name_index)?;
				let descriptor = cp.get_utf8(mi.descriptor_index)?.parse()?;
				let attributes = mi
					.attributes
					.into_iter()
					.map(|attr| LIRMethodAttribute::parse(&attr, &cp, &class_attrs))
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

		// FIXME: visit all attributes to validate
		// some attributes hold attributes themselves, we don't validate those currently
		for attr in class_attrs.iter() {
			if let LIRClassAttribute::Unknown(val) = attr {
				panic!("unknown attribute `{}` in class `{}`", val, this_class)
			}
		}

		for attr in fields.iter().flat_map(|f| &f.attributes) {
			if let LIRFieldAttribute::Unknown(val) = attr {
				panic!("unknown attribute `{}` in field `{}`", val, this_class)
			}
		}

		for attr in methods.iter().flat_map(|m| &m.attributes) {
			if let LIRMethodAttribute::Unknown(val) = attr {
				panic!("unknown attribute `{}` in method `{}`", val, this_class)
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

	pub fn to_class_file(&self) -> Result<ClassFile> {
		let mut cp = ConstantPool::new();

		let this_class = cp.add_class(self.this_class.clone());
		let super_class = self.super_class.as_ref().map_or(0, |s| cp.add_class(s.clone()));
		let interfaces = self.interfaces.iter().map(|i| cp.add_class(i.clone())).collect();

		let fields = self
			.fields
			.iter()
			.map(|f| {
				let name_index = cp.add_utf8(f.name.clone());
				let descriptor_index = cp.add_utf8(f.descriptor.jvm_repr());
				let attributes = f
					.attributes
					.iter()
					.map(|a| a.write(&mut cp))
					.collect::<Result<Vec<_>>>()?;
				Ok(FieldInfo {
					access_flags: f.access_flags,
					name_index,
					descriptor_index,
					attributes,
				})
			})
			.collect::<Result<Vec<_>>>()?;

		let mut bsm_pool: Vec<BootstrapMethod> = Vec::new();
		for method in &self.methods {
			for attr in &method.attributes {
				if let LIRMethodAttribute::Code(code) = attr {
					for (_, inst, _) in &code.code {
						let Instruction::InvokeDynamic {
							bsm_handle, bsm_args, ..
						} = inst
						else {
							continue;
						};

						if !bsm_pool
							.iter()
							.any(|b| crate::attribute::bsm_eq(b, bsm_handle, bsm_args))
						{
							bsm_pool.push(BootstrapMethod {
								method: bsm_handle.clone(),
								arguments: bsm_args.clone(),
							});
						}
					}
				}
			}
		}

		let methods = self
			.methods
			.iter()
			.map(|m| {
				let name_index = cp.add_utf8(m.name.clone());
				let descriptor_index = cp.add_utf8(m.descriptor.jvm_repr());
				let attributes = m
					.attributes
					.iter()
					.map(|a| a.write(&mut cp, &bsm_pool))
					.collect::<Result<Vec<_>>>()?;
				Ok(MethodInfo {
					access_flags: m.access_flags,
					name_index,
					descriptor_index,
					attributes,
				})
			})
			.collect::<Result<Vec<_>>>()?;

		let mut attributes = Vec::new();
		for attr in &self.attributes {
			if !matches!(attr, LIRClassAttribute::BootstrapMethods(_)) {
				attributes.push(attr.write(&mut cp)?);
			}
		}
		if !bsm_pool.is_empty() {
			let attr = LIRClassAttribute::BootstrapMethods(BootstrapMethodsAttribute { methods: bsm_pool });
			attributes.push(attr.write(&mut cp)?);
		}

		Ok(ClassFile {
			version: ClassFileVersion {
				major: self.version.major,
				minor: self.version.minor,
			},
			cp,
			access_flags: self.access_flags,
			this_class,
			super_class,
			interfaces,
			fields,
			methods,
			attributes,
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
		let cf = ClassFile::read(&mut cursor)?;
		let lir_cf = LIRClass::parse(cf);
		println!("{lir_cf:#?}");
		Ok(())
	}

	#[test]
	fn write_hello_world() -> eyre::Result<()> {
		let hello_world_class = include_bytes!("../../../test_data/hello_world/HelloWorld.class");
		let mut cursor = Cursor::new(hello_world_class);
		let cf = ClassFile::read(&mut cursor)?;
		let lir_cf = LIRClass::parse(cf)?;

		let cf = lir_cf.to_class_file()?;
		let mut cursor = Cursor::new(Vec::new());
		cf.write(&mut cursor)?;

		std::fs::write("HelloWorld.paw.class", cursor.into_inner())?;

		Ok(())
	}

	#[test]
	fn parse_enterprise_hello_world() -> eyre::Result<()> {
		println!("{}", std::env::current_dir()?.display());
		for entry in fs::read_dir("../../test_data/enterprise_hello_world/")? {
			let entry = entry?;
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path)?;
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor)?;
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			}
		}
		Ok(())
	}

	#[test]
	fn parse_annos() -> eyre::Result<()> {
		println!("{}", std::env::current_dir()?.display());
		for entry in fs::read_dir("../../test_data/hello_world2/")? {
			let entry = entry?;
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path)?;
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor)?;
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			}
		}
		Ok(())
	}

	#[test]
	fn parse_module() -> eyre::Result<()> {
		println!("{}", std::env::current_dir()?.display());
		for entry in fs::read_dir("../../test_data/modules/out/test.module")? {
			let entry = entry?;
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path)?;
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor)?;
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			}
		}
		for entry in fs::read_dir("../../test_data/modules/out/test.module/test")? {
			let entry = entry?;
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path)?;
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor)?;
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			}
		}
		Ok(())
	}

	#[test]
	fn parse_indy() -> eyre::Result<()> {
		println!("{}", std::env::current_dir()?.display());
		for entry in fs::read_dir("../../test_data/indy")? {
			let entry = entry?;
			let path = entry.path();
			if path.is_file() && path.extension().unwrap() == "class" {
				let hello_world_class = fs::read(path)?;
				let mut cursor = Cursor::new(hello_world_class);
				let cf = ClassFile::read(&mut cursor)?;
				let lir_cf = LIRClass::parse(cf);
				println!("{lir_cf:#?}");
			}
		}
		Ok(())
	}
}
