use std::io::Cursor;

use crate::{
	AttributeInfo, ParameterAccessFlags,
	attributes::{
		AnnotationDefaultAttribute, RuntimeAnnotation, RuntimeAnnotationValue, RuntimeAnnotationsAttribute,
		RuntimeTypeAnnotationsAttribute, SignatureAttribute,
		class::{BootstrapMethod, ClassAttributeKind},
		code::{
			LIRCodeAttribute, LineNumberTableAttribute, LineNumberTableAttributeEntry, LocalVariableTableAttribute,
			LocalVariableTableEntry, LocalVariableTypeTableAttribute, LocalVariableTypeTableEntry,
			StackMapTableAttribute,
		},
	},
	constant_pool::{ConstantPool, ConstantPoolIndex},
	ext::{BytesReadExt, BytesWriteExt},
	instruction::Instruction,
};
use eyre::{Context, Result, bail};
use num_conv::Truncate;

#[derive(Debug, Clone)]
pub enum LIRMethodAttribute {
	Code(CodeAttribute),
	Exceptions(ExceptionsAttribute),
	AnnotationDefault(AnnotationDefaultAttribute),
	MethodParameters(MethodParametersAttribute),
	Synthetic,
	Deprecated,
	Signature(SignatureAttribute),
	RuntimeVisibleParameterAnnotations(RuntimeParameterAnnotationsAttribute),
	RuntimeInvisibleParameterAnnotations(RuntimeParameterAnnotationsAttribute),
	RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	Unknown(String),
}

impl LIRMethodAttribute {
	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool, class_attrs: &[ClassAttributeKind]) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;
		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"Code" => LIRMethodAttribute::Code(CodeAttribute::parse(&mut buffer, cp, class_attrs)?),
			"Exceptions" => LIRMethodAttribute::Exceptions(ExceptionsAttribute::parse(&mut buffer, cp)?),
			"Synthetic" => LIRMethodAttribute::Synthetic,
			"Deprecated" => LIRMethodAttribute::Deprecated,
			"Signature" => LIRMethodAttribute::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"RuntimeVisibleParameterAnnotations" => LIRMethodAttribute::RuntimeVisibleParameterAnnotations(
				RuntimeParameterAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleParameterAnnotations" => LIRMethodAttribute::RuntimeInvisibleParameterAnnotations(
				RuntimeParameterAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeVisibleAnnotations" => {
				LIRMethodAttribute::RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleAnnotations" => {
				LIRMethodAttribute::RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeVisibleTypeAnnotations" => LIRMethodAttribute::RuntimeVisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleTypeAnnotations" => LIRMethodAttribute::RuntimeInvisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"AnnotationDefault" => {
				LIRMethodAttribute::AnnotationDefault(RuntimeAnnotationValue::parse(&mut buffer, cp)?)
			}
			"MethodParameters" => {
				LIRMethodAttribute::MethodParameters(MethodParametersAttribute::parse(&mut buffer, cp)?)
			}
			_ => {
				eprintln!("WARN: unknown attribute {}", name);
				LIRMethodAttribute::Unknown(name.clone())
			}
		};

		let remaining = buffer.len();
		if remaining != 0 {
			bail!("{} extra attribute bytes in {} method attribute data", remaining, name);
		}
		Ok(kind)
	}

	/// Returns `true` if the lirmethod attribute is [`RuntimeVisibleParameterAnnotations`].
	///
	/// [`RuntimeVisibleParameterAnnotations`]: LIRMethodAttribute::RuntimeVisibleParameterAnnotations
	#[must_use]
	pub fn is_runtime_visible_parameter_annotations(&self) -> bool {
		matches!(self, Self::RuntimeVisibleParameterAnnotations(..))
	}

	/// Returns `true` if the lirmethod attribute is [`Code`].
	///
	/// [`Code`]: LIRMethodAttribute::Code
	#[must_use]
	pub fn is_code(&self) -> bool {
		matches!(self, Self::Code(..))
	}
}

#[derive(Debug, Clone)]
pub struct CodeAttributeException {
	pub start_pc: u16,
	pub end_pc: u16,
	pub handler_pc: u16,
	pub catch_type: Option<String>, // ClassRef
}

impl CodeAttributeException {
	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = self
			.catch_type
			.as_ref()
			.map_or(ConstantPoolIndex::new_internal(0), |t| cp.add_class(t.clone()));
		info.write_u16(self.start_pc)?;
		info.write_u16(self.end_pc)?;
		info.write_u16(self.handler_pc)?;
		info.write_u16(idx.get())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct CodeAttribute {
	pub max_stack: u16,
	pub max_locals: u16,
	pub code: Vec<Instruction>,
	pub exception_table: Vec<CodeAttributeException>,
	pub attributes: Vec<LIRCodeAttribute>,
}

impl CodeAttribute {
	pub fn parse<B: BytesReadExt>(
		buffer: &mut B,
		cp: &ConstantPool,
		class_attrs: &[ClassAttributeKind],
	) -> Result<Self> {
		let max_stack = buffer
			.read_u16()
			.wrap_err("failed to read max_stack from Code attribute")?;
		let max_locals = buffer
			.read_u16()
			.wrap_err("failed to read max_locals from Code attribute")?;
		let code_len = buffer
			.read_u32()
			.wrap_err("failed to read code_length from Code attribute")?;
		let code = buffer.read_vec_with(code_len as usize, |reader| {
			reader
				.read_u8()
				.wrap_err("failed to read code data from Code attribute")
		})?;
		let exceptions_len = buffer
			.read_u16()
			.wrap_err("failed to read exception_table_length from Code attribute")?;
		let exception_table = buffer.read_vec_with(exceptions_len as usize, |reader| {
			Ok(CodeAttributeException {
				start_pc: reader
					.read_u16()
					.wrap_err("failed to read exception_table start_pc from Code attribute")?,
				end_pc: reader
					.read_u16()
					.wrap_err("failed to read exception_table end_pc from Code attribute")?,
				handler_pc: reader
					.read_u16()
					.wrap_err("failed to read exception_table handler_pc from Code attribute")?,
				catch_type: {
					let idx = reader
						.read_u16()
						.wrap_err("failed to read exception_table catch_type from Code attribute")?;
					if idx == 0 {
						None
					} else {
						let class = cp.get_class(ConstantPoolIndex::new_internal(idx))?;
						Some(cp.resolve_class_name(class)?)
					}
				},
			})
		})?;
		let attrs_count = usize::from(
			buffer
				.read_u16()
				.wrap_err("failed to read attributes_count from Code attribute")?,
		);

		let mut code_buffer = Cursor::new(code);
		let mut code: Vec<Instruction> = Vec::new();
		while code_buffer.position() < code_buffer.get_ref().len() as u64 {
			#[allow(
				clippy::cast_possible_truncation,
				reason = "length shouldnt be >u32::MAX per jvm spec"
			)]
			let pc = code_buffer.position() as u32;
			let instruction = Instruction::parse(&mut code_buffer, cp, class_attrs, pc)?;
			code.push(instruction);
		}

		let attributes = buffer.read_vec_with(attrs_count, |reader| {
			let raw = AttributeInfo::read(reader).wrap_err("failed to read attribute from Code attribute")?;
			let name = cp.get_utf8(raw.attribute_name_index)?;

			let mut buffer = raw.info.as_slice();
			let kind = match name.as_ref() {
				"StackMapTable" => LIRCodeAttribute::StackMapTable(StackMapTableAttribute::parse(&mut buffer, cp)?),
				"LineNumberTable" => {
					let table_len = usize::from(
						buffer
							.read_u16()
							.wrap_err("failed to read line_number_table_length from LineNumberTable attribute")?,
					);
					let table = buffer.read_vec_with(table_len, |reader| {
						Ok(LineNumberTableAttributeEntry {
							start_pc: reader
								.read_u16()
								.wrap_err("failed to read line_number_table start_pc from LineNumberTable attribute")?,
							line_number: reader
								.read_u16()
								.wrap_err("failed to read line_number_table line_number from SourceFile attribute")?,
						})
					})?;
					LIRCodeAttribute::LineNumberTable(LineNumberTableAttribute { table })
				}
				"LocalVariableTable" => {
					let num_entries =
						usize::from(buffer.read_u16().wrap_err(
							"failed to read local_variable_table_length from LocalVariableTable attribute",
						)?);
					let table = buffer.read_vec_with(num_entries, |reader| {
						let start_pc = reader.read_u16()?;
						let len = reader.read_u16()?;
						let name_idx = reader.read_u16()?;
						let name = cp.get_utf8(ConstantPoolIndex::new_internal(name_idx))?;
						let descriptor_idx = reader.read_u16()?;
						let descriptor = cp.get_utf8(ConstantPoolIndex::new_internal(descriptor_idx))?.parse()?;
						let local_idx = reader.read_u16()?;
						Ok(LocalVariableTableEntry {
							start_pc,
							len,
							name,
							descriptor,
							local_idx,
						})
					})?;
					LIRCodeAttribute::LocalVariableTable(LocalVariableTableAttribute { table })
				}
				"LocalVariableTypeTable" => {
					let num_entries = usize::from(buffer.read_u16()?);
					let table = buffer.read_vec_with(num_entries, |reader| {
						let start_pc = reader.read_u16()?;
						let len = reader.read_u16()?;
						let name_idx = reader.read_u16()?;
						let name = cp.get_utf8(ConstantPoolIndex::new_internal(name_idx))?;
						let signature_idx = reader.read_u16()?;
						let signature = cp.get_utf8(ConstantPoolIndex::new_internal(signature_idx))?;
						let local_idx = reader.read_u16()?;
						Ok(LocalVariableTypeTableEntry {
							start_pc,
							len,
							name,
							signature,
							local_idx,
						})
					})?;
					LIRCodeAttribute::LocalVariableTypeTable(LocalVariableTypeTableAttribute { table })
				}
				"RuntimeVisibleTypeAnnotations" => LIRCodeAttribute::RuntimeVisibleTypeAnnotations(
					RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
				),
				"RuntimeInvisibleTypeAnnotations" => LIRCodeAttribute::RuntimeInvisibleTypeAnnotations(
					RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
				),
				_ => {
					eprintln!("WARN: unknown attribute {}", name);
					LIRCodeAttribute::Unknown(name.clone())
				}
			};

			let remaining = buffer.len();
			if remaining != 0 {
				bail!("{} extra attribute bytes in {} code attribute data", remaining, name);
			}
			Ok(kind)
		})?;

		Ok(CodeAttribute {
			max_stack,
			max_locals,
			code,
			exception_table,
			attributes,
		})
	}

	pub fn write<W: BytesWriteExt>(
		&self,
		cp: &mut ConstantPool,
		info: &mut W,
		bsm_pool: &[BootstrapMethod],
	) -> Result<()> {
		info.write_u16(self.max_stack)?;
		info.write_u16(self.max_locals)?;

		let mut code_buf = Vec::new();
		for ele in &self.code {
			let pc = code_buf.len();
			ele.write(&mut code_buf, cp, pc as u32, bsm_pool)?;
		}

		#[allow(clippy::cast_possible_truncation, reason = "aaaa")]
		info.write_u32(code_buf.len() as u32)?;
		info.write_all(&code_buf)?;

		info.write_u16(self.exception_table.len().truncate())?;
		for exc in &self.exception_table {
			exc.write(cp, info)?;
		}

		info.write_u16(self.attributes.len().truncate())?;
		for attr in &self.attributes {
			attr.write(cp)?.write(info)?;
		}

		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct ExceptionsAttribute {
	pub exception_classes: Vec<String>,
}

impl ExceptionsAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let num_exceptions = usize::from(
			buffer
				.read_u16()
				.wrap_err("failed to read number_of_exceptions from Exceptions attribute")?,
		);
		let exception_classes = buffer.read_vec_with(num_exceptions, |b| {
			let index = b
				.read_u16()
				.wrap_err("failed to read exception_index from Exceptions attribute")?;
			Ok(cp.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(index))?)?)
		})?;
		Ok(Self { exception_classes })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.exception_classes.len().truncate())?;
		for ele in &self.exception_classes {
			let idx = cp.add_class(ele.clone());
			info.write_u16(idx.get())?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct MethodParameterEntry {
	pub name: Option<String>,
	pub access: ParameterAccessFlags,
}

impl MethodParameterEntry {
	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let name_idx = self
			.name
			.as_ref()
			.map_or(ConstantPoolIndex::new_internal(0), |name| cp.add_utf8(name.clone()));
		info.write_u16(name_idx.get())?;
		info.write_u16(self.access.bits())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct MethodParametersAttribute {
	pub parameters: Vec<MethodParameterEntry>,
}

impl MethodParametersAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let param_count = usize::from(buffer.read_u8()?);
		let parameters = buffer.read_vec_with(param_count, |b| {
			let name_idx = b.read_u16()?;
			let access = ParameterAccessFlags::try_from(b.read_u16()?)?;
			let name = if name_idx != 0 {
				Some(cp.get_utf8(ConstantPoolIndex::new_internal(name_idx))?)
			} else {
				None
			};
			Ok(MethodParameterEntry { name, access })
		})?;
		Ok(Self { parameters })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u8(self.parameters.len().truncate())?;
		for ele in &self.parameters {
			ele.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeParameterAnnotation {
	pub annotations: Vec<RuntimeAnnotation>,
}

impl RuntimeParameterAnnotation {
	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.annotations.len().truncate())?;
		for ele in &self.annotations {
			ele.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeParameterAnnotationsAttribute {
	pub param_annotations: Vec<RuntimeParameterAnnotation>,
}

impl RuntimeParameterAnnotationsAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_params = usize::from(buffer.read_u8()?);
		let param_annotations = buffer.read_vec_with(n_params, |b| {
			let n_annotations = usize::from(b.read_u16()?);
			Ok(RuntimeParameterAnnotation {
				annotations: b.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?,
			})
		})?;
		Ok(Self { param_annotations })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u8(self.param_annotations.len().truncate())?;
		for ele in &self.param_annotations {
			ele.write(cp, info)?;
		}
		Ok(())
	}
}
