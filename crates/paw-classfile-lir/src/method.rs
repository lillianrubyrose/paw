use paw_classfile_format::{AccessFlags, CPTag};
use thiserror::Error;

use crate::{attribute::LIRAttribute, descriptor::MethodDescriptor};

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

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4.8
#[derive(Debug, Clone)]
pub struct LIRMethodHandle {
	pub ref_kind: LIRMethodHandleKind,
	// TODO: Document restrictions on ref_tag
	pub ref_tag: CPTag,
}

#[derive(Debug)]
pub struct LIRMethod {
	pub access_flags: AccessFlags,
	pub name: String,
	pub descriptor: MethodDescriptor,
	pub attributes: Vec<LIRAttribute>,
}
