use paw_classfile_format::AccessFlags;

use crate::{attribute::LIRAttribute, descriptor::MethodDescriptor};

#[derive(Debug)]
pub struct LIRMethod {
	pub access_flags: AccessFlags,
	pub name: String,
	pub descriptor: MethodDescriptor,
	pub attributes: Vec<LIRAttribute>,
}
