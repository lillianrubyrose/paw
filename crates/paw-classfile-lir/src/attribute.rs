use bytemuck::AnyBitPattern;
use byteorder::BigEndian;
use eyre::{Context, OptionExt, Result, bail, eyre};
use paw_classfile_format::{AccessFlags, AttributeInfo, CPTag, ext::ReadBytesExt};

use crate::{
	class::{get_class_name_cp_entry, get_utf8_cp_entry},
	descriptor::{Descriptor, DescriptorReader, MethodDescriptor},
	method::LIRMethodHandle,
};

#[derive(Debug, Clone)]
pub enum LIRAttribute {
	ConstantValue(ConstantValueAttribute),
	Code(CodeAttribute),
	StackMapTable(StackMapTableAttribute),
	Exceptions(ExceptionsAttribute),
	InnerClasses(InnerClassesAttribute),
	EnclosingMethod(EnclosingMethodAttribute),
	Synthetic,
	Signature(SignatureAttribute),
	SourceFile(SourceFileAttribute),
	SourceDebugExtension(DebugExtensionAttribute),
	LineNumberTable(LineNumberTableAttribute),
	LocalVariableTable(LocalVariableTableAttribute),
	LocalVariableTypeTable(LocalVariableTypeTableAttribute),
	Deprecated,
	RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeVisibleParameterAnnotations(RuntimeParameterAnnotationsAttribute),
	RuntimeInvisibleParameterAnnotations(RuntimeParameterAnnotationsAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	AnnotationDefault(RuntimeAnnotationValue),
	BootstrapMethods(BootstrapMethodsAttribute),
	MethodParameters(MethodParametersAttribute),
	// TODO: Module
	// TODO: ModulePackages
	// TODO: ModuleMainClass
	NestHost(NestHostAttribute),
	NestMembers(NestMembersAttribute),
	Record(RecordAttribute),
	PermittedSubclasses(PermittedSubclassesAttribute),
	Unknown(String),
}

impl LIRAttribute {
	pub fn parse(raw: AttributeInfo, cp: &[CPTag]) -> Result<Self> {
		let name = match cp.get(raw.attribute_name_index as usize - 1).unwrap() {
			CPTag::Utf8 { bytes } => paw_mutf8::decode(bytes)?.into_owned(),
			_ => unreachable!(),
		};
		eprintln!("parsing attr {}", name);

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"ConstantValue" => LIRAttribute::ConstantValue(ConstantValueAttribute::parse(&mut buffer, cp)?),
			"Code" => LIRAttribute::Code(CodeAttribute::parse(&mut buffer, cp)?),
			"StackMapTable" => LIRAttribute::StackMapTable(StackMapTableAttribute::parse(&mut buffer, cp)?),
			"Exceptions" => LIRAttribute::Exceptions(ExceptionsAttribute::parse(&mut buffer, cp)?),
			"InnerClasses" => LIRAttribute::InnerClasses(InnerClassesAttribute::parse(&mut buffer, cp)?),
			"EnclosingMethod" => LIRAttribute::EnclosingMethod(EnclosingMethodAttribute::parse(&mut buffer, cp)?),
			"Synthetic" => LIRAttribute::Synthetic,
			"Signature" => LIRAttribute::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"SourceFile" => LIRAttribute::SourceFile(SourceFileAttribute::parse(&mut buffer, cp)?),
			"SourceDebugExtension" => LIRAttribute::SourceDebugExtension(DebugExtensionAttribute::parse(&mut buffer)?),
			"LineNumberTable" => LIRAttribute::LineNumberTable(LineNumberTableAttribute::parse(&mut buffer)?),
			"LocalVariableTable" => {
				LIRAttribute::LocalVariableTable(LocalVariableTableAttribute::parse(&mut buffer, cp)?)
			}
			"LocalVariableTypeTable" => {
				LIRAttribute::LocalVariableTypeTable(LocalVariableTypeTableAttribute::parse(&mut buffer, cp)?)
			}
			"Deprecated" => LIRAttribute::Deprecated,
			"RuntimeVisibleAnnotations" => {
				LIRAttribute::RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleAnnotations" => {
				LIRAttribute::RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeVisibleParameterAnnotations" => LIRAttribute::RuntimeVisibleParameterAnnotations(
				RuntimeParameterAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleParameterAnnotations" => LIRAttribute::RuntimeInvisibleParameterAnnotations(
				RuntimeParameterAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeVisibleTypeAnnotations" => {
				LIRAttribute::RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleTypeAnnotations" => {
				LIRAttribute::RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"AnnotationDefault" => LIRAttribute::AnnotationDefault(RuntimeAnnotationValue::parse(&mut buffer, cp)?),
			"BootstrapMethods" => LIRAttribute::BootstrapMethods(BootstrapMethodsAttribute::parse(&mut buffer, cp)?),
			"MethodParameters" => LIRAttribute::MethodParameters(MethodParametersAttribute::parse(&mut buffer, cp)?),
			// TODO: Module
			// TODO: ModulePackages
			// TODO: ModuleMainClass
			"NestHost" => LIRAttribute::NestHost(NestHostAttribute::parse(&mut buffer, cp)?),
			"NestMembers" => LIRAttribute::NestMembers(NestMembersAttribute::parse(&mut buffer, cp)?),
			"Record" => LIRAttribute::Record(RecordAttribute::parse(&mut buffer, cp)?),
			"PermittedSubclasses" => {
				LIRAttribute::PermittedSubclasses(PermittedSubclassesAttribute::parse(&mut buffer, cp)?)
			}
			_ => LIRAttribute::Unknown(name.clone()),
		};

		let remaining = buffer.len();
		if remaining != 0 {
			// FIXME: reenable this when everything has moved
			bail!("{} extra attribute bytes in {} attribute data", remaining, name);
		}
		Ok(kind)
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let index = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read tag index from ConstantValue attribute")?;
		let Some(tag) = cp.get(index as usize - 1) else {
			bail!("cp idx {} in ConstantValue attribute was out of range", index)
		};
		let value = match tag {
			CPTag::Integer(i) => ConstantValueAttribute::Int(i.cast_signed()),
			CPTag::Float(f) => ConstantValueAttribute::Float(*f),
			CPTag::Long(l) => ConstantValueAttribute::Long(l.cast_signed()),
			CPTag::Double(d) => ConstantValueAttribute::Double(*d),
			CPTag::String { utf8_index } => ConstantValueAttribute::String(get_utf8_cp_entry(cp, *utf8_index)?),
			_ => bail!("invalid ConstantValue attribute tag {:?}", tag),
		};
		Ok(value)
	}
}

#[derive(Debug, Clone, Copy, AnyBitPattern)]
pub struct CodeAttributeException {
	pub start_pc: u16,
	pub end_pc: u16,
	pub handler_pc: u16,
	pub catch_type: u16,
}

#[derive(Debug, Clone)]
pub struct CodeAttribute {
	pub max_stack: u16,
	pub max_locals: u16,
	pub code: Vec<u8>,
	pub exception_table: Vec<CodeAttributeException>,
	pub attributes: Vec<LIRAttribute>,
}

impl CodeAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let max_stack = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read max_stack from Code attribute")?;
		let max_locals = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read max_locals from Code attribute")?;
		let code_len = buffer
			.read_u32::<BigEndian>()
			.wrap_err("failed to read code_length from Code attribute")?;
		let code = buffer.read_vec_with(code_len as usize, |reader| {
			reader
				.read_u8()
				.wrap_err("failed to read code data from Code attribute")
		})?;
		let exceptions_len = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read exception_table_length from Code attribute")?;
		let exception_table = buffer.read_vec_with(exceptions_len as usize, |reader| {
			Ok(CodeAttributeException {
				start_pc: reader
					.read_u16::<BigEndian>()
					.wrap_err("failed to read exception_table start_pc from Code attribute")?,
				end_pc: reader
					.read_u16::<BigEndian>()
					.wrap_err("failed to read exception_table end_pc from Code attribute")?,
				handler_pc: reader
					.read_u16::<BigEndian>()
					.wrap_err("failed to read exception_table handler_pc from Code attribute")?,
				catch_type: reader
					.read_u16::<BigEndian>()
					.wrap_err("failed to read exception_table catch_type from Code attribute")?,
			})
		})?;
		let attrs_count = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read attributes_count from Code attribute")?,
		);
		let attributes = buffer.read_vec_with(attrs_count, |b| {
			let raw = AttributeInfo::read(b).wrap_err("failed to read attribute from Code attribute")?;
			LIRAttribute::parse(raw, cp)
		})?;
		Ok(CodeAttribute {
			max_stack,
			max_locals,
			code,
			exception_table,
			attributes,
		})
	}
}

#[derive(Debug, Clone)]
pub enum StackMapFrame {
	SameFrame {
		frame_type: u8,
	},
	SameLocals1StackItemFrame {
		frame_type: u8,
		stack: VerificationTypeInfo,
	},
	SameLocals1StackItemFrameExtended {
		offset_delta: u16,
		stack: VerificationTypeInfo,
	},
	ChopFrame {
		chop_locals: u8,
		offset_delta: u16,
	},
	SameFrameExtended {
		offset_delta: u16,
	},
	AppendFrame {
		offset_delta: u16,
		locals: Vec<VerificationTypeInfo>,
	},
	FullFrame {
		offset_delta: u16,
		locals: Vec<VerificationTypeInfo>,
		stack: Vec<VerificationTypeInfo>,
	},
}

impl StackMapFrame {
	pub fn offset_delta(&self) -> u16 {
		match self {
			StackMapFrame::SameFrame { frame_type } => *frame_type as u16,
			StackMapFrame::SameLocals1StackItemFrame { frame_type, .. } => *frame_type as u16 - 64,
			StackMapFrame::SameLocals1StackItemFrameExtended { offset_delta, .. } => *offset_delta,
			StackMapFrame::ChopFrame { offset_delta, .. } => *offset_delta,
			StackMapFrame::SameFrameExtended { offset_delta, .. } => *offset_delta,
			StackMapFrame::AppendFrame { offset_delta, .. } => *offset_delta,
			StackMapFrame::FullFrame { offset_delta, .. } => *offset_delta,
		}
	}

	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let frame_type = buffer.read_u8()?;
		let frame = match frame_type {
			0..=63 => Self::SameFrame { frame_type },
			64..=127 => Self::SameLocals1StackItemFrame {
				frame_type,
				stack: VerificationTypeInfo::parse(buffer, cp)?,
			},
			247 => Self::SameLocals1StackItemFrameExtended {
				offset_delta: buffer.read_u16::<BigEndian>()?,
				stack: VerificationTypeInfo::parse(buffer, cp)?,
			},
			248..=250 => Self::ChopFrame {
				/*
				   The frame type chop_frame is represented by tags in the range [248-250]. If the frame_type is chop_frame,-
				   it means that the operand stack is empty and the current locals are the same as the locals in the previous frame,-
				   except that the k last locals are absent. The value of k is given by the formula 251 - frame_type.
				*/
				chop_locals: 251 - frame_type,
				offset_delta: buffer.read_u16::<BigEndian>()?,
			},
			251 => Self::SameFrameExtended {
				offset_delta: buffer.read_u16::<BigEndian>()?,
			},
			252..=254 => {
				let offset_delta = buffer.read_u16::<BigEndian>()?;

				let n_locals = (frame_type - 251) as usize;
				let locals = buffer.read_vec_with(n_locals, |b| VerificationTypeInfo::parse(b, cp))?;
				Self::AppendFrame { offset_delta, locals }
			}
			255 => {
				let offset_delta = buffer.read_u16::<BigEndian>()?;
				let n_locals = buffer.read_u16::<BigEndian>()? as usize;
				let mut locals = Vec::with_capacity(n_locals);
				for _ in 0..n_locals {
					locals.push(VerificationTypeInfo::parse(buffer, cp)?);
				}

				let n_stack = buffer.read_u16::<BigEndian>()? as usize;
				let mut stack = Vec::with_capacity(n_stack);
				for _ in 0..n_stack {
					stack.push(VerificationTypeInfo::parse(buffer, cp)?);
				}

				Self::FullFrame {
					offset_delta,
					locals,
					stack,
				}
			}
			_ => bail!("invalid frame tag {frame_type}"),
		};
		Ok(frame)
	}
}

#[derive(Debug, Clone)]
pub struct StackMapTableAttribute {
	pub entries: Vec<StackMapFrame>,
}

impl StackMapTableAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let entries_count = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read number_of_entries from StackMapTable attribute")?,
		);
		let entries = buffer.read_vec_with(entries_count, |b| StackMapFrame::parse(b, cp))?;
		Ok(StackMapTableAttribute { entries })
	}
}

#[derive(Debug, Clone)]
pub struct ExceptionsAttribute {
	exception_classes: Vec<String>,
}

impl ExceptionsAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let num_exceptions = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read number_of_exceptions from Exceptions attribute")?,
		);
		let exception_classes = buffer.read_vec_with(num_exceptions, |b| {
			let index = usize::from(
				b.read_u16::<BigEndian>()
					.wrap_err("failed to read exception_index from Exceptions attribute")?,
			);
			let tag = cp
				.get(index - 1)
				.ok_or_else(|| eyre!("cp idx {} in Exceptions attribute was out of range", index))?;
			let CPTag::Class { name_index } = tag else {
				bail!("invalid Exceptions attribute table tag {:?}", tag);
			};
			get_utf8_cp_entry(cp, *name_index)
		})?;
		Ok(Self { exception_classes })
	}
}

#[derive(Debug, Clone)]
pub struct InnerClassesAttributeClass {
	pub inner_class_info: String,         // ClassRef
	pub outer_class_info: Option<String>, // ClassRef
	pub inner_name: Option<String>,       // Utf8Ref
	pub inner_class_access_flags: AccessFlags,
}

impl InnerClassesAttributeClass {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let inner_info_idx = buffer.read_u16::<BigEndian>()?;
		let outer_info_idx = buffer.read_u16::<BigEndian>()?;
		let inner_name_idx = buffer.read_u16::<BigEndian>()?;
		let inner_class_access_flags = AccessFlags::try_from(buffer.read_u16::<BigEndian>()?)?;

		let CPTag::Class {
			name_index: inner_info_name_idx,
		} = cp.get(inner_info_idx as usize - 1)
			.ok_or_eyre("inner class inner_info_idx invalid")?
		else {
			bail!("inner class inner_info_idx doesn't point to a class tag");
		};
		let inner_class_info = get_utf8_cp_entry(cp, *inner_info_name_idx)?;

		let outer_info_tag = if outer_info_idx == 0 {
			None
		} else {
			let tag = cp
				.get(outer_info_idx as usize - 1)
				.ok_or_eyre("inner class outer_info_idx invalid")?;
			match tag {
				CPTag::Class { name_index } => Some(get_utf8_cp_entry(cp, *name_index)?),
				_ => bail!("inner class outer_info_idx doesn't point to a class tag"),
			}
		};

		let inner_name_tag = if inner_name_idx == 0 {
			None
		} else {
			Some(get_utf8_cp_entry(cp, inner_name_idx)?)
		};

		Ok(Self {
			inner_class_info,
			outer_class_info: outer_info_tag,
			inner_name: inner_name_tag,
			inner_class_access_flags,
		})
	}
}

#[derive(Debug, Clone)]
pub struct InnerClassesAttribute {
	pub classes: Vec<InnerClassesAttributeClass>,
}

impl InnerClassesAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_classes = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read number_of_classes from InnerClasses attribute")?,
		);
		let classes = buffer.read_vec_with(n_classes, |b| InnerClassesAttributeClass::parse(b, cp))?;
		Ok(InnerClassesAttribute { classes })
	}
}

#[derive(Debug, Clone)]
pub struct EnclosingMethodAttribute {
	pub class: String,
	pub method_name: String,
	pub method_descriptor: MethodDescriptor,
}

impl EnclosingMethodAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let class_idx = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read class_index from EnclosingMethod attribute")?,
		);
		let method_idx = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read method_index from EnclosingMethod attribute")?,
		);
		let tag = cp
			.get(class_idx - 1)
			.ok_or_else(|| eyre!("class_idx {} in EnclosingMethod attribute was out of range", class_idx))?;
		let CPTag::Class { name_index } = tag else {
			bail!("invalid EnclosingMethod attribute class tag {:?}", tag);
		};
		let class = get_utf8_cp_entry(cp, *name_index)?;

		let tag = cp.get(method_idx - 1).ok_or_else(|| {
			eyre!(
				"method_idx {} in EnclosingMethod attribute was out of range",
				method_idx
			)
		})?;
		let CPTag::NameAndType {
			name_index,
			descriptor_index,
		} = tag
		else {
			bail!("invalid EnclosingMethod attribute method tag {:?}", tag);
		};
		let method_name = get_utf8_cp_entry(cp, *name_index)?;
		let method_descriptor = get_utf8_cp_entry(cp, *descriptor_index)?.parse()?;
		Ok(EnclosingMethodAttribute {
			class,
			method_name,
			method_descriptor,
		})
	}
}

#[derive(Debug, Clone)]
pub struct SignatureAttribute {
	// FIXME: more structured data?
	signature: String,
}

impl SignatureAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let signature_index = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read signature_index from Signature attribute")?;
		let signature = get_utf8_cp_entry(cp, signature_index)?;
		Ok(Self { signature })
	}
}

#[derive(Debug, Clone)]
pub struct SourceFileAttribute {
	source_file: String,
}

impl SourceFileAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let sourcefile_index = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read sourcefile_index from SourceFile attribute")?;
		let source_file = get_utf8_cp_entry(cp, sourcefile_index)?;
		Ok(Self { source_file })
	}
}

#[derive(Debug, Clone)]
pub struct DebugExtensionAttribute {
	debug_data: Vec<u8>,
}

impl DebugExtensionAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B) -> Result<Self> {
		// the debug extension just uses the entire length of the attribute data
		let mut buf = Vec::new();
		buffer.read_to_end(&mut buf)?;
		Ok(Self { debug_data: buf })
	}
}

#[derive(Debug, Clone, Copy, AnyBitPattern)]
pub struct LineNumberTableAttributeEntry {
	pub start_pc: u16,
	pub line_number: u16,
}

#[derive(Debug, Clone)]
pub struct LineNumberTableAttribute {
	pub table: Vec<LineNumberTableAttributeEntry>,
}

impl LineNumberTableAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B) -> Result<Self> {
		let table_len = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read line_number_table_length from LineNumberTable attribute")?,
		);
		let table = buffer.read_vec_with(table_len, |reader| {
			Ok(LineNumberTableAttributeEntry {
				start_pc: reader
					.read_u16::<BigEndian>()
					.wrap_err("failed to read line_number_table start_pc from LineNumberTable attribute")?,
				line_number: reader
					.read_u16::<BigEndian>()
					.wrap_err("failed to read line_number_table line_number from SourceFile attribute")?,
			})
		})?;
		Ok(Self { table })
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTableEntry {
	pub start_pc: u16,
	pub len: u16,
	pub name: String,
	pub descriptor: Descriptor,
	pub local_idx: u16,
}

impl LocalVariableTableEntry {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let start_pc = buffer.read_u16::<BigEndian>()?;
		let len = buffer.read_u16::<BigEndian>()?;
		let name_idx = buffer.read_u16::<BigEndian>()?;
		let name = get_utf8_cp_entry(cp, name_idx)?;
		let descriptor_idx = buffer.read_u16::<BigEndian>()?;
		let descriptor = get_utf8_cp_entry(cp, descriptor_idx)?.parse()?;
		let local_idx = buffer.read_u16::<BigEndian>()?;
		Ok(LocalVariableTableEntry {
			start_pc,
			len,
			name,
			descriptor,
			local_idx,
		})
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTableAttribute {
	pub table: Vec<LocalVariableTableEntry>,
}

impl LocalVariableTableAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let num_entries = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read local_variable_table_length from LocalVariableTable attribute")?,
		);
		let table = buffer.read_vec_with(num_entries, |b| LocalVariableTableEntry::parse(b, cp))?;
		Ok(Self { table })
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTypeTableEntry {
	pub start_pc: u16,
	pub len: u16,
	pub name: String,
	// FIXME: strutured data for this
	pub signature: String,
	pub local_idx: u16,
}

impl LocalVariableTypeTableEntry {
	pub fn read<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let start_pc = buffer.read_u16::<BigEndian>()?;
		let len = buffer.read_u16::<BigEndian>()?;
		let name_idx = buffer.read_u16::<BigEndian>()?;
		let name = get_utf8_cp_entry(cp, name_idx)?;
		let signature_idx = buffer.read_u16::<BigEndian>()?;
		let signature = get_utf8_cp_entry(cp, signature_idx)?;
		let local_idx = buffer.read_u16::<BigEndian>()?;
		Ok(LocalVariableTypeTableEntry {
			start_pc,
			len,
			name,
			signature,
			local_idx,
		})
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTypeTableAttribute {
	pub table: Vec<LocalVariableTypeTableEntry>,
}

impl LocalVariableTypeTableAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let num_entries = usize::from(buffer.read_u16::<BigEndian>()?);
		let table = buffer.read_vec_with(num_entries, |reader| LocalVariableTypeTableEntry::read(reader, cp))?;
		Ok(Self { table })
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

impl RuntimeAnnotationValue {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let tag = buffer.read_u8()?;
		Ok(match tag {
			b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp
					.get(index as usize - 1)
					.ok_or_eyre("runtime annotation value constant value tag index is invalid")?;
				Self::ConstValueIndex(match tag {
					CPTag::Integer(i) => ConstantValueAttribute::Int(i.cast_signed()),
					CPTag::Float(f) => ConstantValueAttribute::Float(*f),
					CPTag::Long(l) => ConstantValueAttribute::Long(l.cast_signed()),
					CPTag::Double(d) => ConstantValueAttribute::Double(*d),
					CPTag::Utf8 { bytes } => ConstantValueAttribute::String(paw_mutf8::decode(&bytes)?.into_owned()),
					tag => panic!("invalid RuntimeAnnotationValue attribute tag {tag:?}"),
				})
			}

			b'e' => Self::EnumConstValue {
				type_name: get_utf8_cp_entry(cp, buffer.read_u16::<BigEndian>()?)?,
				const_name: get_utf8_cp_entry(cp, buffer.read_u16::<BigEndian>()?)?,
			},

			b'c' => Self::ClassInfoIndex(get_utf8_cp_entry(cp, buffer.read_u16::<BigEndian>()?)?),
			b'@' => Self::Annotation(Box::new(RuntimeAnnotation::parse(buffer, cp)?)),
			b'[' => {
				let n_values = buffer.read_u16::<BigEndian>()? as usize;
				let mut values = Vec::with_capacity(n_values);

				for _ in 0..n_values {
					values.push(RuntimeAnnotationValue::parse(buffer, cp)?);
				}

				Self::ArrayValue { values }
			}
			_ => bail!("invalid runtime annotation value tag: {tag}"),
		})
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let ty_idx = buffer.read_u16::<BigEndian>()?;
		let ty = get_utf8_cp_entry(cp, ty_idx)?;

		let n_pairs = buffer.read_u16::<BigEndian>()? as usize;
		let mut pairs = Vec::with_capacity(n_pairs);

		for _ in 0..n_pairs {
			let name_idx = buffer.read_u16::<BigEndian>()?;
			let name = get_utf8_cp_entry(cp, name_idx)?;

			pairs.push(RuntimeAnnotationElementValuePair {
				name,
				value: RuntimeAnnotationValue::parse(buffer, cp)?,
			});
		}

		let mut ty = DescriptorReader::new(ty);
		Ok(Self {
			ty: ty.next().ok_or_eyre("runtime annotation invalid type descriptor")??,
			pairs,
		})
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeAnnotationsAttribute {
	pub annotations: Vec<RuntimeAnnotation>,
}

impl RuntimeAnnotationsAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_annotations = buffer.read_u16::<BigEndian>()? as usize;
		let annotations = buffer.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?;
		Ok(Self { annotations })
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeParameterAnnotation {
	pub annotations: Vec<RuntimeAnnotation>,
}

#[derive(Debug, Clone)]
pub struct RuntimeParameterAnnotationsAttribute {
	pub param_annotations: Vec<RuntimeParameterAnnotation>,
}

impl RuntimeParameterAnnotationsAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_params = usize::from(buffer.read_u8()?);
		let param_annotations = buffer.read_vec_with(n_params, |b| {
			let n_annotations = usize::from(b.read_u16::<BigEndian>()?);
			Ok(RuntimeParameterAnnotation {
				annotations: b.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?,
			})
		})?;
		Ok(Self { param_annotations })
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeTypeAnnotation {
	pub target_info: RuntimeTypeAnnotationTargetInfo,
	pub target_path: TypePath,
	pub ty: Descriptor, // Utf8Ref
	pub pairs: Vec<RuntimeAnnotationElementValuePair>,
}

impl RuntimeTypeAnnotation {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let target_type = buffer.read_u8()?;
		let target_info = RuntimeTypeAnnotationTargetInfo::parse(target_type, buffer)?;
		let target_path = parse_type_path(buffer)?;

		let type_index = buffer.read_u16::<BigEndian>()?;
		let type_name = get_utf8_cp_entry(cp, type_index)?;

		let n_pairs = buffer.read_u16::<BigEndian>()? as usize;
		let mut pairs = Vec::with_capacity(n_pairs);

		for _ in 0..n_pairs {
			let name_idx = buffer.read_u16::<BigEndian>()?;
			let name = get_utf8_cp_entry(cp, name_idx)?;

			pairs.push(RuntimeAnnotationElementValuePair {
				name,
				value: RuntimeAnnotationValue::parse(buffer, cp)?,
			});
		}

		let mut type_name = DescriptorReader::new(type_name);
		Ok(Self {
			target_info,
			target_path,
			ty: type_name
				.next()
				.ok_or_eyre("invalid runtimetypeannotation type name")??,
			pairs,
		})
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeTypeAnnotationsAttribute {
	pub annotations: Vec<RuntimeTypeAnnotation>,
}

impl RuntimeTypeAnnotationsAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_annotations = buffer.read_u16::<BigEndian>()? as usize;
		let annotations = buffer.read_vec_with(n_annotations, |b| RuntimeTypeAnnotation::parse(b, cp))?;
		Ok(Self { annotations })
	}
}

#[derive(Debug, Clone)]
pub struct BootstrapMethod {
	pub method: LIRMethodHandle,
	pub arguments: Vec<CPTag>, // FIXME: Don't store CPTag directly, should be its own union, what tags are valid arguments?
}

impl BootstrapMethod {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let method_idx = buffer.read_u16::<BigEndian>()?;
		let n_args = buffer.read_u16::<BigEndian>()? as usize;
		let argument_idxs = buffer.read_vec_with(n_args, |b| Ok(b.read_u16::<BigEndian>()?))?;

		Ok(Self {
			method: LIRMethodHandle::resolve(cp, method_idx)?,
			arguments: argument_idxs
				.into_iter()
				.map(|idx| {
					cp.get(idx as usize - 1)
						.cloned()
						.ok_or_eyre("bootstrap method method reference idx invalid")
				})
				.collect::<Result<Vec<_>>>()?,
		})
	}
}

#[derive(Debug, Clone)]
pub struct BootstrapMethodsAttribute {
	methods: Vec<BootstrapMethod>,
}

impl BootstrapMethodsAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_methods = usize::from(buffer.read_u16::<BigEndian>()?);
		let methods = buffer.read_vec_with(n_methods, |b| BootstrapMethod::parse(b, cp))?;
		Ok(Self { methods })
	}
}

#[derive(Debug, Clone)]
pub struct MethodParameterEntry {
	pub name: Option<String>,
	// FIXME: document/enforce restrictions
	// FIXME: this actually uses 0x8000 as ACC_MANDATED, we need to split out the access flags by context
	pub access: AccessFlags,
}

#[derive(Debug, Clone)]
pub struct MethodParametersAttribute {
	parameters: Vec<MethodParameterEntry>,
}

impl MethodParametersAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let param_count = usize::from(buffer.read_u8()?);
		let parameters = buffer.read_vec_with(param_count, |b| {
			let name_idx = b.read_u16::<BigEndian>()?;
			let access = AccessFlags::try_from(b.read_u16::<BigEndian>()?)?;
			let name = if name_idx != 0 {
				Some(get_utf8_cp_entry(cp, name_idx)?)
			} else {
				None
			};
			Ok(MethodParameterEntry { name, access })
		})?;
		Ok(Self { parameters })
	}
}

#[derive(Debug, Clone)]
pub struct NestHostAttribute {
	host_class: String,
}

impl NestHostAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let idx = buffer.read_u16::<BigEndian>()?;
		let CPTag::Class { name_index } = cp.get(idx as usize - 1).ok_or_eyre("invalid nesthost idx")? else {
			bail!("NestHost idx is not a classref");
		};
		let host_class = get_utf8_cp_entry(cp, *name_index)?;
		Ok(Self { host_class })
	}
}

#[derive(Debug, Clone)]
pub struct NestMembersAttribute {
	member_classes: Vec<String>,
}

impl NestMembersAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_classes = usize::from(buffer.read_u16::<BigEndian>()?);
		let member_classes = buffer.read_vec_with(n_classes, |b| {
			let index = b.read_u16::<BigEndian>()?;
			let CPTag::Class { name_index } = cp.get(index as usize - 1).ok_or_eyre("invalid nestmembers idx")? else {
				bail!("A NestMembers name_index idx is not a classref");
			};
			get_utf8_cp_entry(cp, *name_index)
		})?;
		Ok(Self { member_classes })
	}
}

#[derive(Debug, Clone)]
pub struct RecordComponent {
	pub name: String,
	pub descriptor: Descriptor,
	pub attributes: Vec<LIRAttribute>,
}

impl RecordComponent {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let name_idx = buffer.read_u16::<BigEndian>()?;
		let descriptor_idx = buffer.read_u16::<BigEndian>()?;
		let attr_count = usize::from(buffer.read_u16::<BigEndian>()?);

		let name = get_utf8_cp_entry(cp, name_idx)?;
		let descriptor = get_utf8_cp_entry(cp, descriptor_idx)?.parse()?;
		let attributes = buffer.read_vec_with(attr_count, |b| {
			let attr_raw = AttributeInfo::read(b)?;
			LIRAttribute::parse(attr_raw, cp)
		})?;
		Ok(Self {
			name,
			descriptor,
			attributes,
		})
	}
}

#[derive(Debug, Clone)]
pub struct RecordAttribute {
	components: Vec<RecordComponent>,
}

impl RecordAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_components = usize::from(buffer.read_u16::<BigEndian>()?);
		let components = buffer.read_vec_with(n_components, |b| RecordComponent::parse(b, cp))?;
		Ok(Self { components })
	}
}

#[derive(Debug, Clone)]
pub struct PermittedSubclassesAttribute {
	subclasses: Vec<String>,
}

impl PermittedSubclassesAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let n_classes = buffer.read_u16::<BigEndian>()? as usize;
		let subclasses = buffer.read_vec_with(n_classes, |b| {
			let index = b.read_u16::<BigEndian>()?;
			let CPTag::Class { name_index } = cp
				.get(index as usize - 1)
				.ok_or_eyre("invalid permitted subclasses idx")?
			else {
				bail!("A PermittedSubclasses name_index idx is not a classref");
			};
			get_utf8_cp_entry(cp, *name_index)
		})?;
		Ok(Self { subclasses })
	}
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum VerificationTypeInfo {
	TopVariableInfo = 0,
	IntegerVariableInfo = 1,
	FloatVariableInfo = 2,
	LongVariableInfo = 4,
	DoubleVariableInfo = 3,
	NullVariableInfo = 5,
	UninitializedThisVariableInfo = 6,
	ObjectVariableInfo { class_name: String } = 7,
	UninitializedVariableInfo { offset: u16 } = 8,
}

impl VerificationTypeInfo {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let tag = buffer.read_u8()?;
		Ok(match tag {
			0 => Self::TopVariableInfo,
			1 => Self::IntegerVariableInfo,
			2 => Self::FloatVariableInfo,
			4 => Self::LongVariableInfo,
			3 => Self::DoubleVariableInfo,
			5 => Self::NullVariableInfo,
			6 => Self::UninitializedThisVariableInfo,
			7 => Self::ObjectVariableInfo {
				class_name: get_class_name_cp_entry(cp, buffer.read_u16::<BigEndian>()?)?,
			},
			8 => Self::UninitializedVariableInfo {
				offset: buffer.read_u16::<BigEndian>()?,
			},
			tag => bail!("Unrecognized verification type info tag: {tag}"),
		})
	}
}

#[derive(Debug, Clone)]
#[repr(u8)]
/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.7.20.2
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B) -> Result<Self> {
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
}

pub type TypePath = Vec<TypePathPart>;

fn parse_type_path<B: ReadBytesExt>(buffer: &mut B) -> Result<TypePath> {
	let length = buffer.read_u8()?;
	let mut path = Vec::with_capacity(length as usize);

	for _ in 0..length {
		path.push(TypePathPart::parse(buffer)?);
	}

	Ok(path)
}

#[derive(Debug, Clone)]
pub struct RuntimeTypeAnnotationLocalVarTargetTableEntry {
	pub start_pc: u16,
	pub length: u16,
	pub index: u16,
}

#[derive(Debug, Clone)]
pub enum RuntimeTypeAnnotationTargetInfo {
	TypeParameterTarget {
		type_param_index: u8,
	},
	SupertypeTarget {
		supertype_index: u16,
	},
	TypeParameterBoundTarget {
		type_param_index: u8,
		bound_index: u8,
	},
	EmptyTarget,
	FormalParameterTarget {
		formal_param_index: u8,
	},
	ThrowsTarget {
		throws_type_index: u16,
	},
	LocalvarTarget {
		table: Vec<RuntimeTypeAnnotationLocalVarTargetTableEntry>,
	},
	CatchTarget {
		exception_table_index: u16,
	},
	OffsetTarget {
		offset: u16,
	},
	TypeArgumentTarget {
		offset: u16,
		type_argument_index: u8,
	},
}

impl RuntimeTypeAnnotationTargetInfo {
	pub fn parse<B: ReadBytesExt>(target_type: u8, buffer: &mut B) -> Result<Self> {
		Ok(match target_type {
			// 4.7.20-A
			0x0 | 0x01 => RuntimeTypeAnnotationTargetInfo::TypeParameterTarget {
				type_param_index: buffer.read_u8()?,
			},
			0x10 => RuntimeTypeAnnotationTargetInfo::SupertypeTarget {
				supertype_index: buffer.read_u16::<BigEndian>()?,
			},
			0x11 | 0x12 => RuntimeTypeAnnotationTargetInfo::TypeParameterBoundTarget {
				type_param_index: buffer.read_u8()?,
				bound_index: buffer.read_u8()?,
			},
			0x13..=0x15 => RuntimeTypeAnnotationTargetInfo::EmptyTarget,
			0x16 => RuntimeTypeAnnotationTargetInfo::FormalParameterTarget {
				formal_param_index: buffer.read_u8()?,
			},
			0x17 => RuntimeTypeAnnotationTargetInfo::ThrowsTarget {
				throws_type_index: buffer.read_u16::<BigEndian>()?,
			},

			// 4.7.20-B
			0x40 | 0x41 => {
				let n_entries = buffer.read_u16::<BigEndian>()? as usize;
				let mut table = Vec::with_capacity(n_entries);

				for _ in 0..n_entries {
					table.push(RuntimeTypeAnnotationLocalVarTargetTableEntry {
						start_pc: buffer.read_u16::<BigEndian>()?,
						length: buffer.read_u16::<BigEndian>()?,
						index: buffer.read_u16::<BigEndian>()?,
					});
				}

				RuntimeTypeAnnotationTargetInfo::LocalvarTarget { table }
			}
			0x42 => RuntimeTypeAnnotationTargetInfo::CatchTarget {
				exception_table_index: buffer.read_u16::<BigEndian>()?,
			},
			0x43..=0x45 => RuntimeTypeAnnotationTargetInfo::OffsetTarget {
				offset: buffer.read_u16::<BigEndian>()?,
			},
			0x47..=0x4B => RuntimeTypeAnnotationTargetInfo::TypeArgumentTarget {
				offset: buffer.read_u16::<BigEndian>()?,
				type_argument_index: buffer.read_u8()?,
			},

			target_type => bail!("Unknown RuntimeTypeAnnotationTargetInfo target_type: {}", target_type),
		})
	}
}
