use paw_classfile_format::FieldAccessFlags;

use crate::{attribute::LIRFieldAttribute, descriptor::Descriptor};

#[derive(Debug, Clone)]
pub struct LIRField {
	pub access_flags: FieldAccessFlags,
	pub name: String,
	pub descriptor: Descriptor,
	pub attributes: Vec<LIRFieldAttribute>,
}
