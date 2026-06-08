use crate::{
	AttributeInfo,
	attributes::{RuntimeAnnotationsAttribute, RuntimeTypeAnnotationsAttribute, SignatureAttribute},
	constant_pool::ConstantPool,
};
use eyre::{Result, bail};

#[derive(Debug, Clone)]
pub enum LIRRecordComponentAttribute {
	Signature(SignatureAttribute),
	RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	Unknown(String),
}

impl LIRRecordComponentAttribute {
	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"Signature" => LIRRecordComponentAttribute::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"RuntimeVisibleAnnotations" => LIRRecordComponentAttribute::RuntimeVisibleAnnotations(
				RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleAnnotations" => LIRRecordComponentAttribute::RuntimeInvisibleAnnotations(
				RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeVisibleTypeAnnotations" => LIRRecordComponentAttribute::RuntimeVisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleTypeAnnotations" => LIRRecordComponentAttribute::RuntimeInvisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			_ => {
				eprintln!("WARN: unknown attribute {}", name);
				LIRRecordComponentAttribute::Unknown(name.clone())
			}
		};

		let remaining = buffer.len();
		if remaining != 0 {
			bail!("{} extra attribute bytes in {} record attribute data", remaining, name);
		}
		Ok(kind)
	}

	pub fn write(&self, cp: &mut ConstantPool) -> Result<AttributeInfo> {
		let name = match self {
			LIRRecordComponentAttribute::Signature(_) => "Signature",
			LIRRecordComponentAttribute::RuntimeVisibleAnnotations(_) => "RuntimeVisibleAnnotations",
			LIRRecordComponentAttribute::RuntimeInvisibleAnnotations(_) => "RuntimeInvisibleAnnotations",
			LIRRecordComponentAttribute::RuntimeVisibleTypeAnnotations(_) => "RuntimeVisibleTypeAnnotations",
			LIRRecordComponentAttribute::RuntimeInvisibleTypeAnnotations(_) => "RuntimeInvisibleTypeAnnotations",
			LIRRecordComponentAttribute::Unknown(_) => unreachable!("Code attribute 'Unknown' should never be written"),
		};
		let attribute_name_index = cp.add_utf8(name.to_string());
		let mut info = Vec::new();

		match self {
			LIRRecordComponentAttribute::Signature(s) => {
				s.write(cp, &mut info)?;
			}
			LIRRecordComponentAttribute::RuntimeVisibleAnnotations(ra)
			| LIRRecordComponentAttribute::RuntimeInvisibleAnnotations(ra) => {
				ra.write(cp, &mut info)?;
			}
			LIRRecordComponentAttribute::RuntimeVisibleTypeAnnotations(rta)
			| LIRRecordComponentAttribute::RuntimeInvisibleTypeAnnotations(rta) => {
				rta.write(cp, &mut info)?;
			}
			LIRRecordComponentAttribute::Unknown(_) => {
				unreachable!("Record component attribute 'Unknown' cannot be written")
			}
		}

		Ok(AttributeInfo {
			attribute_name_index,
			info,
		})
	}
}
