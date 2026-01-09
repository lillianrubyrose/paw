use std::str::FromStr;

use eyre::{Result, bail};
use paw_classfile_format::{
	AttributeInfo, CPTag, MethodAccessFlags,
	class_pool::{ConstantPool, FieldRefTag, InterfaceMethodRefTag, MethodHandleTag, MethodRefTag},
	descriptor::{Descriptor, MethodDescriptor},
};
use thiserror::Error;

use crate::attribute::{BootstrapMethod, LIRMethodAttribute};

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

impl LIRMethodHandleKind {
	#[must_use]
	pub const fn is_field(&self) -> bool {
		matches!(
			self,
			Self::GetField | Self::GetStatic | Self::PutField | Self::PutStatic
		)
	}
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

#[derive(Debug, Clone, PartialEq)]
pub enum LIRHandleDescriptor {
	Field(Descriptor),
	Method(MethodDescriptor),
}

// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.4.8
#[derive(Debug, Clone, PartialEq)]
pub struct LIRMethodHandle {
	pub kind: LIRMethodHandleKind,
	pub owner: String,
	pub name: String,
	pub descriptor: LIRHandleDescriptor,
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
			LIRHandleDescriptor::Field(Descriptor::from_str(&descriptor)?)
		} else {
			LIRHandleDescriptor::Method(MethodDescriptor::from_str(&descriptor)?)
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

impl LIRMethodAttribute {
	pub fn write(&self, cp: &mut ConstantPool, bsm_pool: &[BootstrapMethod]) -> Result<AttributeInfo> {
		let name = match self {
			LIRMethodAttribute::Code(..) => "Code",
			LIRMethodAttribute::Exceptions(..) => "Exceptions",
			LIRMethodAttribute::AnnotationDefault(..) => "AnnotationDefault",
			LIRMethodAttribute::MethodParameters(..) => "MethodParameters",
			LIRMethodAttribute::Synthetic => "Synthetic",
			LIRMethodAttribute::Deprecated => "Deprecated",
			LIRMethodAttribute::Signature(..) => "Signature",
			LIRMethodAttribute::RuntimeVisibleParameterAnnotations(..) => "RuntimeVisibleParameterAnnotations",
			LIRMethodAttribute::RuntimeInvisibleParameterAnnotations(..) => "RuntimeInvisibleParameterAnnotations",
			LIRMethodAttribute::RuntimeVisibleAnnotations(..) => "RuntimeVisibleAnnotations",
			LIRMethodAttribute::RuntimeInvisibleAnnotations(..) => "RuntimeInvisibleAnnotations",
			LIRMethodAttribute::RuntimeVisibleTypeAnnotations(..) => "RuntimeVisibleTypeAnnotations",
			LIRMethodAttribute::RuntimeInvisibleTypeAnnotations(..) => "RuntimeInvisibleTypeAnnotations",
			LIRMethodAttribute::Unknown(name) => name.as_str(),
		};
		let attribute_name_index = cp.add_utf8(name.to_string());
		let mut info = Vec::new();

		match self {
			LIRMethodAttribute::Code(c) => {
				c.write(cp, &mut info, bsm_pool)?;
			}
			LIRMethodAttribute::Exceptions(e) => {
				e.write(cp, &mut info)?;
			}
			LIRMethodAttribute::AnnotationDefault(ad) => {
				ad.write(cp, &mut info)?;
			}
			LIRMethodAttribute::MethodParameters(mp) => {
				mp.write(cp, &mut info)?;
			}
			LIRMethodAttribute::Synthetic | LIRMethodAttribute::Deprecated => {}
			LIRMethodAttribute::Signature(s) => {
				s.write(cp, &mut info)?;
			}
			LIRMethodAttribute::RuntimeVisibleParameterAnnotations(rpa)
			| LIRMethodAttribute::RuntimeInvisibleParameterAnnotations(rpa) => {
				rpa.write(cp, &mut info)?;
			}
			LIRMethodAttribute::RuntimeVisibleAnnotations(ra) | LIRMethodAttribute::RuntimeInvisibleAnnotations(ra) => {
				ra.write(cp, &mut info)?;
			}
			LIRMethodAttribute::RuntimeVisibleTypeAnnotations(rta)
			| LIRMethodAttribute::RuntimeInvisibleTypeAnnotations(rta) => {
				rta.write(cp, &mut info)?;
			}
			LIRMethodAttribute::Unknown(_) => unreachable!("Method attribute 'Unknown' should never be written"),
		}

		Ok(AttributeInfo {
			attribute_name_index,
			info,
		})
	}
}
