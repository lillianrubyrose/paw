use eyre::{OptionExt, bail};
use paw_classfile_format::{CPTag, MethodAccessFlags};
use thiserror::Error;

use crate::{
	attribute::LIRMethodAttribute,
	class::{get_class_name_cp_entry, get_utf8_cp_entry},
	descriptor::MethodDescriptor,
};

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
	pub descriptor: String,
	pub is_interface: bool,
}

impl LIRMethodHandle {
	pub fn resolve(cp: &[CPTag], index: u16) -> eyre::Result<Self> {
		let tag = cp.get(index as usize - 1).ok_or_eyre("method handle idx invalid")?;
		let (reference_kind, reference_index) = match tag {
			CPTag::MethodHandle {
				reference_kind,
				reference_index,
			} => (*reference_kind, *reference_index),
			_ => bail!("LIRMethodHandle expected idx {index} to be CPTag::MethodHandle"),
		};

		let kind = LIRMethodHandleKind::try_from(reference_kind)?;
		let reference_tag = cp
			.get(reference_index as usize - 1)
			.ok_or_eyre("method handle ref idx invalid")?;
		let (class_index, name_and_ty_index, is_interface) = match reference_tag {
			CPTag::FieldRef {
				class_index,
				name_and_ty_index,
			} => (*class_index, *name_and_ty_index, false),
			CPTag::MethodRef {
				class_index,
				name_and_ty_index,
			} => (*class_index, *name_and_ty_index, false),
			CPTag::InterfaceMethodRef {
				class_index,
				name_and_ty_index,
			} => (*class_index, *name_and_ty_index, true),
			tag => bail!("invalid reference tag for method handle {tag:?}"),
		};

		let owner = get_class_name_cp_entry(cp, class_index)?;
		let (name, descriptor) = match cp
			.get(name_and_ty_index as usize - 1)
			.ok_or_eyre("method handle name_and_type idx invalid")?
		{
			CPTag::NameAndType {
				name_index,
				descriptor_index,
			} => (
				get_utf8_cp_entry(cp, *name_index)?,
				get_utf8_cp_entry(cp, *descriptor_index)?,
			),
			tag => bail!("expected name and type tag. got: {tag:?}"),
		};

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
