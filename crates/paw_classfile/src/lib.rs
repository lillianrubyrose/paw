use std::str::FromStr;

use bitflags::bitflags;
use eyre::{Result, bail};
use num_conv::Truncate;
use thiserror::Error;

pub use crate::constant_pool::{CPTag, MethodHandleRefKind, MethodHandleRefKindFromIntErr};
use crate::{
	attributes::class::{BootstrapMethod, BootstrapMethodArgument},
	constant_pool::{
		ConstantPool, ConstantPoolIndex, FieldRefTag, InterfaceMethodRefTag, MethodHandleTag, MethodRefTag,
	},
	descriptor::{Descriptor, MethodDescriptor},
	ext::{BytesReadExt, BytesWriteExt},
};

pub mod attributes;
pub mod constant_pool;
pub mod descriptor;
pub mod ext;
pub mod instruction;

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
	pub fn read<B: BytesReadExt>(buffer: &mut B) -> Result<ClassFile> {
		let magic = buffer.read_u32()?;
		if magic != CLASSFILE_MAGIC {
			return Err(ClassFileReadError::InvalidMagic)?;
		}

		let minor_version = buffer.read_u16()?;
		let major_version = buffer.read_u16()?;
		let cp = ConstantPool::read(buffer)?;
		let access_flags = ClassAccessFlags::try_from(buffer.read_u16()?)?;
		let this_class = buffer.read_u16()?;
		let super_class = buffer.read_u16()?;
		let interface_count = buffer.read_u16()?;
		let interfaces = buffer.read_vec_with(usize::from(interface_count), |reader| Ok(reader.read_u16()?))?;

		let field_count = buffer.read_u16()?;
		let fields = buffer.read_vec_with(usize::from(field_count), |b| FieldInfo::read(b))?;
		let method_count = buffer.read_u16()?;
		let methods = buffer.read_vec_with(usize::from(method_count), |b| MethodInfo::read(b))?;
		let attribute_count = buffer.read_u16()?;
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

	pub fn write<B: BytesWriteExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u32(CLASSFILE_MAGIC)?;
		buffer.write_u16(self.version.minor)?;
		buffer.write_u16(self.version.major)?;

		self.cp.write(buffer)?;

		buffer.write_u16(self.access_flags.bits())?;
		buffer.write_u16(self.this_class)?;
		buffer.write_u16(self.super_class)?;

		debug_assert!(
			u16::try_from(self.interfaces.len()).is_ok(),
			"class has too many interfaces"
		);
		buffer.write_u16(self.interfaces.len().truncate())?;
		for iface in &self.interfaces {
			buffer.write_u16(*iface)?;
		}

		debug_assert!(u16::try_from(self.fields.len()).is_ok(), "class has too many fields");
		buffer.write_u16(self.fields.len().truncate())?;
		for field in &self.fields {
			field.write(buffer)?;
		}

		debug_assert!(u16::try_from(self.methods.len()).is_ok(), "class has too many methods");
		buffer.write_u16(self.methods.len().truncate())?;
		for method in &self.methods {
			method.write(buffer)?;
		}

		debug_assert!(
			u16::try_from(self.attributes.len()).is_ok(),
			"class has too many attributes"
		);
		buffer.write_u16(self.attributes.len().truncate())?;
		for attr in &self.attributes {
			attr.write(buffer)?;
		}
		Ok(())
	}
}

#[derive(Debug)]
pub struct AttributeInfo {
	pub attribute_name_index: ConstantPoolIndex,
	pub info: Vec<u8>,
}

impl AttributeInfo {
	pub fn read<B: BytesReadExt>(buffer: &mut B) -> Result<AttributeInfo> {
		let attribute_name_index = ConstantPoolIndex::new_internal(buffer.read_u16()?);
		let attribute_length = buffer.read_u32()?;
		Ok(AttributeInfo {
			attribute_name_index,
			info: buffer.read_vec_with(attribute_length as usize, |reader| Ok(reader.read_u8()?))?,
		})
	}

	pub fn write<B: BytesWriteExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u16(self.attribute_name_index.get())?;
		debug_assert!(u32::try_from(self.info.len()).is_ok(), "attribute info too large");
		#[allow(clippy::cast_possible_truncation, reason = "checked above")]
		buffer.write_u32(self.info.len() as u32)?;
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
	pub fn read<B: BytesReadExt>(buffer: &mut B) -> Result<FieldInfo> {
		let access_flags = FieldAccessFlags::try_from(buffer.read_u16()?)?;
		let name_index = buffer.read_u16()?;
		let descriptor_index = buffer.read_u16()?;
		let attributes_count = buffer.read_u16()?;
		let attributes = buffer.read_vec_with(usize::from(attributes_count), |b| AttributeInfo::read(b))?;

		Ok(FieldInfo {
			access_flags,
			name_index,
			descriptor_index,
			attributes,
		})
	}

	pub fn write<B: BytesWriteExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u16(self.access_flags.bits())?;
		buffer.write_u16(self.name_index)?;
		buffer.write_u16(self.descriptor_index)?;

		debug_assert!(
			u16::try_from(self.attributes.len()).is_ok(),
			"field {} has too many attributes",
			self.name_index
		);
		buffer.write_u16(self.attributes.len().truncate())?;
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
	pub fn read<B: BytesReadExt>(buffer: &mut B) -> Result<MethodInfo> {
		let access_flags = MethodAccessFlags::try_from(buffer.read_u16()?)?;
		let name_index = buffer.read_u16()?;
		let descriptor_index = buffer.read_u16()?;
		let attributes_count = buffer.read_u16()?;
		let attributes = buffer.read_vec_with(usize::from(attributes_count), |b| AttributeInfo::read(b))?;

		Ok(MethodInfo {
			access_flags,
			name_index,
			descriptor_index,
			attributes,
		})
	}

	pub fn write<B: BytesWriteExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u16(self.access_flags.bits())?;
		buffer.write_u16(self.name_index)?;
		buffer.write_u16(self.descriptor_index)?;

		debug_assert!(
			u16::try_from(self.attributes.len()).is_ok(),
			"method {} has too many attributes",
			self.name_index
		);
		buffer.write_u16(self.attributes.len().truncate())?;
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

#[derive(Debug, Clone, PartialEq)]
pub enum MethodHandleDescriptor {
	Field(Descriptor),
	Method(MethodDescriptor),
}

// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.8
#[derive(Debug, Clone, PartialEq)]
pub struct MethodHandle {
	pub kind: MethodHandleRefKind,
	pub owner: String,
	pub name: String,
	pub descriptor: MethodHandleDescriptor,
	pub is_interface: bool,
}

impl MethodHandle {
	pub fn resolve(cp: &ConstantPool, index: ConstantPoolIndex) -> eyre::Result<Self> {
		let MethodHandleTag {
			reference_kind: kind,
			reference_index,
		} = cp.get_method_handle(index)?;

		let reference_tag = cp.get_tag(*reference_index)?;
		let (class_index, name_and_ty_index, is_interface) = match reference_tag {
			CPTag::FieldRef(FieldRefTag {
				class_index,
				name_and_ty_index,
			})
			| CPTag::MethodRef(MethodRefTag {
				class_index,
				name_and_ty_index,
			}) => (*class_index, *name_and_ty_index, false),
			CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
				class_index,
				name_and_ty_index,
			}) => (*class_index, *name_and_ty_index, true),
			tag => bail!("invalid reference tag for method handle {tag:?}"),
		};

		let owner = cp.get_class(class_index)?;
		let owner = cp.resolve_class_name(owner)?;

		let nat = cp.get_name_and_type(name_and_ty_index)?;
		let name = cp.get_utf8(nat.name_index)?;
		let descriptor = cp.get_utf8(nat.descriptor_index)?;
		let descriptor = if kind.is_field() {
			MethodHandleDescriptor::Field(Descriptor::from_str(&descriptor)?)
		} else {
			MethodHandleDescriptor::Method(MethodDescriptor::from_str(&descriptor)?)
		};

		Ok(Self {
			kind: *kind,
			owner,
			name,
			descriptor,
			is_interface,
		})
	}
}

#[must_use]
pub fn bsm_eq(bsm: &BootstrapMethod, handle: &MethodHandle, args: &[BootstrapMethodArgument]) -> bool {
	if &bsm.method != handle {
		return false;
	}
	if bsm.arguments.len() != args.len() {
		return false;
	}
	for (a, b) in bsm.arguments.iter().zip(args.iter()) {
		let match_arg = match (a, b) {
			(BootstrapMethodArgument::Int(x), BootstrapMethodArgument::Int(y)) => x == y,
			(BootstrapMethodArgument::Long(x), BootstrapMethodArgument::Long(y)) => x == y,
			(BootstrapMethodArgument::Float(x), BootstrapMethodArgument::Float(y)) => x.to_bits() == y.to_bits(),
			(BootstrapMethodArgument::Double(x), BootstrapMethodArgument::Double(y)) => x.to_bits() == y.to_bits(),
			(BootstrapMethodArgument::String(x), BootstrapMethodArgument::String(y))
			| (BootstrapMethodArgument::Class(x), BootstrapMethodArgument::Class(y)) => x == y,
			(BootstrapMethodArgument::MethodHandle(x), BootstrapMethodArgument::MethodHandle(y)) => x == y,
			(BootstrapMethodArgument::MethodType(x), BootstrapMethodArgument::MethodType(y)) => x == y,
			_ => false,
		};
		if !match_arg {
			return false;
		}
	}
	true
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
