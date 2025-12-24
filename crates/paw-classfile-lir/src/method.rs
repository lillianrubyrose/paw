use eyre::bail;
use paw_classfile_format::{
	CPTag, MethodAccessFlags,
	class_pool::{ConstantPool, FieldRefTag, InterfaceMethodRefTag, MethodHandleTag, MethodRefTag},
	descriptor::MethodDescriptor,
};
use thiserror::Error;

use crate::attribute::LIRMethodAttribute;

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-5.html#jvms-5.4.3.5
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum LIRMethodHandleKind {
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

#[derive(Debug, Error)]
#[error("invalid MethodHandleKind: {0}")]
pub struct LIRMethodHandleKindError(u8);

impl TryFrom<u8> for LIRMethodHandleKind {
	type Error = LIRMethodHandleKindError;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
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
			_ => Err(LIRMethodHandleKindError(value)),
		}
	}
}

impl From<LIRMethodHandleKind> for u8 {
	fn from(value: LIRMethodHandleKind) -> Self {
		value as u8
	}
}

// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.8
#[derive(Debug, Clone)]
pub struct LIRMethodHandle {
	pub kind: LIRMethodHandleKind,
	pub owner: String,
	pub name: String,
	pub descriptor: MethodDescriptor,
	pub is_interface: bool,
}

impl LIRMethodHandle {
	pub fn resolve(cp: &ConstantPool, index: u16) -> eyre::Result<Self> {
		let MethodHandleTag {
			reference_kind,
			reference_index,
		} = cp.get_method_handle(index)?;

		let kind = LIRMethodHandleKind::try_from(*reference_kind)?;
		let reference_tag = cp.get_tag(*reference_index)?;
		let (class_index, name_and_ty_index, is_interface) = match reference_tag {
			CPTag::FieldRef(FieldRefTag {
				class_index,
				name_and_ty_index,
			}) => (*class_index, *name_and_ty_index, false),
			CPTag::MethodRef(MethodRefTag {
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
		let (name, descriptor) = cp.resolve_method_name_and_type(nat)?;

		Ok(Self {
			kind,
			owner,
			name,
			descriptor,
			is_interface,
		})
	}
}

#[derive(Debug)]
pub struct LIRMethod {
	pub access_flags: MethodAccessFlags,
	pub name: String,
	pub descriptor: MethodDescriptor,
	pub attributes: Vec<LIRMethodAttribute>,
}
