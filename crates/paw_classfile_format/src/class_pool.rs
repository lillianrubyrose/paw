use core::fmt;

use byteorder::{BigEndian, WriteBytesExt};
use eyre::Result;
use num_conv::Truncate;
use thiserror::Error;

use crate::{
	ClassFileReadError,
	descriptor::{Descriptor, DescriptorParseErr, MethodDescriptor},
	ext::ReadBytesExt,
};

pub enum CPEntry {
	Tag(CPTag),
	Padding,
}

pub struct ConstantPool {
	tags: Vec<CPEntry>,
}

// FIXME: newtype this for actual type safety
pub type ConstantPoolIndex = u16;

#[derive(Debug, Clone, Error)]
pub enum ConstantPoolIndexErr {
	#[error("index {0} out of range for constant pool of size {1}")]
	IndexOutOfRange(ConstantPoolIndex, usize),
	#[error("expected tag of kind {1} at index {0}, found {2} instead")]
	InvalidType(ConstantPoolIndex, &'static str, &'static str),
	#[error("index {0} is a padding entry for a preceding wide entry")]
	PaddingEntry(ConstantPoolIndex),
	#[error(transparent)]
	InvalidDescriptor(#[from] DescriptorParseErr),
}

impl ConstantPool {
	#[must_use]
	pub const fn new() -> Self {
		Self { tags: Vec::new() }
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.tags.len()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn get_tag(&self, idx: ConstantPoolIndex) -> Result<&CPTag, ConstantPoolIndexErr> {
		let checked_idx_sub = (idx as usize)
			.checked_sub(1)
			.ok_or(ConstantPoolIndexErr::IndexOutOfRange(idx, self.tags.len()))?;
		match self
			.tags
			.get(checked_idx_sub)
			.ok_or(ConstantPoolIndexErr::IndexOutOfRange(idx, self.tags.len()))?
		{
			CPEntry::Tag(tag) => Ok(tag),
			CPEntry::Padding => Err(ConstantPoolIndexErr::PaddingEntry(idx)),
		}
	}

	pub fn get_utf8(&self, idx: ConstantPoolIndex) -> Result<String, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Utf8(tag) => Ok(tag.value.clone()),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Utf8", tag.name())),
		}
	}

	pub fn get_integer(&self, idx: ConstantPoolIndex) -> Result<u32, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Integer(tag) => Ok(*tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Int", tag.name())),
		}
	}

	pub fn get_float(&self, idx: ConstantPoolIndex) -> Result<f32, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Float(tag) => Ok(*tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Float", tag.name())),
		}
	}

	pub fn get_long(&self, idx: ConstantPoolIndex) -> Result<u64, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Long(tag) => Ok(*tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Long", tag.name())),
		}
	}

	pub fn get_double(&self, idx: ConstantPoolIndex) -> Result<f64, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Double(tag) => Ok(*tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Double", tag.name())),
		}
	}

	pub fn get_class(&self, idx: ConstantPoolIndex) -> Result<&ClassTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Class(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Class", tag.name())),
		}
	}

	pub fn resolve_class_name(&self, tag: &ClassTag) -> Result<String, ConstantPoolIndexErr> {
		let name = self.get_utf8(tag.name_index)?;
		Ok(name)
	}

	pub fn get_string(&self, idx: ConstantPoolIndex) -> Result<&StringTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::String(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "String", tag.name())),
		}
	}

	pub fn resolve_string(&self, tag: &StringTag) -> Result<String, ConstantPoolIndexErr> {
		self.get_utf8(tag.utf8_index)
	}

	pub fn get_field_ref(&self, idx: ConstantPoolIndex) -> Result<&FieldRefTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::FieldRef(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "FieldRef", tag.name())),
		}
	}

	pub fn get_method_ref(&self, idx: ConstantPoolIndex) -> Result<&MethodRefTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::MethodRef(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "MethodRef", tag.name())),
		}
	}

	pub fn get_interface_method_ref(
		&self,
		idx: ConstantPoolIndex,
	) -> Result<&InterfaceMethodRefTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::InterfaceMethodRef(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "InterfaceMethodRef", tag.name())),
		}
	}

	pub fn get_name_and_type(&self, idx: ConstantPoolIndex) -> Result<&NameAndTypeTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::NameAndType(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "NameAndType", tag.name())),
		}
	}

	pub fn resolve_field_name_and_type(
		&self,
		name_and_type: &NameAndTypeTag,
	) -> Result<(String, Descriptor), ConstantPoolIndexErr> {
		let name = self.get_utf8(name_and_type.name_index)?;
		let descriptor = self.get_utf8(name_and_type.descriptor_index)?;
		Ok((name, descriptor.parse()?))
	}

	pub fn resolve_method_name_and_type(
		&self,
		name_and_type: &NameAndTypeTag,
	) -> Result<(String, MethodDescriptor), ConstantPoolIndexErr> {
		let name = self.get_utf8(name_and_type.name_index)?;
		let descriptor = self.get_utf8(name_and_type.descriptor_index)?;
		Ok((name, descriptor.parse()?))
	}

	pub fn get_method_handle(&self, idx: ConstantPoolIndex) -> Result<&MethodHandleTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::MethodHandle(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "MethodHandle", tag.name())),
		}
	}

	pub fn get_method_type(&self, idx: ConstantPoolIndex) -> Result<&MethodTypeTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::MethodType(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "MethodType", tag.name())),
		}
	}

	pub fn get_invoke_dynamic(&self, idx: ConstantPoolIndex) -> Result<&InvokeDynamicTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::InvokeDynamic(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "InvokeDynamic", tag.name())),
		}
	}

	pub fn get_module(&self, idx: ConstantPoolIndex) -> Result<&ModuleTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Module(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Module", tag.name())),
		}
	}

	pub fn resolve_module_name(&self, tag: &ModuleTag) -> Result<String, ConstantPoolIndexErr> {
		let name = self.get_utf8(tag.name_index)?;
		Ok(name)
	}

	pub fn get_package(&self, idx: ConstantPoolIndex) -> Result<&PackageTag, ConstantPoolIndexErr> {
		let tag = self.get_tag(idx)?;
		match tag {
			CPTag::Package(tag) => Ok(tag),
			tag => Err(ConstantPoolIndexErr::InvalidType(idx, "Package", tag.name())),
		}
	}

	pub fn resolve_package_name(&self, tag: &PackageTag) -> Result<String, ConstantPoolIndexErr> {
		let name = self.get_utf8(tag.name_index)?;
		Ok(name)
	}

	pub fn push(&mut self, tag: CPTag) -> u16 {
		self.tags.push(CPEntry::Tag(tag));
		self.len().truncate()
	}

	pub fn add_utf8(&mut self, val: String) -> u16 {
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Utf8(t)) if t.value == val))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::Utf8(Utf8Tag { value: val }))
	}

	pub fn add_class(&mut self, name: String) -> u16 {
		let name_index = self.add_utf8(name);
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Class(t)) if t.name_index == name_index))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::Class(ClassTag { name_index }))
	}

	pub fn add_string(&mut self, val: String) -> u16 {
		let utf8_index = self.add_utf8(val);
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::String(t)) if t.utf8_index == utf8_index))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::String(StringTag { utf8_index }))
	}

	pub fn add_integer(&mut self, val: u32) -> u16 {
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Integer(v)) if *v == val))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::Integer(val))
	}

	pub fn add_float(&mut self, val: f32) -> u16 {
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Float(v)) if v.eq(&val)))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::Float(val))
	}

	pub fn add_long(&mut self, val: u64) -> u16 {
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Long(v)) if *v == val))
		{
			return (idx + 1).truncate();
		}
		let idx = self.push(CPTag::Long(val));
		self.tags.push(CPEntry::Padding);
		idx
	}

	pub fn add_double(&mut self, val: f64) -> u16 {
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Double(v)) if v.eq(&val)))
		{
			return (idx + 1).truncate();
		}
		let idx = self.push(CPTag::Double(val));
		self.tags.push(CPEntry::Padding);
		idx
	}

	pub fn add_name_and_type(&mut self, name: String, descriptor: String) -> u16 {
		let name_index = self.add_utf8(name);
		let descriptor_index = self.add_utf8(descriptor);
		if let Some(idx) = self.tags.iter().position(
			|t| matches!(t, CPEntry::Tag(CPTag::NameAndType(t)) if t.name_index == name_index && t.descriptor_index == descriptor_index),
		) {
			return (idx + 1).truncate();
		}
		self.push(CPTag::NameAndType(NameAndTypeTag {
			name_index,
			descriptor_index,
		}))
	}

	pub fn add_field_ref(&mut self, class: String, name: String, descriptor: String) -> u16 {
		let class_index = self.add_class(class);
		let name_and_ty_index = self.add_name_and_type(name, descriptor);
		if let Some(idx) = self.tags.iter().position(
			|t| matches!(t, CPEntry::Tag(CPTag::FieldRef(t)) if t.class_index == class_index && t.name_and_ty_index == name_and_ty_index),
		) {
			return (idx + 1).truncate();
		}
		self.push(CPTag::FieldRef(FieldRefTag {
			class_index,
			name_and_ty_index,
		}))
	}

	pub fn add_method_ref(&mut self, class: String, name: String, descriptor: String) -> u16 {
		let class_index = self.add_class(class);
		let name_and_ty_index = self.add_name_and_type(name, descriptor);
		if let Some(idx) = self.tags.iter().position(
			|t| matches!(t, CPEntry::Tag(CPTag::MethodRef(t)) if t.class_index == class_index && t.name_and_ty_index == name_and_ty_index),
		) {
			return (idx + 1).truncate();
		}
		self.push(CPTag::MethodRef(MethodRefTag {
			class_index,
			name_and_ty_index,
		}))
	}

	pub fn add_interface_method_ref(&mut self, class: String, name: String, descriptor: String) -> u16 {
		let class_index = self.add_class(class);
		let name_and_ty_index = self.add_name_and_type(name, descriptor);
		if let Some(idx) = self.tags.iter().position(|t| matches!(t, CPEntry::Tag(CPTag::InterfaceMethodRef(t)) if t.class_index == class_index && t.name_and_ty_index == name_and_ty_index)) {
			return (idx + 1).truncate();
		}
		self.push(CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
			class_index,
			name_and_ty_index,
		}))
	}

	pub fn add_method_handle(&mut self, kind: u8, reference_index: u16) -> u16 {
		if let Some(idx) = self.tags.iter().position(
			|t| matches!(t, CPEntry::Tag(CPTag::MethodHandle(t)) if t.reference_kind == kind && t.reference_index == reference_index),
		) {
			return (idx + 1).truncate();
		}
		self.push(CPTag::MethodHandle(MethodHandleTag {
			reference_kind: kind,
			reference_index,
		}))
	}

	pub fn add_method_type(&mut self, descriptor: String) -> u16 {
		let descriptor_index = self.add_utf8(descriptor);
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::MethodType(t)) if t.descriptor_index == descriptor_index))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::MethodType(MethodTypeTag { descriptor_index }))
	}

	pub fn add_invoke_dynamic(&mut self, bsm_attr_idx: u16, name: String, descriptor: String) -> u16 {
		let name_and_ty_idx = self.add_name_and_type(name, descriptor);
		if let Some(idx) = self.tags.iter().position(|t| {
			matches!(t, CPEntry::Tag(CPTag::InvokeDynamic(t))
                if t.bootstrap_method_attr_index == bsm_attr_idx
                && t.name_and_ty_index == name_and_ty_idx)
		}) {
			return (idx + 1).truncate();
		}
		self.push(CPTag::InvokeDynamic(InvokeDynamicTag {
			bootstrap_method_attr_index: bsm_attr_idx,
			name_and_ty_index: name_and_ty_idx,
		}))
	}

	pub fn add_module(&mut self, name: String) -> u16 {
		let name_index = self.add_utf8(name);
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Module(t)) if t.name_index == name_index))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::Module(ModuleTag { name_index }))
	}

	pub fn add_package(&mut self, name: String) -> u16 {
		let name_index = self.add_utf8(name);
		if let Some(idx) = self
			.tags
			.iter()
			.position(|t| matches!(t, CPEntry::Tag(CPTag::Package(t)) if t.name_index == name_index))
		{
			return (idx + 1).truncate();
		}
		self.push(CPTag::Package(PackageTag { name_index }))
	}
}

impl Default for ConstantPool {
	fn default() -> Self {
		Self::new()
	}
}

// Read/Write
impl ConstantPool {
	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<Self> {
		let cp_count = buffer.read_u16::<BigEndian>()?;
		if cp_count == 0 {
			return Ok(Self::new());
		}

		let mut tags = Vec::with_capacity((cp_count - 1) as usize);
		let mut i = 1;
		while i < cp_count {
			let tag = CPTag::read(buffer)?;
			let is_wide = matches!(tag, CPTag::Long(_) | CPTag::Double(_));
			tags.push(CPEntry::Tag(tag));
			i += 1;

			if is_wide {
				tags.push(CPEntry::Padding);
				i += 1;
			}
		}

		Ok(Self { tags })
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		let len = self.tags.len() + 1;
		debug_assert!(u16::try_from(len).is_ok(), "class has too many constants");
		buffer.write_u16::<BigEndian>(len.truncate())?;
		for entry in self.tags.iter() {
			match entry {
				CPEntry::Tag(tag) => tag.write(buffer)?,
				CPEntry::Padding => {}
			}
		}
		Ok(())
	}
}

impl fmt::Debug for ConstantPool {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "ConstantPool(<{} tags>)", self.tags.len())
	}
}

#[derive(Debug, Clone)]
// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.7
pub struct Utf8Tag {
	pub value: String,
}

#[derive(Debug, Clone)]
pub struct ClassTag {
	pub name_index: u16, // StringTag
}

#[derive(Debug, Clone)]
pub struct StringTag {
	pub utf8_index: u16, // Utf8Tag
}

#[derive(Debug, Clone)]
pub struct FieldRefTag {
	pub class_index: u16,       // ClassTag
	pub name_and_ty_index: u16, // NameAndTypeTag
}

#[derive(Debug, Clone)]
pub struct MethodRefTag {
	pub class_index: u16,       // ClassTag
	pub name_and_ty_index: u16, // NameAndTypeTag
}

#[derive(Debug, Clone)]
pub struct InterfaceMethodRefTag {
	pub class_index: u16,       // ClassTag
	pub name_and_ty_index: u16, // NameAndTypeTag
}

#[derive(Debug, Clone)]
pub struct NameAndTypeTag {
	pub name_index: u16,       // StringTag
	pub descriptor_index: u16, // StringTag
}

#[derive(Debug, Clone)]
/// <https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.8>
pub struct MethodHandleTag {
	pub reference_kind: u8,   // FIXME: types?
	pub reference_index: u16, // FIXME: what does this point to
}

#[derive(Debug, Clone)]
/// <https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.9>
pub struct MethodTypeTag {
	pub descriptor_index: u16, // StringTag
}

#[derive(Debug, Clone)]
/// <https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.10>
pub struct InvokeDynamicTag {
	pub bootstrap_method_attr_index: u16,
	pub name_and_ty_index: u16, // NameAndTypeTag
}

#[derive(Debug, Clone)]
pub struct ModuleTag {
	pub name_index: u16, // StringTag
}

#[derive(Debug, Clone)]
pub struct PackageTag {
	pub name_index: u16, // StringTag
}

#[derive(Debug, Clone)]
#[repr(C, u8)]
pub enum CPTag {
	Utf8(Utf8Tag) = 1,
	Integer(u32) = 3,
	Float(f32) = 4,
	// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.5
	// All 8-byte constants take up two entries in the constant_pool table of the class file.
	// If a CONSTANT_Long_info or CONSTANT_Double_info structure is the item in the constant_pool table-
	// at index n, then the next usable item in the pool is located at index n+2.
	// The constant_pool index n+1 must be valid but is considered unusable.
	Long(u64) = 5,
	Double(f64) = 6,
	Class(ClassTag) = 7,
	String(StringTag) = 8,
	FieldRef(FieldRefTag) = 9,
	MethodRef(MethodRefTag) = 10,
	InterfaceMethodRef(InterfaceMethodRefTag) = 11,
	NameAndType(NameAndTypeTag) = 12,
	MethodHandle(MethodHandleTag) = 15,
	MethodType(MethodTypeTag) = 16,
	InvokeDynamic(InvokeDynamicTag) = 18,
	Module(ModuleTag) = 19,
	Package(PackageTag) = 20,
}

impl CPTag {
	#[must_use]
	pub const fn name(&self) -> &'static str {
		match self {
			CPTag::Utf8(_) => "Utf8",
			CPTag::Integer(_) => "Integer",
			CPTag::Float(_) => "Float",
			CPTag::Long(_) => "Long",
			CPTag::Double(_) => "Double",
			CPTag::Class(_) => "Class",
			CPTag::String(_) => "String",
			CPTag::FieldRef(_) => "FieldRef",
			CPTag::MethodRef(_) => "MethodRef",
			CPTag::InterfaceMethodRef(_) => "InterfaceMethodRef",
			CPTag::NameAndType(_) => "NameAndType",
			CPTag::MethodHandle(_) => "MethodHandle",
			CPTag::MethodType(_) => "MethodType",
			CPTag::InvokeDynamic(_) => "InvokeDynamic",
			CPTag::Module(_) => "Module",
			CPTag::Package(_) => "Package",
		}
	}

	pub fn read<B: ReadBytesExt>(buffer: &mut B) -> Result<CPTag> {
		let tag = buffer.read_u8()?;
		match tag {
			1 => {
				let len = buffer.read_u16::<BigEndian>()?;
				let bytes = buffer.read_vec_with(usize::from(len), |reader| Ok(reader.read_u8()?))?;
				Ok(CPTag::Utf8(Utf8Tag {
					value: paw_mutf8::decode(&bytes)?.into_owned(),
				}))
			}
			3 => Ok(CPTag::Integer(buffer.read_u32::<BigEndian>()?)),
			4 => Ok(CPTag::Float(buffer.read_f32::<BigEndian>()?)),
			5 => Ok(CPTag::Long(buffer.read_u64::<BigEndian>()?)),
			6 => Ok(CPTag::Double(buffer.read_f64::<BigEndian>()?)),
			7 => Ok(CPTag::Class(ClassTag {
				name_index: buffer.read_u16::<BigEndian>()?,
			})),
			8 => Ok(CPTag::String(StringTag {
				utf8_index: buffer.read_u16::<BigEndian>()?,
			})),
			9 => Ok(CPTag::FieldRef(FieldRefTag {
				class_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			})),
			10 => Ok(CPTag::MethodRef(MethodRefTag {
				class_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			})),
			11 => Ok(CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
				class_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			})),
			12 => Ok(CPTag::NameAndType(NameAndTypeTag {
				name_index: buffer.read_u16::<BigEndian>()?,
				descriptor_index: buffer.read_u16::<BigEndian>()?,
			})),
			15 => Ok(CPTag::MethodHandle(MethodHandleTag {
				reference_kind: buffer.read_u8()?,
				reference_index: buffer.read_u16::<BigEndian>()?,
			})),
			16 => Ok(CPTag::MethodType(MethodTypeTag {
				descriptor_index: buffer.read_u16::<BigEndian>()?,
			})),
			18 => Ok(CPTag::InvokeDynamic(InvokeDynamicTag {
				bootstrap_method_attr_index: buffer.read_u16::<BigEndian>()?,
				name_and_ty_index: buffer.read_u16::<BigEndian>()?,
			})),
			19 => Ok(CPTag::Module(ModuleTag {
				name_index: buffer.read_u16::<BigEndian>()?,
			})),
			20 => Ok(CPTag::Package(PackageTag {
				name_index: buffer.read_u16::<BigEndian>()?,
			})),
			tag => Err(ClassFileReadError::UnknownClassPoolTag(tag))?,
		}
	}

	#[must_use]
	pub fn id(&self) -> u8 {
		// SAFETY: IOCpTag is repr(C, u8), therefore the byte at offset 0 of the struct is the discriminant
		unsafe { core::ptr::from_ref(self).cast::<u8>().read() }
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		buffer.write_u8(self.id())?;
		match self {
			CPTag::Utf8(Utf8Tag { value }) => {
				let bytes = paw_mutf8::encode(value);
				buffer.write_u16::<BigEndian>(bytes.len().truncate::<u16>())?;
				buffer.write_all(&bytes)?;
			}
			CPTag::Integer(i) => buffer.write_u32::<BigEndian>(*i)?,
			CPTag::Float(f) => buffer.write_f32::<BigEndian>(*f)?,
			CPTag::Long(l) => buffer.write_u64::<BigEndian>(*l)?,
			CPTag::Double(d) => buffer.write_f64::<BigEndian>(*d)?,
			CPTag::Class(ClassTag { name_index }) => buffer.write_u16::<BigEndian>(*name_index)?,
			CPTag::String(StringTag { utf8_index }) => buffer.write_u16::<BigEndian>(*utf8_index)?,
			CPTag::FieldRef(FieldRefTag {
				class_index,
				name_and_ty_index,
			})
			| CPTag::MethodRef(MethodRefTag {
				class_index,
				name_and_ty_index,
			})
			| CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
				class_index,
				name_and_ty_index,
			}) => {
				buffer.write_u16::<BigEndian>(*class_index)?;
				buffer.write_u16::<BigEndian>(*name_and_ty_index)?;
			}
			CPTag::NameAndType(NameAndTypeTag {
				name_index,
				descriptor_index,
			}) => {
				buffer.write_u16::<BigEndian>(*name_index)?;
				buffer.write_u16::<BigEndian>(*descriptor_index)?;
			}
			CPTag::MethodHandle(MethodHandleTag {
				reference_kind,
				reference_index,
			}) => {
				buffer.write_u8(*reference_kind)?;
				buffer.write_u16::<BigEndian>(*reference_index)?;
			}
			CPTag::MethodType(MethodTypeTag { descriptor_index }) => {
				buffer.write_u16::<BigEndian>(*descriptor_index)?;
			}
			CPTag::InvokeDynamic(InvokeDynamicTag {
				bootstrap_method_attr_index,
				name_and_ty_index: name_and_type_index,
			}) => {
				buffer.write_u16::<BigEndian>(*bootstrap_method_attr_index)?;
				buffer.write_u16::<BigEndian>(*name_and_type_index)?;
			}
			CPTag::Module(ModuleTag { name_index }) | CPTag::Package(PackageTag { name_index }) => {
				buffer.write_u16::<BigEndian>(*name_index)?;
			}
		}
		Ok(())
	}
}
