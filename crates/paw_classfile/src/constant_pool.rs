use core::fmt;
use std::{collections::HashMap, fmt::Display, hash::Hash};

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
	next_idx: ConstantPoolIndex,
	tags: HashMap<ConstantPoolIndex, CPTag>,
	by_tag: HashMap<CPTag, ConstantPoolIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstantPoolIndex(u16);

impl ConstantPoolIndex {
	pub fn get(&self) -> u16 {
		self.0
	}

	pub(crate) fn new_internal(val: u16) -> Self {
		Self(val)
	}
}

impl Display for ConstantPoolIndex {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

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
	pub fn new() -> Self {
		Self {
			next_idx: ConstantPoolIndex::new_internal(1),
			tags: HashMap::new(),
			by_tag: HashMap::new(),
		}
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
		Ok(self
			.tags
			.get(&idx)
			.ok_or(ConstantPoolIndexErr::IndexOutOfRange(idx, self.tags.len()))?)
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

	pub fn push(&mut self, tag: CPTag) -> ConstantPoolIndex {
		let idx = *self.by_tag.entry(tag.clone()).or_insert_with(|| {
			let idx = self.next_idx;
			let next_idx = self.next_idx.get() + tag.idx_len();
			self.next_idx = ConstantPoolIndex::new_internal(next_idx);
			idx
		});
		self.tags.insert(idx, tag);
		idx
	}

	pub fn add_utf8(&mut self, val: String) -> ConstantPoolIndex {
		self.push(CPTag::Utf8(Utf8Tag { value: val }))
	}

	pub fn add_class(&mut self, name: String) -> ConstantPoolIndex {
		let name_index = self.add_utf8(name);
		self.push(CPTag::Class(ClassTag { name_index }))
	}

	pub fn add_string(&mut self, val: String) -> ConstantPoolIndex {
		let utf8_index = self.add_utf8(val);
		self.push(CPTag::String(StringTag { utf8_index }))
	}

	pub fn add_integer(&mut self, val: u32) -> ConstantPoolIndex {
		self.push(CPTag::Integer(val))
	}

	pub fn add_float(&mut self, val: f32) -> ConstantPoolIndex {
		self.push(CPTag::Float(val))
	}

	pub fn add_long(&mut self, val: u64) -> ConstantPoolIndex {
		self.push(CPTag::Long(val))
	}

	pub fn add_double(&mut self, val: f64) -> ConstantPoolIndex {
		self.push(CPTag::Double(val))
	}

	pub fn add_name_and_type(&mut self, name: String, descriptor: String) -> ConstantPoolIndex {
		let name_index = self.add_utf8(name);
		let descriptor_index = self.add_utf8(descriptor);
		self.push(CPTag::NameAndType(NameAndTypeTag {
			name_index,
			descriptor_index,
		}))
	}

	pub fn add_field_ref(&mut self, class: String, name: String, descriptor: String) -> ConstantPoolIndex {
		let class_index = self.add_class(class);
		let name_and_ty_index = self.add_name_and_type(name, descriptor);
		self.push(CPTag::FieldRef(FieldRefTag {
			class_index,
			name_and_ty_index,
		}))
	}

	pub fn add_method_ref(&mut self, class: String, name: String, descriptor: String) -> ConstantPoolIndex {
		let class_index = self.add_class(class);
		let name_and_ty_index = self.add_name_and_type(name, descriptor);
		self.push(CPTag::MethodRef(MethodRefTag {
			class_index,
			name_and_ty_index,
		}))
	}

	pub fn add_interface_method_ref(&mut self, class: String, name: String, descriptor: String) -> ConstantPoolIndex {
		let class_index = self.add_class(class);
		let name_and_ty_index = self.add_name_and_type(name, descriptor);
		self.push(CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
			class_index,
			name_and_ty_index,
		}))
	}

	pub fn add_method_handle(
		&mut self,
		kind: MethodHandleRefKind,
		reference_index: ConstantPoolIndex,
	) -> ConstantPoolIndex {
		self.push(CPTag::MethodHandle(MethodHandleTag {
			reference_kind: kind,
			reference_index,
		}))
	}

	pub fn add_method_type(&mut self, descriptor: String) -> ConstantPoolIndex {
		let descriptor_index = self.add_utf8(descriptor);
		self.push(CPTag::MethodType(MethodTypeTag { descriptor_index }))
	}

	pub fn add_invoke_dynamic(
		&mut self,
		bsm_attr_idx: ConstantPoolIndex,
		name: String,
		descriptor: String,
	) -> ConstantPoolIndex {
		let name_and_ty_idx = self.add_name_and_type(name, descriptor);
		self.push(CPTag::InvokeDynamic(InvokeDynamicTag {
			bootstrap_method_attr_index: bsm_attr_idx,
			name_and_ty_index: name_and_ty_idx,
		}))
	}

	pub fn add_module(&mut self, name: String) -> ConstantPoolIndex {
		let name_index = self.add_utf8(name);
		self.push(CPTag::Module(ModuleTag { name_index }))
	}

	pub fn add_package(&mut self, name: String) -> ConstantPoolIndex {
		let name_index = self.add_utf8(name);
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

		let mut this = Self::new();

		let mut i = 1;
		while i < cp_count {
			let tag = CPTag::read(buffer)?;
			i += tag.idx_len();

			this.push(tag);
		}

		Ok(this)
	}

	pub fn write<B: WriteBytesExt>(&self, buffer: &mut B) -> Result<()> {
		let len = self.tags.len() + 1;
		debug_assert!(u16::try_from(len).is_ok(), "class has too many constants");
		buffer.write_u16::<BigEndian>(len.truncate())?;

		let mut tags = self.tags.iter().collect::<Vec<(&ConstantPoolIndex, &CPTag)>>();
		tags.sort_by_key(|(idx, _)| *idx);
		for (_, tag) in tags {
			tag.write(buffer)?;
		}
		Ok(())
	}
}

impl fmt::Debug for ConstantPool {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "ConstantPool(<{} tags>)", self.tags.len())
	}
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.7
pub struct Utf8Tag {
	pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassTag {
	pub name_index: ConstantPoolIndex, // StringTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringTag {
	pub utf8_index: ConstantPoolIndex, // Utf8Tag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldRefTag {
	pub class_index: ConstantPoolIndex,       // ClassTag
	pub name_and_ty_index: ConstantPoolIndex, // NameAndTypeTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MethodRefTag {
	pub class_index: ConstantPoolIndex,       // ClassTag
	pub name_and_ty_index: ConstantPoolIndex, // NameAndTypeTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceMethodRefTag {
	pub class_index: ConstantPoolIndex,       // ClassTag
	pub name_and_ty_index: ConstantPoolIndex, // NameAndTypeTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameAndTypeTag {
	pub name_index: ConstantPoolIndex,       // StringTag
	pub descriptor_index: ConstantPoolIndex, // StringTag
}

#[derive(Debug, Clone, Copy, Error)]
pub enum MethodHandleRefKindFromIntErr {
	#[error("MethodHandle kind out of range 1-9: {0}")]
	OutOfRange(u8),
}

// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-5.html#jvms-5.4.3.5
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum MethodHandleRefKind {
	GetField = 1,
	GetStatic,
	PutField,
	PutStatic,
	InvokeVirtual,
	InvokeStatic,
	InvokeSpecial,
	NewInvokeSpecial,
	InvokeInterface,
}

impl MethodHandleRefKind {
	#[must_use]
	pub const fn is_field(&self) -> bool {
		matches!(
			self,
			Self::GetField | Self::GetStatic | Self::PutField | Self::PutStatic
		)
	}
}

impl TryFrom<u8> for MethodHandleRefKind {
	type Error = MethodHandleRefKindFromIntErr;

	fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
		match value {
			1 => Ok(Self::GetField),
			2 => Ok(Self::GetStatic),
			3 => Ok(Self::PutField),
			4 => Ok(Self::PutStatic),
			5 => Ok(Self::InvokeVirtual),
			6 => Ok(Self::InvokeStatic),
			7 => Ok(Self::InvokeSpecial),
			8 => Ok(Self::NewInvokeSpecial),
			9 => Ok(Self::InvokeInterface),
			_ => Err(MethodHandleRefKindFromIntErr::OutOfRange(value)),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// <https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.8>
pub struct MethodHandleTag {
	pub reference_kind: MethodHandleRefKind,
	pub reference_index: ConstantPoolIndex, // FieldRefTag, MethodRefTag, InterfaceMethodRefTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// <https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.9>
pub struct MethodTypeTag {
	pub descriptor_index: ConstantPoolIndex, // StringTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// <https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.10>
pub struct InvokeDynamicTag {
	pub bootstrap_method_attr_index: ConstantPoolIndex,
	pub name_and_ty_index: ConstantPoolIndex, // NameAndTypeTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleTag {
	pub name_index: ConstantPoolIndex, // StringTag
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageTag {
	pub name_index: ConstantPoolIndex, // StringTag
}

#[derive(Debug, Clone)]
#[repr(C, u8)]
pub enum CPTag {
	Utf8(Utf8Tag) = 1,
	Integer(u32) = 3,
	Float(f32) = 4,
	// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.5
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

impl PartialEq for CPTag {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Utf8(l0), Self::Utf8(r0)) => l0 == r0,
			(Self::Integer(l0), Self::Integer(r0)) => l0 == r0,
			(Self::Float(l0), Self::Float(r0)) => l0.to_ne_bytes() == r0.to_ne_bytes(),
			(Self::Long(l0), Self::Long(r0)) => l0 == r0,
			(Self::Double(l0), Self::Double(r0)) => l0.to_ne_bytes() == r0.to_ne_bytes(),
			(Self::Class(l0), Self::Class(r0)) => l0 == r0,
			(Self::String(l0), Self::String(r0)) => l0 == r0,
			(Self::FieldRef(l0), Self::FieldRef(r0)) => l0 == r0,
			(Self::MethodRef(l0), Self::MethodRef(r0)) => l0 == r0,
			(Self::InterfaceMethodRef(l0), Self::InterfaceMethodRef(r0)) => l0 == r0,
			(Self::NameAndType(l0), Self::NameAndType(r0)) => l0 == r0,
			(Self::MethodHandle(l0), Self::MethodHandle(r0)) => l0 == r0,
			(Self::MethodType(l0), Self::MethodType(r0)) => l0 == r0,
			(Self::InvokeDynamic(l0), Self::InvokeDynamic(r0)) => l0 == r0,
			(Self::Module(l0), Self::Module(r0)) => l0 == r0,
			(Self::Package(l0), Self::Package(r0)) => l0 == r0,
			_ => false,
		}
	}
}

impl Eq for CPTag {}

impl PartialOrd for CPTag {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for CPTag {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		match (self, other) {
			(Self::Utf8(l0), Self::Utf8(r0)) => l0.cmp(r0),
			(Self::Integer(l0), Self::Integer(r0)) => l0.cmp(r0),
			(Self::Float(l0), Self::Float(r0)) => l0.total_cmp(r0),
			(Self::Long(l0), Self::Long(r0)) => l0.cmp(r0),
			(Self::Double(l0), Self::Double(r0)) => l0.total_cmp(r0),
			(Self::Class(l0), Self::Class(r0)) => l0.cmp(r0),
			(Self::String(l0), Self::String(r0)) => l0.cmp(r0),
			(Self::FieldRef(l0), Self::FieldRef(r0)) => l0.cmp(r0),
			(Self::MethodRef(l0), Self::MethodRef(r0)) => l0.cmp(r0),
			(Self::InterfaceMethodRef(l0), Self::InterfaceMethodRef(r0)) => l0.cmp(r0),
			(Self::NameAndType(l0), Self::NameAndType(r0)) => l0.cmp(r0),
			(Self::MethodHandle(l0), Self::MethodHandle(r0)) => l0.cmp(r0),
			(Self::MethodType(l0), Self::MethodType(r0)) => l0.cmp(r0),
			(Self::InvokeDynamic(l0), Self::InvokeDynamic(r0)) => l0.cmp(r0),
			(Self::Module(l0), Self::Module(r0)) => l0.cmp(r0),
			(Self::Package(l0), Self::Package(r0)) => l0.cmp(r0),
			_ => self.id().cmp(&other.id()),
		}
	}
}

impl Hash for CPTag {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		core::mem::discriminant(self).hash(state);
		match self {
			CPTag::Utf8(utf8_tag) => utf8_tag.hash(state),
			CPTag::Integer(i) => i.hash(state),
			CPTag::Float(f) => f.to_ne_bytes().hash(state),
			CPTag::Long(l) => l.hash(state),
			CPTag::Double(d) => d.to_ne_bytes().hash(state),
			CPTag::Class(tag) => tag.hash(state),
			CPTag::String(tag) => tag.hash(state),
			CPTag::FieldRef(tag) => tag.hash(state),
			CPTag::MethodRef(tag) => tag.hash(state),
			CPTag::InterfaceMethodRef(tag) => tag.hash(state),
			CPTag::NameAndType(tag) => tag.hash(state),
			CPTag::MethodHandle(tag) => tag.hash(state),
			CPTag::MethodType(tag) => tag.hash(state),
			CPTag::InvokeDynamic(tag) => tag.hash(state),
			CPTag::Module(tag) => tag.hash(state),
			CPTag::Package(tag) => tag.hash(state),
		}
	}
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

	pub fn idx_len(&self) -> u16 {
		match self {
			CPTag::Long(..) => 2,
			CPTag::Double(..) => 2,
			CPTag::Utf8(..)
			| CPTag::Integer(..)
			| CPTag::Float(..)
			| CPTag::Class(..)
			| CPTag::String(..)
			| CPTag::FieldRef(..)
			| CPTag::MethodRef(..)
			| CPTag::InterfaceMethodRef(..)
			| CPTag::NameAndType(..)
			| CPTag::MethodHandle(..)
			| CPTag::MethodType(..)
			| CPTag::InvokeDynamic(..)
			| CPTag::Module(..)
			| CPTag::Package(..) => 1,
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
				name_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			8 => Ok(CPTag::String(StringTag {
				utf8_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			9 => Ok(CPTag::FieldRef(FieldRefTag {
				class_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
				name_and_ty_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			10 => Ok(CPTag::MethodRef(MethodRefTag {
				class_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
				name_and_ty_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			11 => Ok(CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
				class_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
				name_and_ty_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			12 => Ok(CPTag::NameAndType(NameAndTypeTag {
				name_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
				descriptor_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			15 => Ok(CPTag::MethodHandle(MethodHandleTag {
				reference_kind: MethodHandleRefKind::try_from(buffer.read_u8()?)?,
				reference_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			16 => Ok(CPTag::MethodType(MethodTypeTag {
				descriptor_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			18 => Ok(CPTag::InvokeDynamic(InvokeDynamicTag {
				bootstrap_method_attr_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
				name_and_ty_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			19 => Ok(CPTag::Module(ModuleTag {
				name_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
			})),
			20 => Ok(CPTag::Package(PackageTag {
				name_index: ConstantPoolIndex::new_internal(buffer.read_u16::<BigEndian>()?),
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
			CPTag::Class(ClassTag { name_index }) => buffer.write_u16::<BigEndian>(name_index.get())?,
			CPTag::String(StringTag { utf8_index }) => buffer.write_u16::<BigEndian>(utf8_index.get())?,
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
				buffer.write_u16::<BigEndian>(class_index.get())?;
				buffer.write_u16::<BigEndian>(name_and_ty_index.get())?;
			}
			CPTag::NameAndType(NameAndTypeTag {
				name_index,
				descriptor_index,
			}) => {
				buffer.write_u16::<BigEndian>(name_index.get())?;
				buffer.write_u16::<BigEndian>(descriptor_index.get())?;
			}
			CPTag::MethodHandle(MethodHandleTag {
				reference_kind,
				reference_index,
			}) => {
				buffer.write_u8(*reference_kind as u8)?;
				buffer.write_u16::<BigEndian>(reference_index.get())?;
			}
			CPTag::MethodType(MethodTypeTag { descriptor_index }) => {
				buffer.write_u16::<BigEndian>(descriptor_index.get())?;
			}
			CPTag::InvokeDynamic(InvokeDynamicTag {
				bootstrap_method_attr_index,
				name_and_ty_index: name_and_type_index,
			}) => {
				buffer.write_u16::<BigEndian>(bootstrap_method_attr_index.get())?;
				buffer.write_u16::<BigEndian>(name_and_type_index.get())?;
			}
			CPTag::Module(ModuleTag { name_index }) | CPTag::Package(PackageTag { name_index }) => {
				buffer.write_u16::<BigEndian>(name_index.get())?;
			}
		}
		Ok(())
	}
}
