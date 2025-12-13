use eyre::{Result, bail, eyre};
use paw_classfile_format::{AccessFlags, CPTag, ClassFile, ClassFileVersion};

use crate::{attribute::LIRAttribute, field::LIRField, method::LIRMethod};

#[derive(Debug)]
pub struct LIRClass {
	pub version: ClassFileVersion,
	pub cp: Vec<CPTag>,
	pub access_flags: AccessFlags,
	pub this_class: String,          // ClassRef
	pub super_class: Option<String>, // ClassRef
	pub interfaces: Vec<String>,     // ClassRef
	pub fields: Vec<LIRField>,
	pub methods: Vec<LIRMethod>,
	pub attributes: Vec<LIRAttribute>,
}

impl LIRClass {
	pub fn parse(raw: ClassFile) -> Self {
		let version = raw.version;
		let cp = raw.cp;
		let access_flags = raw.access_flags;
		let this_class = match cp.get(raw.this_class as usize - 1).unwrap() {
			CPTag::Class { name_index } => match cp.get(*name_index as usize - 1).unwrap() {
				CPTag::Utf8 { bytes } => paw_mutf8::decode(&bytes).unwrap(),
				_ => unreachable!(),
			},
			_ => unreachable!(),
		};
		let super_class = if raw.super_class == 0 {
			None
		} else {
			let CPTag::Class { name_index } = cp
				.get(raw.super_class as usize - 1)
				.expect("invalid super_class cp idx")
			else {
				panic!("super_class cp idx pointed to a non-class entry")
			};
			let CPTag::Utf8 { bytes } = cp
				.get(*name_index as usize - 1)
				.expect("invalid super_class name cp idx")
			else {
				panic!("super_class name cp idx pointed at non-utf8 entry")
			};

			Some(paw_mutf8::decode(&bytes).unwrap())
		};
		let interfaces = raw
			.interfaces
			.iter()
			.map(|idx| {
				let t = cp.get(*idx as usize - 1).expect("invalid interface cp idx");
				match t {
					CPTag::Class { name_index } => {
						match cp.get(*name_index as usize - 1).expect("invalid interface name cp idx") {
							CPTag::Utf8 { bytes } => paw_mutf8::decode(&bytes).unwrap().into_owned(),
							_ => panic!("interface name cp idx pointed at non-utf8 entry"),
						}
					}
					_ => panic!("interface cp idx pointed to a non-class entry"),
				}
			})
			.collect::<Vec<String>>();

		let fields = raw
			.fields
			.into_iter()
			.map(|fi| {
				let access_flags = fi.access_flags;
				let name = get_utf8_cp_entry(cp.as_slice(), fi.name_index).unwrap();
				let descriptor = get_utf8_cp_entry(cp.as_slice(), fi.descriptor_index)
					.unwrap()
					.parse()
					.expect("invalid descriptor string");
				let attributes = fi
					.attributes
					.into_iter()
					.map(|attr| LIRAttribute::parse(attr, &cp))
					.collect::<Result<Vec<_>>>()
					.unwrap();
				LIRField {
					access_flags,
					name,
					descriptor,
					attributes,
				}
			})
			.collect::<Vec<LIRField>>();
		dbg!(&fields);

		let methods = raw
			.methods
			.into_iter()
			.map(|mi| {
				let access_flags = mi.access_flags;
				dbg!(&access_flags);
				let name = get_utf8_cp_entry(cp.as_slice(), mi.name_index).unwrap();
				dbg!(&name);
				let descriptor = get_utf8_cp_entry(cp.as_slice(), mi.descriptor_index)
					.unwrap()
					.parse()
					.expect("invalid descriptor string");
				dbg!(&descriptor);
				let attributes = mi
					.attributes
					.into_iter()
					.map(|attr| LIRAttribute::parse(attr, &cp))
					.collect::<Result<Vec<_>>>()
					.unwrap();
				LIRMethod {
					access_flags,
					name,
					descriptor,
					attributes,
				}
			})
			.collect::<Vec<LIRMethod>>();
		dbg!(&methods);

		let attributes = raw
			.attributes
			.into_iter()
			.map(|attr| LIRAttribute::parse(attr, &cp))
			.collect::<Result<Vec<LIRAttribute>>>()
			.unwrap();
		dbg!(&attributes);

		todo!()
	}
}

pub fn get_utf8_cp_entry(cp: &[CPTag], idx: u16) -> Result<String> {
	let tag = cp
		.get(idx as usize - 1)
		.ok_or_else(|| eyre!("invalid cp idx {}", idx))?;
	let CPTag::Utf8 { bytes } = tag else {
		bail!("expected cp idx {} to point to utf8 tag", idx)
	};
	let s = paw_mutf8::decode(&bytes)?;
	Ok(s.into_owned())
}

#[cfg(test)]
mod t {
	use std::io::Cursor;

	use paw_classfile_format::ClassFile;

	use crate::class::LIRClass;

	#[test]
	fn parse_hello_world() -> eyre::Result<()> {
		color_eyre::install()?;
		let hello_world_class = include_bytes!("../../../test_data/HelloWorld.class");
		let mut cursor = Cursor::new(hello_world_class);
		let cf = ClassFile::read(&mut cursor).unwrap();
		let lir_cf = LIRClass::parse(cf);
		println!("{lir_cf:?}");
		Ok(())
	}
}
