use crate::{
	CPTag,
	constant_pool::{ConstantPool, ConstantPoolIndex, Utf8Tag},
	descriptor::Descriptor,
	ext::{BytesReadExt, BytesWriteExt},
};
use eyre::{Context, Result, bail};
use num_conv::Truncate;

pub mod class;
pub mod code;
pub mod field;
pub mod method;
pub mod record;

#[derive(Debug, Clone)]
pub struct SignatureAttribute {
	// FIXME: more structured data?
	pub signature: String,
}

impl SignatureAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let signature_index = buffer
			.read_u16()
			.wrap_err("failed to read signature_index from Signature attribute")?;
		let signature = cp.get_utf8(ConstantPoolIndex::new_internal(signature_index))?;
		Ok(Self { signature })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_utf8(self.signature.clone());
		info.write_u16(idx.get())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub enum ConstantValueAttribute {
	Int(i32),
	Float(f32),
	Long(i64),
	Double(f64),
	String(String),
}

impl ConstantValueAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let index = buffer
			.read_u16()
			.wrap_err("failed to read tag index from ConstantValue attribute")?;
		let tag = cp
			.get_tag(ConstantPoolIndex::new_internal(index))
			.wrap_err("invalid ConstantAttribute tag index")?;
		let value = match tag {
			CPTag::Integer(i) => ConstantValueAttribute::Int(i.cast_signed()),
			CPTag::Float(f) => ConstantValueAttribute::Float(*f),
			CPTag::Long(l) => ConstantValueAttribute::Long(l.cast_signed()),
			CPTag::Double(d) => ConstantValueAttribute::Double(*d),
			CPTag::String(string_tag) => ConstantValueAttribute::String(cp.resolve_string(string_tag)?),
			_ => bail!("invalid ConstantValue attribute tag {:?}", tag),
		};
		Ok(value)
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = match self {
			ConstantValueAttribute::Int(v) => cp.add_integer(v.cast_unsigned()),
			ConstantValueAttribute::Float(v) => cp.add_float(*v),
			ConstantValueAttribute::Long(v) => cp.add_long(v.cast_unsigned()),
			ConstantValueAttribute::Double(v) => cp.add_double(*v),
			ConstantValueAttribute::String(v) => cp.add_string(v.clone()),
		};
		info.write_u16(idx.get())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub enum RuntimeAnnotationValue {
	ConstValueIndex(ConstantValueAttribute), // ConstValueRef
	EnumConstValue {
		type_name: String,  // Utf8Ref
		const_name: String, // Utf8Ref
	},
	ClassInfoIndex(String), // Utf8Ref
	Annotation(Box<RuntimeAnnotation>),
	ArrayValue {
		values: Vec<RuntimeAnnotationValue>,
	},
}

pub type AnnotationDefaultAttribute = RuntimeAnnotationValue;

impl RuntimeAnnotationValue {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let tag = buffer.read_u8()?;
		Ok(match tag {
			b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => {
				let index = buffer.read_u16()?;
				let tag = cp
					.get_tag(ConstantPoolIndex::new_internal(index))
					.wrap_err("runtime annotation value constant value tag index is invalid")?;
				Self::ConstValueIndex(match tag {
					CPTag::Integer(i) => ConstantValueAttribute::Int(i.cast_signed()),
					CPTag::Float(f) => ConstantValueAttribute::Float(*f),
					CPTag::Long(l) => ConstantValueAttribute::Long(l.cast_signed()),
					CPTag::Double(d) => ConstantValueAttribute::Double(*d),
					CPTag::Utf8(Utf8Tag { value }) => ConstantValueAttribute::String(value.clone()),
					tag => bail!("invalid RuntimeAnnotationValue attribute tag {:?}", tag),
				})
			}

			b'e' => Self::EnumConstValue {
				type_name: cp.get_utf8(ConstantPoolIndex::new_internal(buffer.read_u16()?))?,
				const_name: cp.get_utf8(ConstantPoolIndex::new_internal(buffer.read_u16()?))?,
			},

			b'c' => Self::ClassInfoIndex(cp.get_utf8(ConstantPoolIndex::new_internal(buffer.read_u16()?))?),
			b'@' => Self::Annotation(Box::new(RuntimeAnnotation::parse(buffer, cp)?)),
			b'[' => {
				let n_values = buffer.read_u16()? as usize;
				let mut values = Vec::with_capacity(n_values);

				for _ in 0..n_values {
					values.push(RuntimeAnnotationValue::parse(buffer, cp)?);
				}

				Self::ArrayValue { values }
			}
			_ => bail!("invalid runtime annotation value tag: {tag}"),
		})
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		match self {
			RuntimeAnnotationValue::ConstValueIndex(c) => {
				let (tag, idx) = match c {
					ConstantValueAttribute::Int(v) => (b'I', cp.add_integer(v.cast_unsigned())),
					ConstantValueAttribute::Float(v) => (b'F', cp.add_float(*v)),
					ConstantValueAttribute::Long(v) => (b'J', cp.add_long(v.cast_unsigned())),
					ConstantValueAttribute::Double(v) => (b'D', cp.add_double(*v)),
					ConstantValueAttribute::String(v) => (b's', cp.add_utf8(v.clone())),
				};
				info.write_u8(tag)?;
				info.write_u16(idx.get())?;
			}
			RuntimeAnnotationValue::EnumConstValue { type_name, const_name } => {
				info.write_u8(b'e')?;
				let type_idx = cp.add_utf8(type_name.clone());
				let const_idx = cp.add_utf8(const_name.clone());
				info.write_u16(type_idx.get())?;
				info.write_u16(const_idx.get())?;
			}
			RuntimeAnnotationValue::ClassInfoIndex(name) => {
				info.write_u8(b'c')?;
				let idx = cp.add_utf8(name.clone());
				info.write_u16(idx.get())?;
			}
			RuntimeAnnotationValue::Annotation(a) => {
				info.write_u8(b'@')?;
				a.write(cp, info)?;
			}
			RuntimeAnnotationValue::ArrayValue { values } => {
				info.write_u8(b'[')?;
				info.write_u16(values.len().truncate())?;
				for v in values {
					v.write(cp, info)?;
				}
			}
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeAnnotationElementValuePair {
	pub name: String, // Utf8Ref
	pub value: RuntimeAnnotationValue,
}

#[derive(Debug, Clone)]
pub struct RuntimeAnnotation {
	pub ty: Descriptor, // Utf8Ref
	pub pairs: Vec<RuntimeAnnotationElementValuePair>,
}

impl RuntimeAnnotation {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let ty = cp
			.get_utf8(ConstantPoolIndex::new_internal(buffer.read_u16()?))?
			.parse()?;

		let n_pairs = buffer.read_u16()? as usize;
		let mut pairs = Vec::with_capacity(n_pairs);

		for _ in 0..n_pairs {
			let name = cp.get_utf8(ConstantPoolIndex::new_internal(buffer.read_u16()?))?;
			pairs.push(RuntimeAnnotationElementValuePair {
				name,
				value: RuntimeAnnotationValue::parse(buffer, cp)?,
			});
		}

		Ok(Self { ty, pairs })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let type_idx = cp.add_utf8(self.ty.jvm_repr());
		info.write_u16(type_idx.get())?;
		info.write_u16(self.pairs.len().truncate())?;
		for pair in &self.pairs {
			let name_idx = cp.add_utf8(pair.name.clone());
			info.write_u16(name_idx.get())?;
			pair.value.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeAnnotationsAttribute {
	pub annotations: Vec<RuntimeAnnotation>,
}

impl RuntimeAnnotationsAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_annotations = buffer.read_u16()? as usize;
		let annotations = buffer.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?;
		Ok(Self { annotations })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.annotations.len().truncate())?;
		for anno in &self.annotations {
			anno.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeTypeAnnotationLocalVarTargetTableEntry {
	pub start_pc: u16,
	pub length: u16,
	pub index: u16,
}

#[derive(Debug, Clone)]
pub enum RuntimeTypeAnnotationTargetInfo {
	/// 0x00 (Class), 0x01 (Method)
	TypeParameterTarget { target_type: u8, type_param_index: u8 },
	/// 0x10
	SupertypeTarget { supertype_index: u16 },
	/// 0x11 (Class), 0x12 (Method)
	TypeParameterBoundTarget {
		target_type: u8,
		type_param_index: u8,
		bound_index: u8,
	},
	/// 0x13 (Field), 0x14 (Method Return), 0x15 (Receiver)
	EmptyTarget { target_type: u8 },
	/// 0x16
	FormalParameterTarget { formal_param_index: u8 },
	/// 0x17
	ThrowsTarget { throws_type_index: u16 },
	/// 0x40 (`LocalVar`), 0x41 (`ResourceVar`)
	LocalvarTarget {
		target_type: u8,
		table: Vec<RuntimeTypeAnnotationLocalVarTargetTableEntry>,
	},
	/// 0x42
	CatchTarget { exception_table_index: u16 },
	/// 0x43 (Instanceof), 0x44 (New), 0x45 (`MethodRefNew`), 0x46 (`MethodRefIdentifier`)
	OffsetTarget { target_type: u8, offset: u16 },
	/// 0x47 (Cast), 0x48 (`CtorGeneric`), 0x49 (`MethodGeneric`), 0x4A (`CtorRefGeneric`), 0x4B`MethodRefGeneric`ic)
	TypeArgumentTarget {
		target_type: u8,
		offset: u16,
		type_argument_index: u8,
	},
}

impl RuntimeTypeAnnotationTargetInfo {
	pub fn parse<B: BytesReadExt>(target_type: u8, buffer: &mut B) -> Result<Self> {
		Ok(match target_type {
			// 4.7.20-A
			0x00 | 0x01 => RuntimeTypeAnnotationTargetInfo::TypeParameterTarget {
				target_type,
				type_param_index: buffer.read_u8()?,
			},
			0x10 => RuntimeTypeAnnotationTargetInfo::SupertypeTarget {
				supertype_index: buffer.read_u16()?,
			},
			0x11 | 0x12 => RuntimeTypeAnnotationTargetInfo::TypeParameterBoundTarget {
				target_type,
				type_param_index: buffer.read_u8()?,
				bound_index: buffer.read_u8()?,
			},
			0x13..=0x15 => RuntimeTypeAnnotationTargetInfo::EmptyTarget { target_type },
			0x16 => RuntimeTypeAnnotationTargetInfo::FormalParameterTarget {
				formal_param_index: buffer.read_u8()?,
			},
			0x17 => RuntimeTypeAnnotationTargetInfo::ThrowsTarget {
				throws_type_index: buffer.read_u16()?,
			},

			// 4.7.20-B
			0x40 | 0x41 => {
				let n_entries = buffer.read_u16()? as usize;
				let mut table = Vec::with_capacity(n_entries);

				for _ in 0..n_entries {
					table.push(RuntimeTypeAnnotationLocalVarTargetTableEntry {
						start_pc: buffer.read_u16()?,
						length: buffer.read_u16()?,
						index: buffer.read_u16()?,
					});
				}

				RuntimeTypeAnnotationTargetInfo::LocalvarTarget { target_type, table }
			}
			0x42 => RuntimeTypeAnnotationTargetInfo::CatchTarget {
				exception_table_index: buffer.read_u16()?,
			},
			0x43..=0x46 => RuntimeTypeAnnotationTargetInfo::OffsetTarget {
				target_type,
				offset: buffer.read_u16()?,
			},
			0x47..=0x4B => RuntimeTypeAnnotationTargetInfo::TypeArgumentTarget {
				target_type,
				offset: buffer.read_u16()?,
				type_argument_index: buffer.read_u8()?,
			},

			target_type => bail!("Unknown RuntimeTypeAnnotationTargetInfo target_type: {}", target_type),
		})
	}

	pub fn write<W: BytesWriteExt>(&self, info: &mut W) -> Result<()> {
		match self {
			Self::TypeParameterTarget {
				target_type,
				type_param_index,
			} => {
				info.write_u8(*target_type)?;
				info.write_u8(*type_param_index)?;
			}
			Self::SupertypeTarget { supertype_index } => {
				info.write_u8(0x10)?;
				info.write_u16(*supertype_index)?;
			}
			Self::TypeParameterBoundTarget {
				target_type,
				type_param_index,
				bound_index,
			} => {
				info.write_u8(*target_type)?;
				info.write_u8(*type_param_index)?;
				info.write_u8(*bound_index)?;
			}
			Self::EmptyTarget { target_type } => {
				info.write_u8(*target_type)?;
			}
			Self::FormalParameterTarget { formal_param_index } => {
				info.write_u8(0x16)?;
				info.write_u8(*formal_param_index)?;
			}
			Self::ThrowsTarget { throws_type_index } => {
				info.write_u8(0x17)?;
				info.write_u16(*throws_type_index)?;
			}
			Self::LocalvarTarget { target_type, table } => {
				info.write_u8(*target_type)?;
				info.write_u16(table.len().truncate())?;
				for entry in table {
					info.write_u16(entry.start_pc)?;
					info.write_u16(entry.length)?;
					info.write_u16(entry.index)?;
				}
			}
			Self::CatchTarget { exception_table_index } => {
				info.write_u8(0x42)?;
				info.write_u16(*exception_table_index)?;
			}
			Self::OffsetTarget { target_type, offset } => {
				info.write_u8(*target_type)?;
				info.write_u16(*offset)?;
			}
			Self::TypeArgumentTarget {
				target_type,
				offset,
				type_argument_index,
			} => {
				info.write_u8(*target_type)?;
				info.write_u16(*offset)?;
				info.write_u8(*type_argument_index)?;
			}
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
#[repr(u8)]
/// <https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.20.2>
pub enum TypePathPart {
	/// Annotation is deeper in an array type.
	ArrayElement = 0,
	/// Annotation is deeper in a nested type.
	InnerType = 1,
	/// Annotation is on the bound of a wildcard type argument of a parameterized type.
	WildcardBound = 2,
	/// Annotation is on a type argument of a parameterized type.
	TypeArgument { index: u8 } = 3,
}

impl TypePathPart {
	pub fn parse<B: BytesReadExt>(buffer: &mut B) -> Result<Self> {
		let type_path_kind = buffer.read_u8()?;
		let type_argument_index = buffer.read_u8()?;

		match type_path_kind {
			0 => {
				if type_argument_index != 0 {
					bail!("TypePathPart::ArrayElement must have a type argument index of 0");
				}
				Ok(Self::ArrayElement)
			}
			1 => {
				if type_argument_index != 0 {
					bail!("TypePathPart::InnerType must have a type argument index of 0");
				}
				Ok(Self::InnerType)
			}
			2 => {
				if type_argument_index != 0 {
					bail!("TypePathPart::WildcardBound must have a type argument index of 0");
				}
				Ok(Self::WildcardBound)
			}
			3 => Ok(Self::TypeArgument {
				index: type_argument_index,
			}),
			kind => bail!("Unknown type path kind: {}", kind),
		}
	}

	pub fn write<W: BytesWriteExt>(&self, info: &mut W) -> Result<()> {
		match self {
			TypePathPart::ArrayElement => {
				info.write_u8(0)?; // type_path_kind
				info.write_u8(0)?; // type_argument_index
			}
			TypePathPart::InnerType => {
				info.write_u8(1)?;
				info.write_u8(0)?;
			}
			TypePathPart::WildcardBound => {
				info.write_u8(2)?;
				info.write_u8(0)?;
			}
			TypePathPart::TypeArgument { index } => {
				info.write_u8(3)?;
				info.write_u8(*index)?;
			}
		}
		Ok(())
	}
}

pub type TypePath = Vec<TypePathPart>;

fn parse_type_path<B: BytesReadExt>(buffer: &mut B) -> Result<TypePath> {
	let length = buffer.read_u8()?;
	let mut path = Vec::with_capacity(length as usize);

	for _ in 0..length {
		path.push(TypePathPart::parse(buffer)?);
	}

	Ok(path)
}

#[derive(Debug, Clone)]
pub struct RuntimeTypeAnnotation {
	pub target_info: RuntimeTypeAnnotationTargetInfo,
	pub target_path: TypePath,
	pub ty: Descriptor, // Utf8Ref
	pub pairs: Vec<RuntimeAnnotationElementValuePair>,
}

impl RuntimeTypeAnnotation {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let target_type = buffer.read_u8()?;
		let target_info = RuntimeTypeAnnotationTargetInfo::parse(target_type, buffer)?;
		let target_path = parse_type_path(buffer)?;

		let ty = cp
			.get_utf8(ConstantPoolIndex::new_internal(buffer.read_u16()?))?
			.parse()?;

		let n_pairs = buffer.read_u16()? as usize;
		let mut pairs = Vec::with_capacity(n_pairs);

		for _ in 0..n_pairs {
			let name = cp.get_utf8(ConstantPoolIndex::new_internal(buffer.read_u16()?))?;
			pairs.push(RuntimeAnnotationElementValuePair {
				name,
				value: RuntimeAnnotationValue::parse(buffer, cp)?,
			});
		}

		Ok(Self {
			target_info,
			target_path,
			ty,
			pairs,
		})
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		self.target_info.write(info)?;

		info.write_u8(self.target_path.len().truncate())?;
		for part in &self.target_path {
			part.write(info)?;
		}

		let type_idx = cp.add_utf8(self.ty.jvm_repr());
		info.write_u16(type_idx.get())?;

		info.write_u16(self.pairs.len().truncate())?;
		for pair in &self.pairs {
			let name_idx = cp.add_utf8(pair.name.clone());
			info.write_u16(name_idx.get())?;
			pair.value.write(cp, info)?;
		}

		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeTypeAnnotationsAttribute {
	pub annotations: Vec<RuntimeTypeAnnotation>,
}

impl RuntimeTypeAnnotationsAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_annotations = buffer.read_u16()? as usize;
		let annotations = buffer.read_vec_with(n_annotations, |b| RuntimeTypeAnnotation::parse(b, cp))?;
		Ok(Self { annotations })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.annotations.len().truncate())?;
		for anno in &self.annotations {
			anno.write(cp, info)?;
		}
		Ok(())
	}
}
