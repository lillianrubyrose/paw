use std::io::Cursor;

use bytemuck::AnyBitPattern;
use byteorder::{BigEndian, ReadBytesExt as _};
use eyre::{OptionExt, Result, bail};
use paw_classfile_format::{AccessFlags, AttributeInfo, CPTag, ext::ReadBytesExt};

use crate::{
	class::get_utf8_cp_entry,
	descriptor::{Descriptor, DescriptorReader, MethodDescriptor},
	method::{LIRMethodHandle, LIRMethodHandleKind},
};

#[derive(Debug, Clone)]
pub struct LIRAttribute {
	pub name: String,
	pub kind: LIRAttributeKind,
}

impl LIRAttribute {
	pub fn parse(raw: AttributeInfo, cp: &[CPTag]) -> Result<Self> {
		let name = match cp.get(raw.attribute_name_index as usize - 1).unwrap() {
			CPTag::Utf8 { bytes } => paw_mutf8::decode(&bytes)?.into_owned(),
			_ => unreachable!(),
		};
		eprintln!("parsing attr {}", name);

		let mut attr_buf = Cursor::new(raw.info);
		let kind = match name.as_ref() {
			"ConstantValue" => {
				if attr_buf.get_ref().len() != 2 {
					bail!("ConstantValue attr must have length of exactly 2");
				}
				let index = attr_buf.read_u16::<BigEndian>().unwrap();
				let Some(tag) = cp.get(index as usize - 1) else {
					bail!("invalid value cp index in ConstantValue attr")
				};
				let value = match tag {
					CPTag::Integer(i) => ConstantValueAttribute::Int(i.cast_signed()),
					CPTag::Float(f) => ConstantValueAttribute::Float(*f),
					CPTag::Long(l) => ConstantValueAttribute::Long(l.cast_signed()),
					CPTag::Double(d) => ConstantValueAttribute::Double(*d),
					CPTag::String { utf8_index } => ConstantValueAttribute::String(get_utf8_cp_entry(cp, *utf8_index)?),
					_ => panic!("invalid ConstantValue attribute tag"),
				};
				LIRAttributeKind::ConstantValue(value)
			}
			"Code" => {
				let max_stack = attr_buf.read_u16::<BigEndian>()?;
				let max_locals = attr_buf.read_u16::<BigEndian>()?;
				let code_len = attr_buf.read_u32::<BigEndian>()?;
				let code = attr_buf.read_vec_with(code_len as usize, |reader| Ok(reader.read_u8()?))?;
				let exceptions_len = attr_buf.read_u16::<BigEndian>()?;
				let exception_table = attr_buf.read_vec_with(exceptions_len as usize, |reader| {
					Ok(CodeAttributeException {
						start_pc: reader.read_u16::<BigEndian>()?,
						end_pc: reader.read_u16::<BigEndian>()?,
						handler_pc: reader.read_u16::<BigEndian>()?,
						catch_type: reader.read_u16::<BigEndian>()?,
					})
				})?;
				let attrs_count = attr_buf.read_u16::<BigEndian>()?;
				let mut attributes = Vec::with_capacity(usize::from(attrs_count));
				for _ in 0..attrs_count {
					let attr_raw = AttributeInfo::read(&mut attr_buf)?;
					let attr = LIRAttribute::parse(attr_raw, cp)?;
					attributes.push(attr);
				}
				LIRAttributeKind::Code(CodeAttribute {
					max_stack,
					max_locals,
					code,
					exception_table,
					attributes,
				})
			}
			"StackMapTable" => {
				let entries_count = attr_buf.read_u16::<BigEndian>()?;
				let mut entries: Vec<StackMapFrame> = Vec::with_capacity(entries_count as usize);
				for _ in 0..entries_count {
					entries.push(StackMapFrame::parse(&mut attr_buf)?);
				}
				LIRAttributeKind::StackMapTable(StackMapTableAttribute { entries })
			}
			"Exceptions" => {
				let n_classes = attr_buf.read_u16::<BigEndian>()? as usize;
				let classes = attr_buf.read_vec_with(n_classes, |b| {
					let index = b.read_u16::<BigEndian>()?;
					let CPTag::Class { name_index } = cp.get(index as usize - 1).ok_or_eyre("invalid nesthost idx")?
					else {
						bail!("A NestMembers name_index idx is not a classref");
					};
					get_utf8_cp_entry(cp, *name_index)
				})?;
				LIRAttributeKind::Exceptions(classes)
			}
			"InnerClasses" => {
				let n_classes = attr_buf.read_u16::<BigEndian>()?;
				let classes =
					attr_buf.read_vec_with(usize::from(n_classes), |b| InnerClassesAttributeClass::parse(b, cp))?;
				LIRAttributeKind::InnerClasses(InnerClassesAttribute { classes })
			}
			"EnclosingMethod" => {
				let class_idx = attr_buf.read_u16::<BigEndian>()?;
				let method_idx = attr_buf.read_u16::<BigEndian>()?;
				let Some(CPTag::Class { name_index }) = cp.get(class_idx as usize - 1) else {
					bail!("EnclosingMethod class_index did not reference a Class value")
				};
				let class = get_utf8_cp_entry(cp, *name_index)?;
				let CPTag::NameAndType {
					name_index,
					descriptor_index,
				} = cp.get(method_idx as usize - 1).unwrap()
				else {
					bail!("EnclosingMethod method_index did not reference a NameAndType value")
				};
				let method_name = get_utf8_cp_entry(cp, *name_index)?;
				let method_descriptor = get_utf8_cp_entry(cp, *descriptor_index)?.parse()?;
				LIRAttributeKind::EnclosingMethod(EnclosingMethodAttribute {
					class,
					method_name,
					method_descriptor,
				})
			}
			"Synthetic" => {
				// TODO: validate that there's no data?
				LIRAttributeKind::Synthetic
			}
			"Signature" => {
				let index = attr_buf.read_u16::<BigEndian>()?;
				let signature = get_utf8_cp_entry(cp, index)?;
				LIRAttributeKind::Signature(signature)
			}
			"SourceFile" => {
				let index = attr_buf.read_u16::<BigEndian>()?;
				let sourcefile_utf8 = match cp.get(index as usize - 1).unwrap() {
					CPTag::Utf8 { bytes } => paw_mutf8::decode(&bytes)?.into_owned(),
					_ => unreachable!(),
				};
				LIRAttributeKind::SourceFile(sourcefile_utf8)
			}
			"SourceDebugExtension" => {
				let n_entries = usize::from(attr_buf.read_u16::<BigEndian>()?);
				let debug_data = attr_buf.read_vec_with(n_entries, |b| Ok(b.read_u8()?))?;
				LIRAttributeKind::SourceDebugExtension(debug_data)
			}
			"LineNumberTable" => {
				let table_len = attr_buf.read_u16::<BigEndian>()?;
				let table = attr_buf.read_vec_with(usize::from(table_len), |reader| {
					Ok(LineNumberTableAttributeEntry {
						start_pc: reader.read_u16::<BigEndian>()?,
						line_number: reader.read_u16::<BigEndian>()?,
					})
				})?;
				LIRAttributeKind::LineNumberTable(LineNumberTableAttribute { table })
			}
			"LocalVariableTable" => {
				let num_entries = attr_buf.read_u16::<BigEndian>()?;
				let mut table = Vec::with_capacity(usize::from(num_entries));
				for _ in 0..num_entries {
					table.push(LocalVariableTableEntry::read(&mut attr_buf, cp)?);
				}
				LIRAttributeKind::LocalVariableTable(LocalVariableTableAttribute { table })
			}
			"LocalVariableTypeTable" => {
				let num_entries = attr_buf.read_u16::<BigEndian>()?;
				let table = attr_buf.read_vec_with(usize::from(num_entries), |reader| {
					LocalVariableTypeTableEntry::read(reader, cp)
				})?;
				LIRAttributeKind::LocalVariableTypeTable(LocalVariableTypeTableAttribute { table })
			}
			"Deprecated" => {
				// TODO: validate that there's no data?
				LIRAttributeKind::Deprecated
			}
			"RuntimeVisibleAnnotations" => {
				let n_annotations = attr_buf.read_u16::<BigEndian>()? as usize;
				let annotations = attr_buf.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?;
				LIRAttributeKind::RuntimeVisibleAnnotations(annotations)
			}
			"RuntimeInvisibleAnnotations" => {
				let n_annotations = attr_buf.read_u16::<BigEndian>()? as usize;
				let annotations = attr_buf.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?;
				LIRAttributeKind::RuntimeInvisibleAnnotations(annotations)
			}
			"RuntimeVisibleParameterAnnotations" => {
				let n_params = attr_buf.read_u8()? as usize;
				let params = attr_buf.read_vec_with(n_params, |b| {
					let n_annotations = b.read_u16::<BigEndian>()? as usize;
					b.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))
				})?;

				LIRAttributeKind::RuntimeVisibleParameterAnnotations(params)
			}
			"RuntimeInvisibleParameterAnnotations" => {
				let n_params = attr_buf.read_u8()? as usize;
				let params = attr_buf.read_vec_with(n_params, |b| {
					let n_annotations = b.read_u16::<BigEndian>()? as usize;
					b.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))
				})?;
				LIRAttributeKind::RuntimeInvisibleParameterAnnotations(params)
			}
			"RuntimeVisibleTypeAnnotations" => {
				let n_annotations = attr_buf.read_u16::<BigEndian>()? as usize;
				let annotations = attr_buf.read_vec_with(n_annotations, |b| RuntimeTypeAnnotation::parse(b, cp))?;
				LIRAttributeKind::RuntimeVisibleTypeAnnotations(annotations)
			}
			"RuntimeInvisibleTypeAnnotations" => {
				let n_annotations = attr_buf.read_u16::<BigEndian>()? as usize;
				let annotations = attr_buf.read_vec_with(n_annotations, |b| RuntimeTypeAnnotation::parse(b, cp))?;
				LIRAttributeKind::RuntimeInvisibleTypeAnnotations(annotations)
			}
			"AnnotationDefault" => {
				LIRAttributeKind::AnnotationDefault(RuntimeAnnotationValue::parse(&mut attr_buf, cp)?)
			}
			"BootstrapMethods" => {
				let n_methods = attr_buf.read_u16::<BigEndian>()? as usize;
				let methods = attr_buf.read_vec_with(n_methods, |b| BootstrapMethod::parse(b, cp))?;
				LIRAttributeKind::BootstrapMethods(methods)
			}
			"MethodParameters" => {
				LIRAttributeKind::MethodParameters(MethodParametersAnnotation::parse(&mut attr_buf, cp)?)
			}
			// TODO: Module
			// TODO: ModulePackages
			// TODO: ModuleMainClass
			"NestHost" => {
				let idx = attr_buf.read_u16::<BigEndian>()?;
				let CPTag::Class { name_index } = cp.get(idx as usize - 1).ok_or_eyre("invalid nesthost idx")? else {
					bail!("NestHost idx is not a classref");
				};
				LIRAttributeKind::NestHost(get_utf8_cp_entry(cp, *name_index)?)
			}
			"NestMembers" => {
				let n_classes = attr_buf.read_u16::<BigEndian>()? as usize;
				let classes = attr_buf.read_vec_with(n_classes, |b| {
					let index = b.read_u16::<BigEndian>()?;
					let CPTag::Class { name_index } = cp.get(index as usize - 1).ok_or_eyre("invalid nesthost idx")?
					else {
						bail!("A NestMembers name_index idx is not a classref");
					};
					get_utf8_cp_entry(cp, *name_index)
				})?;
				LIRAttributeKind::NestMembers(classes)
			}
			"Record" => {
				let n_components = usize::from(attr_buf.read_u16::<BigEndian>()?);
				let components = attr_buf.read_vec_with(n_components, |b| RecordComponent::parse(b, cp))?;
				LIRAttributeKind::Record(components)
			}
			"PermittedSubclasses" => {
				let n_classes = attr_buf.read_u16::<BigEndian>()? as usize;
				let classes = attr_buf.read_vec_with(n_classes, |b| {
					let index = b.read_u16::<BigEndian>()?;
					let CPTag::Class { name_index } = cp.get(index as usize - 1).ok_or_eyre("invalid nesthost idx")?
					else {
						bail!("A PermittedSubclasses name_index idx is not a classref");
					};
					get_utf8_cp_entry(cp, *name_index)
				})?;
				LIRAttributeKind::PermittedSubclasses(classes)
			}
			_ => LIRAttributeKind::Unknown(name.clone()),
		};

		Ok(LIRAttribute { name, kind })
	}
}

#[derive(Debug, Clone)]
pub enum LIRAttributeKind {
	ConstantValue(ConstantValueAttribute),
	Code(CodeAttribute),
	StackMapTable(StackMapTableAttribute),
	Exceptions(Vec<String>), // list of classes declared to be thrown
	InnerClasses(InnerClassesAttribute),
	EnclosingMethod(EnclosingMethodAttribute),
	Synthetic,
	// FIXME: more structured data?
	Signature(String),
	SourceFile(String),
	SourceDebugExtension(Vec<u8>),
	LineNumberTable(LineNumberTableAttribute),
	LocalVariableTable(LocalVariableTableAttribute),
	LocalVariableTypeTable(LocalVariableTypeTableAttribute),
	Deprecated,
	RuntimeVisibleAnnotations(Vec<RuntimeAnnotation>),
	RuntimeInvisibleAnnotations(Vec<RuntimeAnnotation>),
	RuntimeVisibleParameterAnnotations(Vec<Vec<RuntimeAnnotation>>),
	RuntimeInvisibleParameterAnnotations(Vec<Vec<RuntimeAnnotation>>),
	RuntimeVisibleTypeAnnotations(Vec<RuntimeTypeAnnotation>),
	RuntimeInvisibleTypeAnnotations(Vec<RuntimeTypeAnnotation>),
	AnnotationDefault(RuntimeAnnotationValue),
	BootstrapMethods(Vec<BootstrapMethod>),
	MethodParameters(MethodParametersAnnotation),
	// TODO: Module
	// TODO: ModulePackages
	// TODO: ModuleMainClass
	NestHost(String),         // ClassRef
	NestMembers(Vec<String>), // Vec<ClassRef>
	Record(Vec<RecordComponent>),
	PermittedSubclasses(Vec<String>),
	Unknown(String),
}

#[derive(Debug, Clone)]
pub enum ConstantValueAttribute {
	Int(i32),
	Float(f32),
	Long(i64),
	Double(f64),
	String(String),
}

#[derive(Debug, Clone)]
pub struct StackMapTableAttribute {
	pub entries: Vec<StackMapFrame>,
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
	ObjectVariableInfo { cpool_idx: u16 } = 7,
	UninitializedVariableInfo { offset: u16 } = 8,
}

impl VerificationTypeInfo {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B) -> Result<Self> {
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
				cpool_idx: buffer.read_u16::<BigEndian>()?,
			},
			8 => Self::UninitializedVariableInfo {
				offset: buffer.read_u16::<BigEndian>()?,
			},
			tag => bail!("Unrecognized verification type info tag: {tag}"),
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

	pub fn parse<B: ReadBytesExt>(buffer: &mut B) -> Result<Self> {
		let frame_type = buffer.read_u8()?;
		Ok(match frame_type {
			0..=63 => Self::SameFrame { frame_type },
			64..=127 => Self::SameLocals1StackItemFrame {
				frame_type,
				stack: VerificationTypeInfo::parse(buffer)?,
			},
			247 => Self::SameLocals1StackItemFrameExtended {
				offset_delta: buffer.read_u16::<BigEndian>()?,
				stack: VerificationTypeInfo::parse(buffer)?,
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
				let locals = buffer.read_vec_with(n_locals, |b| VerificationTypeInfo::parse(b))?;
				Self::AppendFrame { offset_delta, locals }
			}
			255 => {
				let offset_delta = buffer.read_u16::<BigEndian>()?;
				let n_locals = buffer.read_u16::<BigEndian>()? as usize;
				let mut locals = Vec::with_capacity(n_locals);
				for _ in 0..n_locals {
					locals.push(VerificationTypeInfo::parse(buffer)?);
				}

				let n_stack = buffer.read_u16::<BigEndian>()? as usize;
				let mut stack = Vec::with_capacity(n_stack);
				for _ in 0..n_stack {
					stack.push(VerificationTypeInfo::parse(buffer)?);
				}

				Self::FullFrame {
					offset_delta,
					locals,
					stack,
				}
			}

			_ => panic!("invalid frame tag {frame_type}"),
		})
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

#[derive(Debug, Clone, Copy, AnyBitPattern)]
pub struct LineNumberTableAttributeEntry {
	pub start_pc: u16,
	pub line_number: u16,
}

#[derive(Debug, Clone)]
pub struct LineNumberTableAttribute {
	pub table: Vec<LineNumberTableAttributeEntry>,
}

#[derive(Debug, Clone)]
pub struct MethodParametersParam {
	pub name: Option<String>, // Utf8Ref
	pub access_flags: u16,
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
	pub fn read<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
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

#[derive(Debug, Clone)]
pub struct BootstrapMethod {
	pub method: LIRMethodHandle,
	pub arguments: Vec<CPTag>,
}

impl BootstrapMethod {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let method_idx = buffer.read_u16::<BigEndian>()?;
		let n_args = buffer.read_u16::<BigEndian>()? as usize;
		let argument_idxs = buffer.read_vec_with(n_args, |b| Ok(b.read_u16::<BigEndian>()?))?;

		let CPTag::MethodHandle {
			reference_kind,
			reference_index,
		} = cp.get(method_idx as usize - 1)
			.ok_or_eyre("bootstrap method idx doesn't point to method handle tag")?
		else {
			panic!("should be method handle");
		};

		Ok(Self {
			method: LIRMethodHandle {
				ref_kind: LIRMethodHandleKind::try_from(*reference_kind)?,
				ref_tag: cp
					.get(*reference_index as usize - 1)
					.cloned()
					.ok_or_eyre("bootstrap method method reference idx invalid")?,
			},
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
pub struct EnclosingMethodAttribute {
	pub class: String,
	pub method_name: String,
	pub method_descriptor: MethodDescriptor,
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

#[derive(Debug, Clone)]
pub struct RuntimeTypeAnnotation {
	pub target_type: u8, // TODO: Probably shouldn't store this,
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
			target_type,
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
pub struct MethodParameterEntry {
	pub name: Option<String>,
	// FIXME: document/enforce restrictions
	// FIXME: this actually uses 0x8000 as ACC_MANDATED, we need to split out the access flags by context
	pub access: AccessFlags,
}

#[derive(Debug, Clone)]
pub struct MethodParametersAnnotation {
	parameters: Vec<MethodParameterEntry>,
}

impl MethodParametersAnnotation {
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
