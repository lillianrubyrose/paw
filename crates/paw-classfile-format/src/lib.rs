use bitflags::bitflags;
use byteorder::{BigEndian, WriteBytesExt};
use eyre::{Result, bail};
use num_conv::Truncate;
use thiserror::Error;

use crate::ext::ReadBytesExt;

pub mod ext;

pub const CLASSFILE_MAGIC: u32 = 0xCAFEBABE;

#[derive(Debug, Error)]
pub enum ClassFileReadError {
	#[error("First 4 bytes were not 0xCAFEBABE")]
	InvalidMagic,
	#[error("Unknown class pool tag {0}")]
	UnknownClassPoolTag(u8),
	#[error(transparent)]
	IO(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ClassFileWriteError {
	#[error(transparent)]
	IO(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct ClassFile {
	pub version: ClassFileVersion,
	pub cp: Vec<CPTag>,
	pub access_flags: AccessFlags,
	pub this_class: u16,
	pub super_class: u16,
	pub interfaces: Vec<u16>,
	pub fields: Vec<FieldInfo>,
	pub methods: Vec<MethodInfo>,
	pub attributes: Vec<AttributeInfo>,
}

impl ClassFile {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<ClassFile> {
		let magic = buffer.read_u32::<BigEndian>()?;
		if magic != CLASSFILE_MAGIC {
			return Err(ClassFileReadError::InvalidMagic)?;
		}

		let minor_version = buffer.read_u16::<BigEndian>()?;
		let major_version = buffer.read_u16::<BigEndian>()?;
		let cp_count = buffer.read_u16::<BigEndian>()?;
		let mut cp = Vec::with_capacity(usize::from(cp_count) - 1);
		for _ in 0..cp_count - 1 {
			cp.push(CPTag::read(buffer)?);
		}
		let access_flags = AccessFlags::from_bits_retain(buffer.read_u16::<BigEndian>()?);
		if !AccessFlags::all().contains(access_flags) {
			bail!("access flags contain unknown bits: {:?}", access_flags);
		}
		let this_class = buffer.read_u16::<BigEndian>()?;
		let super_class = buffer.read_u16::<BigEndian>()?;
		let interface_count = buffer.read_u16::<BigEndian>()?;
		let interfaces = buffer.read_vec_with(usize::from(interface_count), |reader| {
			Ok(reader.read_u16::<BigEndian>()?)
		})?;

		let field_count = buffer.read_u16::<BigEndian>()?;
		let mut fields = Vec::with_capacity(usize::from(field_count));
		for _ in 0..field_count {
			fields.push(FieldInfo::read(buffer)?);
		}
		let method_count = buffer.read_u16::<BigEndian>()?;
		let mut methods = Vec::with_capacity(usize::from(method_count));
		for _ in 0..method_count {
			methods.push(MethodInfo::read(buffer)?);
		}
		let attribute_count = buffer.read_u16::<BigEndian>()?;
		let mut attributes = Vec::with_capacity(usize::from(attribute_count));
		for _ in 0..attribute_count {
			attributes.push(AttributeInfo::read(buffer)?);
		}

		Ok(Self {
			version: ClassFileVersion {
				major: major_version,
				minor: minor_version,
			},
			cp,
			access_flags,
			this_class,
			super_class,
			interfaces,
			fields,
			methods,
			attributes,
		})
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u32::<BigEndian>(CLASSFILE_MAGIC)?;
		buffer.write_u16::<BigEndian>(self.version.minor)?;
		buffer.write_u16::<BigEndian>(self.version.minor)?;

		debug_assert!(self.cp.len() <= u16::MAX as usize, "class has too many constants");
		buffer.write_u16::<BigEndian>(self.cp.len().truncate())?;
		for cp in &self.cp {
			cp.write(buffer)?;
		}

		buffer.write_u16::<BigEndian>(self.access_flags.bits())?;
		buffer.write_u16::<BigEndian>(self.this_class)?;
		buffer.write_u16::<BigEndian>(self.super_class)?;

		debug_assert!(
			self.interfaces.len() <= u16::MAX as usize,
			"class has too many interfaces"
		);
		buffer.write_u16::<BigEndian>(self.interfaces.len().truncate())?;
		for iface in &self.interfaces {
			buffer.write_u16::<BigEndian>(*iface)?;
		}

		debug_assert!(self.fields.len() <= u16::MAX as usize, "class has too many fields");
		buffer.write_u16::<BigEndian>(self.fields.len().truncate())?;
		for field in &self.fields {
			field.write(buffer)?;
		}

		debug_assert!(self.methods.len() <= u16::MAX as usize, "class has too many methods");
		buffer.write_u16::<BigEndian>(self.methods.len().truncate())?;
		for method in &self.methods {
			method.write(buffer)?;
		}

		debug_assert!(
			self.attributes.len() <= u16::MAX as usize,
			"class has too many attributes"
		);
		buffer.write_u16::<BigEndian>(self.attributes.len().truncate())?;
		for attr in &self.attributes {
			attr.write(buffer)?;
		}
		Ok(())
	}
}

#[derive(Debug)]
pub struct AttributeInfo {
	pub attribute_name_index: u16,
	pub info: Vec<u8>,
}

impl AttributeInfo {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<AttributeInfo> {
		let attribute_name_index = buffer.read_u16::<BigEndian>()?;
		let attribute_length = buffer.read_u32::<BigEndian>()?;
		Ok(AttributeInfo {
			attribute_name_index,
			info: buffer.read_vec_with(attribute_length as usize, |reader| Ok(reader.read_u8()?))?,
		})
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u16::<BigEndian>(self.attribute_name_index)?;
		debug_assert!(self.info.len() <= u32::MAX as usize, "attribute info too large");
		buffer.write_u32::<BigEndian>(self.info.len() as u32)?;
		buffer.write_all(&self.info)?;
		Ok(())
	}
}

bitflags! {
	// FIXME: maybe have different bitflags for different things?
	// for example private is not valid on classes
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
	pub struct AccessFlags: u16 {
		const PUBLIC = 0x001;
		const PRIVATE = 0x002;
		const PROTECTED = 0x004;
		const STATIC = 0x008;
		const FINAL = 0x010;
		const SUPER = 0x020;
		const VOLATILE = 0x040;
		const TRANSIENT = 0x080;
		const INTERFACE = 0x0200;
		const ABSTRACT = 0x0400;
		const SYNTHETIC = 0x1000;
		const ANNOTATION = 0x2000;
		const ENUM = 0x4000;
		const MODULE = 0x8000;
	}
}

#[derive(Debug)]
pub struct FieldInfo {
	pub access_flags: AccessFlags,
	pub name_index: u16,
	pub descriptor_index: u16,
	pub attributes: Vec<AttributeInfo>,
}

impl FieldInfo {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<FieldInfo> {
		let access_flags = AccessFlags::from_bits_retain(buffer.read_u16::<BigEndian>()?);
		let name_index = buffer.read_u16::<BigEndian>()?;
		let descriptor_index = buffer.read_u16::<BigEndian>()?;
		let attributes_count = buffer.read_u16::<BigEndian>()?;
		let mut attributes = Vec::with_capacity(usize::from(attributes_count));
		for _ in 0..attributes_count {
			attributes.push(AttributeInfo::read(buffer)?);
		}

		if !AccessFlags::all().contains(access_flags) {
			bail!("access flags contain unknown bits: {:?}", access_flags);
		}

		Ok(FieldInfo {
			access_flags,
			name_index,
			descriptor_index,
			attributes,
		})
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u16::<BigEndian>(self.access_flags.bits())?;
		buffer.write_u16::<BigEndian>(self.name_index)?;
		buffer.write_u16::<BigEndian>(self.descriptor_index)?;

		debug_assert!(
			self.attributes.len() <= u16::MAX as usize,
			"field {} has too many attributes",
			self.name_index
		);
		buffer.write_u16::<BigEndian>(self.attributes.len().truncate())?;
		for attr in &self.attributes {
			attr.write(buffer)?;
		}
		Ok(())
	}
}

#[derive(Debug)]
pub struct MethodInfo {
	pub access_flags: AccessFlags,
	pub name_index: u16,
	pub descriptor_index: u16,
	pub attributes: Vec<AttributeInfo>,
}

impl MethodInfo {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<MethodInfo> {
		let access_flags = AccessFlags::from_bits_retain(buffer.read_u16::<BigEndian>()?);
		let name_index = buffer.read_u16::<BigEndian>()?;
		let descriptor_index = buffer.read_u16::<BigEndian>()?;
		let attributes_count = buffer.read_u16::<BigEndian>()?;
		let mut attributes = Vec::with_capacity(usize::from(attributes_count));
		for _ in 0..attributes_count {
			attributes.push(AttributeInfo::read(buffer)?);
		}

		if !AccessFlags::all().contains(access_flags) {
			bail!("access flags contain unknown bits: {:?}", access_flags);
		}

		Ok(MethodInfo {
			access_flags,
			name_index,
			descriptor_index,
			attributes,
		})
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u16::<BigEndian>(self.access_flags.bits())?;
		buffer.write_u16::<BigEndian>(self.name_index)?;
		buffer.write_u16::<BigEndian>(self.descriptor_index)?;

		debug_assert!(
			self.attributes.len() <= u16::MAX as usize,
			"method {} has too many attributes",
			self.name_index
		);
		buffer.write_u16::<BigEndian>(self.attributes.len().truncate())?;
		for attr in &self.attributes {
			attr.write(buffer)?;
		}
		Ok(())
	}
}

#[derive(Debug)]
#[repr(C, u8)]
pub enum CPTag {
	// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.7
	Utf8 {
		bytes: Vec<u8>,
	} = 1,
	Integer(u32) = 3,
	Float(f32) = 4,
	// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.5
	// All 8-byte constants take up two entries in the constant_pool table of the class file.
	// If a CONSTANT_Long_info or CONSTANT_Double_info structure is the item in the constant_pool table-
	// at index n, then the next usable item in the pool is located at index n+2.
	// The constant_pool index n+1 must be valid but is considered unusable.
	Long(u64) = 5,
	Double(f64) = 6,
	Class {
		name_index: u16,
	} = 7,
	String {
		utf8_index: u16,
	} = 8,
	FieldRef {
		class_index: u16,
		name_and_ty_index: u16,
	} = 9,
	MethodRef {
		class_index: u16,
		name_and_ty_index: u16,
	} = 10,
	InterfaceMethodRef {
		class_index: u16,
		name_and_ty_index: u16,
	} = 11,
	NameAndType {
		name_index: u16,
		descriptor_index: u16,
	} = 12,
	// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.8
	MethodHandle {
		reference_kind: u8,
		reference_index: u16,
	} = 15,
	// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.9
	MethodType {
		descriptor_index: u16,
	} = 16,
	// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.10
	InvokeDynamic {
		bootstrap_method_attr_index: u16,
		name_and_ty_index: u16,
	} = 18,
	Module {
		name_index: u16,
	} = 19,
	Package {
		name_index: u16,
	} = 20,
}

impl CPTag {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<CPTag> {
		let tag = buffer.read_u8()?;
		match tag {
			1 => {
				let len = buffer.read_u16::<BigEndian>()?;
				let bytes = buffer.read_vec_with(usize::from(len), |reader| Ok(reader.read_u8()?))?;
				Ok(CPTag::Utf8 { bytes })
			}
			3 => Ok(CPTag::Integer(buffer.read_u32::<BigEndian>()?)),
			4 => Ok(CPTag::Float(buffer.read_f32::<BigEndian>()?)),
			5 => Ok(CPTag::Long(buffer.read_u64::<BigEndian>()?)),
			6 => Ok(CPTag::Double(buffer.read_f64::<BigEndian>()?)),
			7 => Ok(CPTag::Class {
				name_index: buffer.read_u16::<BigEndian>()?,
			}),
			8 => Ok(CPTag::String {
				utf8_index: buffer.read_u16::<BigEndian>()?,
			}),
			9 => Ok(CPTag::FieldRef {
				class_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			}),
			10 => Ok(CPTag::MethodRef {
				class_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			}),
			11 => Ok(CPTag::InterfaceMethodRef {
				class_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			}),
			12 => Ok(CPTag::NameAndType {
				name_index: buffer.read_u16::<BigEndian>()?,
				descriptor_index: buffer.read_u16::<BigEndian>()?,
			}),
			15 => Ok(CPTag::MethodHandle {
				reference_kind: buffer.read_u8()?,
				reference_index: buffer.read_u16::<BigEndian>()?,
			}),
			16 => Ok(CPTag::MethodType {
				descriptor_index: buffer.read_u16::<BigEndian>()?,
			}),
			18 => Ok(CPTag::InvokeDynamic {
				bootstrap_method_attr_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			}),
			19 => Ok(CPTag::Module {
				name_index: buffer.read_u16::<BigEndian>()?,
			}),
			20 => Ok(CPTag::Package {
				name_index: buffer.read_u16::<BigEndian>()?,
			}),
			tag => Err(ClassFileReadError::UnknownClassPoolTag(tag))?,
		}
	}

	pub fn id(&self) -> u8 {
		// SAFETY: IOCpTag is repr(C, u8), therefore the byte at offset 0 of the struct is the discriminant
		unsafe { core::ptr::from_ref(self).cast::<u8>().read() }
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u8(self.id())?;
		match self {
			CPTag::Utf8 { bytes } => {
				buffer.write_u16::<BigEndian>(bytes.len().truncate::<u16>())?;
				buffer.write_all(bytes)?;
			}
			CPTag::Integer(i) => buffer.write_u32::<BigEndian>(*i)?,
			CPTag::Float(f) => buffer.write_f32::<BigEndian>(*f)?,
			CPTag::Long(l) => buffer.write_u64::<BigEndian>(*l)?,
			CPTag::Double(d) => buffer.write_f64::<BigEndian>(*d)?,
			CPTag::Class { name_index } => buffer.write_u16::<BigEndian>(*name_index)?,
			CPTag::String { utf8_index } => buffer.write_u16::<BigEndian>(*utf8_index)?,
			CPTag::FieldRef {
				class_index,
				name_and_ty_index,
			} => {
				buffer.write_u16::<BigEndian>(*class_index)?;
				buffer.write_u16::<BigEndian>(*name_and_ty_index)?;
			}
			CPTag::MethodRef {
				class_index,
				name_and_ty_index,
			} => {
				buffer.write_u16::<BigEndian>(*class_index)?;
				buffer.write_u16::<BigEndian>(*name_and_ty_index)?;
			}
			CPTag::InterfaceMethodRef {
				class_index,
				name_and_ty_index,
			} => {
				buffer.write_u16::<BigEndian>(*class_index)?;
				buffer.write_u16::<BigEndian>(*name_and_ty_index)?;
			}
			CPTag::NameAndType {
				name_index,
				descriptor_index,
			} => {
				buffer.write_u16::<BigEndian>(*name_index)?;
				buffer.write_u16::<BigEndian>(*descriptor_index)?;
			}
			CPTag::MethodHandle {
				reference_kind,
				reference_index,
			} => {
				buffer.write_u8(*reference_kind)?;
				buffer.write_u16::<BigEndian>(*reference_index)?;
			}
			CPTag::MethodType { descriptor_index } => {
				buffer.write_u16::<BigEndian>(*descriptor_index)?;
			}
			CPTag::InvokeDynamic {
				bootstrap_method_attr_index,
				name_and_ty_index: name_and_type_index,
			} => {
				buffer.write_u16::<BigEndian>(*bootstrap_method_attr_index)?;
				buffer.write_u16::<BigEndian>(*name_and_type_index)?;
			}
			CPTag::Module { name_index } => {
				buffer.write_u16::<BigEndian>(*name_index)?;
			}
			CPTag::Package { name_index } => {
				buffer.write_u16::<BigEndian>(*name_index)?;
			}
		}
		Ok(())
	}
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassFileVersion {
	pub major: u16,
	pub minor: u16,
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use crate::ClassFile;

	#[test]
	fn reads_hello_world() -> eyre::Result<()> {
		color_eyre::install()?;
		let hello_world_class = include_bytes!("../../../test_data/HelloWorld.class");
		let mut cursor = Cursor::new(hello_world_class);
		let cf = ClassFile::read(&mut cursor)?;
		println!("{cf:?}");
		Ok(())
	}
}
