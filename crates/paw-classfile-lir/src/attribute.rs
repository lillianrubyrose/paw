use std::io::Cursor;

use bytemuck::AnyBitPattern;
use byteorder::{BigEndian, ReadBytesExt as _};
use eyre::bail;
use paw_classfile_format::{AttributeInfo, CPTag, ext::ReadBytesExt};

use crate::class::get_utf8_cp_entry;

#[derive(Debug, Clone)]
pub struct LIRAttribute {
	pub name: String,
	pub kind: LIRAttributeKind,
}

impl LIRAttribute {
	pub fn parse(raw: AttributeInfo, cp: &[CPTag]) -> eyre::Result<Self> {
		let name = match cp.get(raw.attribute_name_index as usize - 1).unwrap() {
			CPTag::Utf8 { bytes } => paw_mutf8::decode(&bytes)?.into_owned(),
			_ => unreachable!(),
		};

		let mut attr_buf = Cursor::new(raw.info);
		let kind = match name.as_ref() {
			"SourceFile" => {
				let index = attr_buf.read_u16::<BigEndian>()?;
				let sourcefile_utf8 = match cp.get(index as usize - 1).unwrap() {
					CPTag::Utf8 { bytes } => paw_mutf8::decode(&bytes)?.into_owned(),
					_ => unreachable!(),
				};
				LIRAttributeKind::SourceFile(sourcefile_utf8)
			}
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
				let code = attr_buf.read_vec::<u8, BigEndian>(code_len as usize)?;
				let exceptions_len = attr_buf.read_u16::<BigEndian>()?;
				let exception_table =
					attr_buf.read_vec::<CodeAttributeException, BigEndian>(usize::from(exceptions_len))?;
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
			// FIXME: implement these for realsies
			"LineNumberTable" => LIRAttributeKind::Deprecated,
			"LocalVariableTable" => LIRAttributeKind::Deprecated,
			"LocalVariableTypeTable" => LIRAttributeKind::Deprecated,
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
	Synthetic,
	Signature(String),
	SourceFile(String),
	SourceDebugExtension(String),
	LineNumberTable(LineNumberTableAttribute),
	Deprecated,
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

#[derive(Debug, Clone)]
pub enum StackMapFrame {
	SameFrame {
		frame_type: u8,
		offset_delta: u16,
	},
	SameLocals1StackItemFrame {
		frame_type: u8,
		offset_delta: u16,
		stack: VerificationTypeInfo,
	},
	SameLocals1StackItemFrameExtended {
		frame_type: u8,
		offset_delta: u16,
		stack: VerificationTypeInfo,
	},
	/*
	   The frame type chop_frame is represented by tags in the range [248-250]. If the frame_type is chop_frame,-
	   it means that the operand stack is empty and the current locals are the same as the locals in the previous frame,-
	   except that the k last locals are absent. The value of k is given by the formula 251 - frame_type.
	*/
	// TODO: do we store `k` for convenience? wtf is this shit
	ChopFrame {
		frame_type: u8,
		offset_delta: u16,
	},
	SameFrameExtended {
		frame_type: u8,
		offset_delta: u16,
	},
	AppendFrame {
		frame_type: u8,
		offset_delta: u16,
		locals: Vec<VerificationTypeInfo>,
	},
	FullFrame {
		frame_type: u8,
		offset_delta: u16,
		// number_of_locals: u16,
		locals: Vec<VerificationTypeInfo>,
		// verification_type_info locals[number_of_locals];
		// number_of_stack_items: u16,
		stack: Vec<VerificationTypeInfo>,
		// verification_type_info stack[number_of_stack_items];
	},
}

#[derive(Debug, Clone)]
pub struct InnerClassesAttributeClass {
	pub inner_class_info: String,         // ClassRef
	pub outer_class_info: Option<String>, // ClassRef
	pub inner_name: Option<String>,       // Utf8Ref
	pub inner_class_access_flags: u16,
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

#[derive(Debug, Clone)]
pub struct LineNumberTableAttributeEntry {
	pub start_pc: u16,
	pub line_number: u16,
}

#[derive(Debug, Clone)]
pub struct LineNumberTableAttribute {
	pub line_number_table: Vec<LineNumberTableAttributeEntry>,
}

#[derive(Debug, Clone)]
pub struct MethodParametersParam {
	pub name: Option<String>, // Utf8Ref
	pub access_flags: u16,
}
