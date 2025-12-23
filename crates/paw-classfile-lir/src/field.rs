use paw_classfile_format::FieldAccessFlags;

use crate::{attribute::LIRAttribute, descriptor::Descriptor};

#[derive(Debug, Clone)]
pub struct LIRField {
	pub access_flags: FieldAccessFlags,
	pub name: String,
	pub descriptor: Descriptor,
	pub attributes: Vec<LIRAttribute>,
}
