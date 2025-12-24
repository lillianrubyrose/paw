use paw_classfile_format::{FieldAccessFlags, descriptor::Descriptor};

use crate::attribute::LIRFieldAttribute;

#[derive(Debug, Clone)]
pub struct LIRField {
	pub access_flags: FieldAccessFlags,
	pub name: String,
	pub descriptor: Descriptor,
	pub attributes: Vec<LIRFieldAttribute>,
}
