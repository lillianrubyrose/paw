use crate::{
	AttributeInfo,
	attributes::{
		ConstantValueAttribute, RuntimeAnnotationsAttribute, RuntimeTypeAnnotationsAttribute, SignatureAttribute,
	},
	constant_pool::ConstantPool,
};
use eyre::{Result, bail};

#[derive(Debug, Clone)]
pub enum FieldAttribute {
	ConstantValue(ConstantValueAttribute),
	Synthetic,
	Deprecated,
	Signature(SignatureAttribute),
	RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	Unknown(String),
}

impl FieldAttribute {
	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"ConstantValue" => FieldAttribute::ConstantValue(ConstantValueAttribute::parse(&mut buffer, cp)?),
			"Synthetic" => FieldAttribute::Synthetic,
			"Signature" => FieldAttribute::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"Deprecated" => FieldAttribute::Deprecated,
			"RuntimeVisibleAnnotations" => {
				FieldAttribute::RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleAnnotations" => {
				FieldAttribute::RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeVisibleTypeAnnotations" => {
				FieldAttribute::RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleTypeAnnotations" => FieldAttribute::RuntimeInvisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			_ => {
				eprintln!("WARN: unknown attribute {}", name);
				FieldAttribute::Unknown(name.clone())
			}
		};

		let remaining = buffer.len();
		if remaining != 0 {
			bail!("{} extra attribute bytes in {} field attribute data", remaining, name);
		}
		Ok(kind)
	}
}
