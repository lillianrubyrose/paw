use eyre::Result;
use paw_classfile::{AttributeInfo, FieldAccessFlags, class_pool::ConstantPool, descriptor::Descriptor};

use crate::attribute::LIRFieldAttribute;

#[derive(Debug, Clone)]
pub struct LIRField {
	pub access_flags: FieldAccessFlags,
	pub name: String,
	pub descriptor: Descriptor,
	pub attributes: Vec<LIRFieldAttribute>,
}

impl LIRFieldAttribute {
	pub fn write(&self, cp: &mut ConstantPool) -> Result<AttributeInfo> {
		let name = match self {
			LIRFieldAttribute::ConstantValue(..) => "ConstantValue",
			LIRFieldAttribute::Synthetic => "Synthetic",
			LIRFieldAttribute::Deprecated => "Deprecated",
			LIRFieldAttribute::Signature(..) => "Signature",
			LIRFieldAttribute::RuntimeVisibleAnnotations(..) => "RuntimeVisibleAnnotations",
			LIRFieldAttribute::RuntimeInvisibleAnnotations(..) => "RuntimeInvisibleAnnotations",
			LIRFieldAttribute::RuntimeVisibleTypeAnnotations(..) => "RuntimeVisibleTypeAnnotations",
			LIRFieldAttribute::RuntimeInvisibleTypeAnnotations(..) => "RuntimeInvisibleTypeAnnotations",
			LIRFieldAttribute::Unknown(name) => name.as_str(),
		};
		let attribute_name_index = cp.add_utf8(name.to_string());
		let mut info = Vec::new();

		match self {
			LIRFieldAttribute::ConstantValue(cv) => {
				cv.write(cp, &mut info)?;
			}
			LIRFieldAttribute::Synthetic | LIRFieldAttribute::Deprecated => {}
			LIRFieldAttribute::Signature(sig) => {
				sig.write(cp, &mut info)?;
			}
			LIRFieldAttribute::RuntimeVisibleAnnotations(ra) | LIRFieldAttribute::RuntimeInvisibleAnnotations(ra) => {
				ra.write(cp, &mut info)?;
			}
			LIRFieldAttribute::RuntimeVisibleTypeAnnotations(rta)
			| LIRFieldAttribute::RuntimeInvisibleTypeAnnotations(rta) => {
				rta.write(cp, &mut info)?;
			}
			LIRFieldAttribute::Unknown(_) => unreachable!("Field attribute 'Unknown' should never be written"),
		}

		Ok(AttributeInfo {
			attribute_name_index,
			info,
		})
	}
}
