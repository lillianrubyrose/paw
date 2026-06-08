use std::str::FromStr;

use eyre::{bail, Result};
use paw_classfile::{
	constant_pool::{
		ConstantPool, ConstantPoolIndex, FieldRefTag, InterfaceMethodRefTag, MethodHandleTag, MethodRefTag,
	},
	descriptor::{Descriptor, MethodDescriptor},
	AttributeInfo, CPTag, MethodAccessFlags, MethodHandleRefKind,
};

use crate::attribute::{BootstrapMethod, LIRMethodAttribute};

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
