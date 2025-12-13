use paw_classfile_format::AccessFlags;

use crate::{attribute::LIRAttribute, descriptor::Descriptor};

#[derive(Debug, Clone)]
pub struct LIRField {
	pub access_flags: AccessFlags,
	pub name: String,
	pub descriptor: Descriptor,
	pub attributes: Vec<LIRAttribute>,
}
