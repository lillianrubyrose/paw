use std::io::Cursor;

use bytemuck::AnyBitPattern;
use byteorder::{BigEndian, ReadBytesExt as _};
use eyre::{OptionExt, Result, bail};
use paw_classfile_format::{AccessFlags, AttributeInfo, CPTag, ext::ReadBytesExt};

use crate::{
	class::get_utf8_cp_entry,
	descriptor::Descriptor,
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
			// TODO: exceptions
			"InnerClasses" => {
				let n_classes = attr_buf.read_u16::<BigEndian>()?;
				let classes =
					attr_buf.read_vec_with(usize::from(n_classes), |b| InnerClassesAttributeClass::parse(b, cp))?;
				LIRAttributeKind::InnerClasses(InnerClassesAttribute { classes })
			}
			// TODO: EnclosingMethod
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
			// TODO: SourceDebugExtension
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
			// TODO: RuntimeVisibleAnnotations
			// TODO: RuntimeInvisibleAnnotations
			// TODO: RuntimeVisibleParameterAnnotations
			// TODO: RuntimeInisibleParameterAnnotations
			// TODO: AnnotationDefault
			"BootstrapMethods" => {
				let n_methods = attr_buf.read_u16::<BigEndian>()? as usize;
				let mut methods = Vec::with_capacity(n_methods);
				for _ in 0..n_methods {
					methods.push(BootstrapMethod::parse(&mut attr_buf, cp)?);
				}
				LIRAttributeKind::BootstrapMethods(methods)
			}

			n => panic!("unparsed attribute: {n}"),
		};

		Ok(LIRAttribute { name, kind })
	}
}

#[derive(Debug, Clone)]
pub enum LIRAttributeKind {
	ConstantValue(ConstantValueAttribute),
	Code(CodeAttribute),
	StackMapTable(StackMapTableAttribute),
	Exceptions { exception_index_table: Vec<String> },
	InnerClasses(InnerClassesAttribute),
	// TODO: EnclosingMethod
	Synthetic,
	// FIXME: more structured data?
	Signature(String),
	SourceFile(String),
	SourceDebugExtension(String),
	LineNumberTable(LineNumberTableAttribute),
	LocalVariableTable(LocalVariableTableAttribute),
	LocalVariableTypeTable(LocalVariableTypeTableAttribute),
	Deprecated,
	// TODO: RuntimeVisibleAnnotations
	// TODO: RuntimeInvisibleAnnotations
	// TODO: RuntimeVisibleParameterAnnotations
	// TODO: RuntimeInisibleParameterAnnotations
	// TODO: AnnotationDefault
	BootstrapMethods(Vec<BootstrapMethod>),
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
