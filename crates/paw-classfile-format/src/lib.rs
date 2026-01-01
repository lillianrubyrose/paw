use bitflags::bitflags;
use byteorder::{BigEndian, WriteBytesExt};
use eyre::Result;
use num_conv::Truncate;
use thiserror::Error;

pub use crate::class_pool::CPTag;
use crate::{class_pool::ConstantPool, ext::ReadBytesExt};

pub mod class_pool;
pub mod descriptor;
pub mod ext;

pub const CLASSFILE_MAGIC: u32 = 0xCAFE_BABE;

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
	pub cp: ConstantPool,
	pub access_flags: ClassAccessFlags,
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
		let cp = ConstantPool::read(buffer)?;
		let access_flags = ClassAccessFlags::try_from(buffer.read_u16::<BigEndian>()?)?;
		let this_class = buffer.read_u16::<BigEndian>()?;
		let super_class = buffer.read_u16::<BigEndian>()?;
		let interface_count = buffer.read_u16::<BigEndian>()?;
		let interfaces = buffer.read_vec_with(usize::from(interface_count), |reader| {
			Ok(reader.read_u16::<BigEndian>()?)
		})?;

		let field_count = buffer.read_u16::<BigEndian>()?;
		let fields = buffer.read_vec_with(usize::from(field_count), |b| FieldInfo::read(b))?;
		let method_count = buffer.read_u16::<BigEndian>()?;
		let methods = buffer.read_vec_with(usize::from(method_count), |b| MethodInfo::read(b))?;
		let attribute_count = buffer.read_u16::<BigEndian>()?;
		let attributes = buffer.read_vec_with(usize::from(attribute_count), |b| AttributeInfo::read(b))?;

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

		self.cp.write(buffer)?;

		buffer.write_u16::<BigEndian>(self.access_flags.bits())?;
		buffer.write_u16::<BigEndian>(self.this_class)?;
		buffer.write_u16::<BigEndian>(self.super_class)?;

		debug_assert!(
			u16::try_from(self.interfaces.len()).is_ok(),
			"class has too many interfaces"
		);
		buffer.write_u16::<BigEndian>(self.interfaces.len().truncate())?;
		for iface in &self.interfaces {
			buffer.write_u16::<BigEndian>(*iface)?;
		}

		debug_assert!(u16::try_from(self.fields.len()).is_ok(), "class has too many fields");
		buffer.write_u16::<BigEndian>(self.fields.len().truncate())?;
		for field in &self.fields {
			field.write(buffer)?;
		}

		debug_assert!(u16::try_from(self.methods.len()).is_ok(), "class has too many methods");
		buffer.write_u16::<BigEndian>(self.methods.len().truncate())?;
		for method in &self.methods {
			method.write(buffer)?;
		}

		debug_assert!(
			u16::try_from(self.attributes.len()).is_ok(),
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
		debug_assert!(u32::try_from(self.info.len()).is_ok(), "attribute info too large");
		#[allow(clippy::cast_possible_truncation, reason = "checked above")]
		buffer.write_u32::<BigEndian>(self.info.len() as u32)?;
		buffer.write_all(&self.info)?;
		Ok(())
	}
}

macro_rules! impl_access_flags {
	($name:ident) => {
		paste::paste! {
		#[derive(Debug, Clone, Copy, Error)]
		#[error("access flags contains unknown bits: {0:?}")]
		pub struct [<Invalid $name AccessFlagsErr>]([<$name AccessFlags>]);
		}

		paste::paste! {
		impl TryFrom<u16> for [<$name AccessFlags>] {
			type Error = [<Invalid $name AccessFlagsErr>];

			fn try_from(value: u16) -> std::result::Result<Self, Self::Error> {
				let access_flags = [<$name AccessFlags>] ::from_bits_retain(value);
				if [<$name AccessFlags>]::all().contains(access_flags) {
					Ok(access_flags)
				} else {
					Err([<Invalid $name AccessFlagsErr>](access_flags))
				}
			}
		}
		}
	};
}

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.1
	// Table 4.1-B
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct ClassAccessFlags: u16 {
		const PUBLIC = 0x0001;
		const FINAL = 0x0010;
		const SUPER = 0x0020;
		const INTERFACE = 0x0200;
		const ABSTRACT = 0x0400;
		const SYNTHETIC = 0x1000;
		const ANNOTATION = 0x2000;
		const ENUM = 0x4000;
		const MODULE = 0x8000;
	}
}

impl_access_flags!(Class);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.5
	// Table 4.5-A
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct FieldAccessFlags: u16 {
		const PUBLIC = 0x0001;
		const PRIVATE = 0x0002;
		const PROTECTED = 0x0004;
		const STATIC = 0x0008;
		const FINAL = 0x0010;
		const VOLATILE = 0x0040;
		const TRANSIENT = 0x0080;
		const SYNTHETIC = 0x1000;
		const ENUM = 0x4000;
	}
}

impl_access_flags!(Field);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.6
	// Table 4.6-A
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct MethodAccessFlags: u16 {
		const PUBLIC = 0x0001;
		const PRIVATE = 0x0002;
		const PROTECTED = 0x0004;
		const STATIC = 0x0008;
		const FINAL = 0x0010;
		const SYNCHRONIZED = 0x0020;
		const BRIDGE = 0x0040;
		const VARARGS = 0x0080;
		const NATIVE = 0x0100;
		const ABSTRACT = 0x0400;
		/// In a class file whose major version number is at least 46 and at most 60: Declared strictfp.
		const STRICT = 0x0800;
		const SYNTHETIC = 0x1000;
	}
}

impl_access_flags!(Method);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.6
	// Table 4.7.6-A
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct InnerClassAccessFlags: u16 {
		const PUBLIC = 0x0001;
		const PRIVATE = 0x0002;
		const PROTECTED = 0x0004;
		const STATIC = 0x0008;
		const FINAL = 0x0010;
		const INTERFACE = 0x0200;
		const ABSTRACT = 0x0400;
		const SYNTHETIC = 0x1000;
		const ANNOTATION = 0x2000;
		const ENUM = 0x4000;
	}
}

impl_access_flags!(InnerClass);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.24
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct ParameterAccessFlags: u16 {
		const FINAL = 0x0010;
		const SYNTHETIC = 0x1000;
		const MANDATED = 0x8000;
	}
}

impl_access_flags!(Parameter);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.25
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct ModuleAccessFlags: u16 {
		const OPEN = 0x0020;
		const SYNTHETIC = 0x1000;
		const MANDATED = 0x8000;
	}
}

impl_access_flags!(Module);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.25
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct ModuleRequireAccessFlags: u16 {
		const TRANSITIVE = 0x0020;
		const STATIC_PHASE = 0x0040;
		const SYNTHETIC = 0x1000;
		const MANDATED = 0x8000;
	}
}

impl_access_flags!(ModuleRequire);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.25
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct ModuleExportAccessFlags: u16 {
		const SYNTHETIC = 0x1000;
		const MANDATED = 0x8000;
	}
}

impl_access_flags!(ModuleExport);

bitflags! {
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.25
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct ModuleOpenAccessFlags: u16 {
		const SYNTHETIC = 0x1000;
		const MANDATED = 0x8000;
	}
}

impl_access_flags!(ModuleOpen);

#[derive(Debug)]
pub struct FieldInfo {
	pub access_flags: FieldAccessFlags,
	pub name_index: u16,
	pub descriptor_index: u16,
	pub attributes: Vec<AttributeInfo>,
}

impl FieldInfo {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<FieldInfo> {
		let access_flags = FieldAccessFlags::try_from(buffer.read_u16::<BigEndian>()?)?;
		let name_index = buffer.read_u16::<BigEndian>()?;
		let descriptor_index = buffer.read_u16::<BigEndian>()?;
		let attributes_count = buffer.read_u16::<BigEndian>()?;
		let attributes = buffer.read_vec_with(usize::from(attributes_count), |b| AttributeInfo::read(b))?;

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
			u16::try_from(self.attributes.len()).is_ok(),
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
	pub access_flags: MethodAccessFlags,
	pub name_index: u16,
	pub descriptor_index: u16,
	pub attributes: Vec<AttributeInfo>,
}

impl MethodInfo {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<MethodInfo> {
		let access_flags = MethodAccessFlags::try_from(buffer.read_u16::<BigEndian>()?)?;
		let name_index = buffer.read_u16::<BigEndian>()?;
		let descriptor_index = buffer.read_u16::<BigEndian>()?;
		let attributes_count = buffer.read_u16::<BigEndian>()?;
		let attributes = buffer.read_vec_with(usize::from(attributes_count), |b| AttributeInfo::read(b))?;

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
			u16::try_from(self.attributes.len()).is_ok(),
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
		let hello_world_class = include_bytes!("../../../test_data/hello_world/HelloWorld.class");
		let mut cursor = Cursor::new(hello_world_class);
		let cf = ClassFile::read(&mut cursor)?;
		println!("{cf:?}");
		Ok(())
	}
}
