use crate::{
	AttributeInfo,
	attributes::RuntimeTypeAnnotationsAttribute,
	constant_pool::{ConstantPool, ConstantPoolIndex},
	descriptor::Descriptor,
	ext::{BytesReadExt, BytesWriteExt},
};
use eyre::{Context, Result, bail};
use num_conv::Truncate;

#[derive(Debug, Clone)]
pub enum LIRCodeAttribute {
	LineNumberTable(LineNumberTableAttribute),
	LocalVariableTable(LocalVariableTableAttribute),
	LocalVariableTypeTable(LocalVariableTypeTableAttribute),
	StackMapTable(StackMapTableAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	Unknown(String),
}

impl LIRCodeAttribute {
	pub fn write(&self, cp: &mut ConstantPool) -> Result<AttributeInfo> {
		let name = match self {
			LIRCodeAttribute::LineNumberTable(..) => "LineNumberTable",
			LIRCodeAttribute::LocalVariableTable(..) => "LocalVariableTable",
			LIRCodeAttribute::LocalVariableTypeTable(..) => "LocalVariableTypeTable",
			LIRCodeAttribute::StackMapTable(..) => "StackMapTable",
			LIRCodeAttribute::RuntimeVisibleTypeAnnotations(..) => "RuntimeVisibleTypeAnnotations",
			LIRCodeAttribute::RuntimeInvisibleTypeAnnotations(..) => "RuntimeInvisibleTypeAnnotations",
			LIRCodeAttribute::Unknown(_) => unreachable!("Code attribute 'Unknown' should never be written"),
		};
		let attribute_name_index = cp.add_utf8(name.to_string());
		let mut info = Vec::new();

		match self {
			LIRCodeAttribute::LineNumberTable(lnt) => {
				lnt.write(&mut info)?;
			}
			LIRCodeAttribute::LocalVariableTable(lvt) => {
				lvt.write(cp, &mut info)?;
			}
			LIRCodeAttribute::LocalVariableTypeTable(lvtt) => {
				lvtt.write(cp, &mut info)?;
			}
			LIRCodeAttribute::StackMapTable(smt) => {
				smt.write(cp, &mut info)?;
			}
			LIRCodeAttribute::RuntimeVisibleTypeAnnotations(rta)
			| LIRCodeAttribute::RuntimeInvisibleTypeAnnotations(rta) => {
				rta.write(cp, &mut info)?;
			}
			LIRCodeAttribute::Unknown(_) => unreachable!("Code attribute 'Unknown' should never be written"),
		}

		Ok(AttributeInfo {
			attribute_name_index,
			info,
		})
	}
}

#[derive(Debug, Clone, Copy)]
pub struct LineNumberTableAttributeEntry {
	pub start_pc: u16,
	pub line_number: u16,
}

#[derive(Debug, Clone)]
pub struct LineNumberTableAttribute {
	pub table: Vec<LineNumberTableAttributeEntry>,
}

impl LineNumberTableAttribute {
	pub fn write<W: BytesWriteExt>(&self, info: &mut W) -> Result<()> {
		info.write_u16(self.table.len().truncate())?;
		for ele in &self.table {
			info.write_u16(ele.start_pc)?;
			info.write_u16(ele.line_number)?;
		}
		Ok(())
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
	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.start_pc)?;
		info.write_u16(self.len)?;

		let name_idx = cp.add_utf8(self.name.clone());
		info.write_u16(name_idx.get())?;

		let descriptor_idx = cp.add_utf8(self.descriptor.jvm_repr());
		info.write_u16(descriptor_idx.get())?;

		info.write_u16(self.local_idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTableAttribute {
	pub table: Vec<LocalVariableTableEntry>,
}

impl LocalVariableTableAttribute {
	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.table.len().truncate())?;
		for ele in &self.table {
			ele.write(cp, info)?;
		}
		Ok(())
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
	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.start_pc)?;
		info.write_u16(self.len)?;

		let name_idx = cp.add_utf8(self.name.clone());
		info.write_u16(name_idx.get())?;

		let signature_idx = cp.add_utf8(self.signature.clone());
		info.write_u16(signature_idx.get())?;

		info.write_u16(self.local_idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTypeTableAttribute {
	pub table: Vec<LocalVariableTypeTableEntry>,
}

impl LocalVariableTypeTableAttribute {
	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.table.len().truncate())?;
		for ele in &self.table {
			ele.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct SameFrameType(u8);

impl SameFrameType {
	pub fn new(frame_type: u8) -> Result<Self> {
		if frame_type > 63 {
			bail!("Invalid SameFrame tag: {}", frame_type);
		}
		Ok(Self(frame_type))
	}
}

#[derive(Debug, Clone)]
pub struct SameLocals1StackItemFrameType(u8);

impl SameLocals1StackItemFrameType {
	pub fn new(frame_type: u8) -> Result<Self> {
		if frame_type > 127 || frame_type < 64 {
			bail!("Invalid SameLocals1StackItemFrame tag: {}", frame_type);
		}
		Ok(Self(frame_type))
	}
}

#[derive(Debug, Clone)]
pub struct ChopFrameAbsentLocals(u8);

impl ChopFrameAbsentLocals {
	pub fn new(absent_locals: u8) -> Result<Self> {
		if absent_locals < 1 || absent_locals > 3 {
			bail!("Invalid ChopFrame absent locals count: {}", absent_locals);
		}
		Ok(Self(absent_locals))
	}
}

#[derive(Debug, Clone)]
pub struct AppendFrameLocals(Vec<VerificationTypeInfo>);

impl AppendFrameLocals {
	#[must_use]
	pub const fn new() -> Self {
		Self(Vec::new())
	}

	pub fn from_vec(locals: Vec<VerificationTypeInfo>) -> Result<Self> {
		// The frame type append_frame is represented by tags in the range [252-254].
		// This frame type indicates that the frame has the same locals as the previous frame except that k additional locals are defined,
		// and that the operand stack is empty. The value of k is given by the formula frame_type - 251.
		// The offset_delta value for the frame is given explicitly.
		if locals.len() > 3 {
			bail!("AppendFrame locals count is too large (max 3): {}", locals.len());
		}

		Ok(Self(locals))
	}

	pub fn push(&mut self, local: VerificationTypeInfo) -> Result<()> {
		if self.0.len() >= 3 {
			bail!("AppendFrame locals count is already at maximum (3)");
		}
		self.0.push(local);
		Ok(())
	}

	#[must_use]
	pub fn into_vec(self) -> Vec<VerificationTypeInfo> {
		self.0
	}
}

impl Default for AppendFrameLocals {
	fn default() -> Self {
		Self::new()
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
	UninitializedVariableInfo { new_instruction_pc: u16 } = 8,
}

impl VerificationTypeInfo {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
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
				class_name: cp
					.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(buffer.read_u16()?))?)?,
			},
			8 => Self::UninitializedVariableInfo {
				new_instruction_pc: buffer.read_u16()?,
			},
			tag => bail!("Unrecognized verification type info tag: {tag}"),
		})
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		match self {
			VerificationTypeInfo::TopVariableInfo => info.write_u8(0)?,
			VerificationTypeInfo::IntegerVariableInfo => info.write_u8(1)?,
			VerificationTypeInfo::FloatVariableInfo => info.write_u8(2)?,
			VerificationTypeInfo::DoubleVariableInfo => info.write_u8(3)?,
			VerificationTypeInfo::LongVariableInfo => info.write_u8(4)?,
			VerificationTypeInfo::NullVariableInfo => info.write_u8(5)?,
			VerificationTypeInfo::UninitializedThisVariableInfo => info.write_u8(6)?,
			VerificationTypeInfo::ObjectVariableInfo { class_name } => {
				info.write_u8(7)?;
				let idx = cp.add_class(class_name.clone());
				info.write_u16(idx.get())?;
			}
			VerificationTypeInfo::UninitializedVariableInfo { new_instruction_pc } => {
				info.write_u8(8)?;
				info.write_u16(*new_instruction_pc)?;
			}
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub enum StackMapFrame {
	SameFrame {
		frame_type: SameFrameType,
	},
	SameLocals1StackItemFrame {
		frame_type: SameLocals1StackItemFrameType,
		stack: VerificationTypeInfo,
	},
	SameLocals1StackItemFrameExtended {
		offset_delta: u16,
		stack: VerificationTypeInfo,
	},
	ChopFrame {
		absent_locals_count: ChopFrameAbsentLocals,
		offset_delta: u16,
	},
	SameFrameExtended {
		offset_delta: u16,
	},
	AppendFrame {
		offset_delta: u16,
		locals: AppendFrameLocals,
	},
	FullFrame {
		offset_delta: u16,
		locals: Vec<VerificationTypeInfo>,
		stack: Vec<VerificationTypeInfo>,
	},
}

impl StackMapFrame {
	#[must_use]
	pub const fn offset_delta(&self) -> u16 {
		match self {
			StackMapFrame::SameFrame {
				frame_type: SameFrameType(frame_type),
			} => *frame_type as u16,
			StackMapFrame::SameLocals1StackItemFrame {
				frame_type: SameLocals1StackItemFrameType(frame_type),
				..
			} => *frame_type as u16 - 64,
			StackMapFrame::SameLocals1StackItemFrameExtended { offset_delta, .. }
			| StackMapFrame::ChopFrame { offset_delta, .. }
			| StackMapFrame::SameFrameExtended { offset_delta, .. }
			| StackMapFrame::AppendFrame { offset_delta, .. }
			| StackMapFrame::FullFrame { offset_delta, .. } => *offset_delta,
		}
	}

	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let frame_type = buffer.read_u8()?;
		let frame = match frame_type {
			0..=63 => Self::SameFrame {
				frame_type: SameFrameType::new(frame_type)?,
			},
			64..=127 => Self::SameLocals1StackItemFrame {
				frame_type: SameLocals1StackItemFrameType::new(frame_type)?,
				stack: VerificationTypeInfo::parse(buffer, cp)?,
			},
			247 => Self::SameLocals1StackItemFrameExtended {
				offset_delta: buffer.read_u16()?,
				stack: VerificationTypeInfo::parse(buffer, cp)?,
			},
			248..=250 => Self::ChopFrame {
				/*
				   The frame type chop_frame is represented by tags in the range [248-250]. If the frame_type is chop_frame,-
				   it means that the operand stack is empty and the current locals are the same as the locals in the previous frame,-
				   except that the k last locals are absent. The value of k is given by the formula 251 - frame_type.
				*/
				absent_locals_count: ChopFrameAbsentLocals::new(251 - frame_type)?,
				offset_delta: buffer.read_u16()?,
			},
			251 => Self::SameFrameExtended {
				offset_delta: buffer.read_u16()?,
			},
			252..=254 => {
				let offset_delta = buffer.read_u16()?;

				let n_locals = (frame_type - 251) as usize;
				let locals = buffer.read_vec_with(n_locals, |b| VerificationTypeInfo::parse(b, cp))?;
				Self::AppendFrame {
					offset_delta,
					locals: AppendFrameLocals::from_vec(locals)?,
				}
			}
			255 => {
				let offset_delta = buffer.read_u16()?;
				let n_locals = buffer.read_u16()? as usize;
				let mut locals = Vec::with_capacity(n_locals);
				for _ in 0..n_locals {
					locals.push(VerificationTypeInfo::parse(buffer, cp)?);
				}

				let n_stack = buffer.read_u16()? as usize;
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

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		match self {
			StackMapFrame::SameFrame { frame_type } => {
				info.write_u8(frame_type.0)?;
			}
			StackMapFrame::SameLocals1StackItemFrame { frame_type, stack } => {
				info.write_u8(frame_type.0)?;
				stack.write(cp, info)?;
			}
			StackMapFrame::SameLocals1StackItemFrameExtended { offset_delta, stack } => {
				info.write_u8(247)?;
				info.write_u16(*offset_delta)?;
				stack.write(cp, info)?;
			}
			StackMapFrame::ChopFrame {
				absent_locals_count: ChopFrameAbsentLocals(absent_locals_count),
				offset_delta,
			} => {
				/*
				   The frame type chop_frame is represented by tags in the range [248-250]. If the frame_type is chop_frame,-
				   it means that the operand stack is empty and the current locals are the same as the locals in the previous frame,-
				   except that the k last locals are absent. The value of k is given by the formula 251 - frame_type.
				*/
				let frame_type = 251_u8
					.checked_sub(*absent_locals_count)
					.ok_or_else(|| eyre::eyre!("Invalid chopframe absent locals count: {}", absent_locals_count))?;

				info.write_u8(frame_type)?;
				info.write_u16(*offset_delta)?;
			}
			StackMapFrame::SameFrameExtended { offset_delta } => {
				info.write_u8(251)?;
				info.write_u16(*offset_delta)?;
			}
			StackMapFrame::AppendFrame {
				offset_delta,
				locals: AppendFrameLocals(locals),
			} => {
				let n_locals = locals.len().truncate::<u8>();
				let frame_type = 251 + n_locals;
				debug_assert!(
					frame_type >= 252 && frame_type <= 254,
					"Invalid AppendFrame frame_type: {}",
					frame_type
				);

				info.write_u8(frame_type)?;
				info.write_u16(*offset_delta)?;
				for local in locals {
					local.write(cp, info)?;
				}
			}
			StackMapFrame::FullFrame {
				offset_delta,
				locals,
				stack,
			} => {
				info.write_u8(255)?;
				info.write_u16(*offset_delta)?;
				info.write_u16(locals.len().truncate())?;
				for local in locals {
					local.write(cp, info)?;
				}
				info.write_u16(stack.len().truncate())?;
				for s in stack {
					s.write(cp, info)?;
				}
			}
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct StackMapTableAttribute {
	pub entries: Vec<StackMapFrame>,
}

impl StackMapTableAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let entries_count = usize::from(
			buffer
				.read_u16()
				.wrap_err("failed to read number_of_entries from StackMapTable attribute")?,
		);
		let entries = buffer.read_vec_with(entries_count, |b| StackMapFrame::parse(b, cp))?;
		Ok(StackMapTableAttribute { entries })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.entries.len().truncate())?;
		for entry in &self.entries {
			entry.write(cp, info)?;
		}
		Ok(())
	}
}
