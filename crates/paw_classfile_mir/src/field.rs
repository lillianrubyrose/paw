use paw_classfile_format::{FieldAccessFlags, descriptor::Descriptor};

pub enum FieldValue {
	Int(i32),
	Float(f32),
	Long(i64),
	Double(f64),
	String(String),
}

pub struct MIRField {
	pub name: String,
	pub descriptor: Descriptor,
	pub access: FieldAccessFlags,
	pub value: Option<FieldValue>,
	pub is_deprecated: bool,
}
