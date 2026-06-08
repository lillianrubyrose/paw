use std::{collections::HashMap, io::Cursor};

use eyre::{bail, Context, Result};
use num_conv::{Extend, Truncate};
use paw_classfile::{
	constant_pool::{ConstantPool, MethodTypeTag},
	descriptor::{Descriptor, MethodDescriptor},
	AttributeInfo, CPTag, InnerClassAccessFlags, ModuleAccessFlags, ModuleExportAccessFlags, ModuleOpenAccessFlags,
	ModuleRequireAccessFlags, ParameterAccessFlags,
};

use crate::{
	instruction::{Instruction, LIRLabel, LIRResolvedLabel},
	method::{LIRHandleDescriptor, LIRMethodHandle},
};

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
pub enum StackMapFrame {
	SameFrame {
		frame_type: SameFrameType,
		pc: LIRResolvedLabel,
	},
	SameLocals1StackItemFrame {
		frame_type: SameLocals1StackItemFrameType,
		stack: VerificationTypeInfo,
		pc: LIRResolvedLabel,
	},
	SameLocals1StackItemFrameExtended {
		stack: VerificationTypeInfo,
		pc: LIRResolvedLabel,
	},
	ChopFrame {
		absent_locals_count: ChopFrameAbsentLocals,
		pc: LIRResolvedLabel,
	},
	SameFrameExtended {
		pc: LIRResolvedLabel,
	},
	AppendFrame {
		locals: AppendFrameLocals,
		pc: LIRResolvedLabel,
	},
	FullFrame {
		locals: Vec<VerificationTypeInfo>,
		stack: Vec<VerificationTypeInfo>,
		pc: LIRResolvedLabel,
	},
}

impl StackMapFrame {
	#[must_use]
	pub const fn pc(&self) -> LIRResolvedLabel {
		match self {
			StackMapFrame::SameFrame { pc, .. }
			| StackMapFrame::SameLocals1StackItemFrame { pc, .. }
			| StackMapFrame::SameLocals1StackItemFrameExtended { pc, .. }
			| StackMapFrame::ChopFrame { pc, .. }
			| StackMapFrame::SameFrameExtended { pc, .. }
			| StackMapFrame::AppendFrame { pc, .. }
			| StackMapFrame::FullFrame { pc, .. } => *pc,
		}
	}

	pub fn parse<B: BytesReadExt>(
		buffer: &mut B,
		cp: &ConstantPool,
		allocate_label: &mut impl FnMut(u32) -> LIRResolvedLabel,
		prev_frame_pc: u32,
	) -> Result<(Self, u32)> {
		let frame_type = buffer.read_u8()?;
		let frame = match frame_type {
			0..=63 => {
				let offset_delta = frame_type as u32;
				let absolute_pc = prev_frame_pc.wrapping_add(offset_delta).wrapping_add(1);
				(
					Self::SameFrame {
						frame_type: SameFrameType::new(frame_type)?,
						pc: allocate_label(absolute_pc),
					},
					absolute_pc,
				)
			}
			64..=127 => {
				let offset_delta = (frame_type - 64) as u32;
				let absolute_pc = prev_frame_pc.wrapping_add(offset_delta).wrapping_add(1);
				(
					Self::SameLocals1StackItemFrame {
						frame_type: SameLocals1StackItemFrameType::new(frame_type)?,
						pc: allocate_label(absolute_pc),
						stack: VerificationTypeInfo::parse(buffer, cp, allocate_label)?,
					},
					absolute_pc,
				)
			}
			247 => {
				let offset_delta = buffer.read_u16()? as u32;
				let absolute_pc = prev_frame_pc.wrapping_add(offset_delta).wrapping_add(1);
				(
					Self::SameLocals1StackItemFrameExtended {
						pc: allocate_label(absolute_pc),
						stack: VerificationTypeInfo::parse(buffer, cp, allocate_label)?,
					},
					absolute_pc,
				)
			}
			248..=250 => {
				let offset_delta = buffer.read_u16()? as u32;
				let absolute_pc = prev_frame_pc.wrapping_add(offset_delta).wrapping_add(1);
				(
					Self::ChopFrame {
						absent_locals_count: ChopFrameAbsentLocals::new(251 - frame_type)?,
						pc: allocate_label(absolute_pc),
					},
					absolute_pc,
				)
			}
			251 => {
				let offset_delta = buffer.read_u16()? as u32;
				let absolute_pc = prev_frame_pc.wrapping_add(offset_delta).wrapping_add(1);
				(
					Self::SameFrameExtended {
						pc: allocate_label(absolute_pc),
					},
					absolute_pc,
				)
			}
			252..=254 => {
				let offset_delta = buffer.read_u16()? as u32;
				let absolute_pc = prev_frame_pc.wrapping_add(offset_delta).wrapping_add(1);

				let n_locals = (frame_type - 251) as usize;
				let locals = buffer.read_vec_with(n_locals, |b| VerificationTypeInfo::parse(b, cp, allocate_label))?;
				(
					Self::AppendFrame {
						pc: allocate_label(absolute_pc),
						locals: AppendFrameLocals::from_vec(locals)?,
					},
					absolute_pc,
				)
			}
			255 => {
				let offset_delta = buffer.read_u16()? as u32;
				let absolute_pc = prev_frame_pc.wrapping_add(offset_delta).wrapping_add(1);

				let n_locals = buffer.read_u16()? as usize;
				let mut locals = Vec::with_capacity(n_locals);
				for _ in 0..n_locals {
					locals.push(VerificationTypeInfo::parse(buffer, cp, allocate_label)?);
				}

				let n_stack = buffer.read_u16()? as usize;
				let mut stack = Vec::with_capacity(n_stack);
				for _ in 0..n_stack {
					stack.push(VerificationTypeInfo::parse(buffer, cp, allocate_label)?);
				}

				(
					Self::FullFrame {
						pc: allocate_label(absolute_pc),
						locals,
						stack,
					},
					absolute_pc,
				)
			}
			_ => bail!("invalid frame tag {frame_type}"),
		};
		Ok(frame)
	}

	pub fn write<W: BytesWriteExt>(
		&self,
		cp: &mut ConstantPool,
		info: &mut W,
		label_positions: &HashMap<LIRResolvedLabel, u32>,
		prev_frame_pc: u32,
	) -> Result<()> {
		let frame_pc = *label_positions
			.get(&self.pc())
			.ok_or_else(|| unreachable!("label was referenced but not attached to an instruction"))?;

		// offset_delta = frame_pc - prev_frame_pc - 1
		let offset_delta = frame_pc.wrapping_sub(prev_frame_pc).wrapping_sub(1);
		if frame_pc <= prev_frame_pc && prev_frame_pc != u32::MAX {
			bail!(
				"bad stack map frame ordering: frame at {} cannot follow frame at {}",
				frame_pc,
				prev_frame_pc
			);
		}

		match self {
			StackMapFrame::SameFrame { frame_type, .. } => {
				// offset_delta must fit in 0..=63 for SameFrame
				if offset_delta > 63 {
					bail!(
						"offset_delta {} too large for SameFrame (max 63), use SameFrameExtended",
						offset_delta
					);
				}
				info.write_u8(offset_delta.truncate())?;
			}
			StackMapFrame::SameLocals1StackItemFrame { stack, .. } => {
				// offset_delta must fit in 0..=63 for SameLocals1StackItemFrame
				if offset_delta > 63 {
					bail!(
						"offset_delta {} too large for SameLocals1StackItemFrame (max 63), use Extended variant",
						offset_delta
					);
				}
				info.write_u8((64 + offset_delta).truncate())?;
				stack.write(cp, info, label_positions)?;
			}
			StackMapFrame::SameLocals1StackItemFrameExtended { stack, .. } => {
				info.write_u8(247)?;
				info.write_u16::<BigEndian>(offset_delta.truncate())?;
				stack.write(cp, info, label_positions)?;
			}
			StackMapFrame::ChopFrame {
				absent_locals_count: ChopFrameAbsentLocals(absent_locals_count),
				..
			} => {
				let frame_type = 251_u8
					.checked_sub(*absent_locals_count)
					.ok_or_else(|| eyre::eyre!("Invalid chopframe absent locals count: {}", absent_locals_count))?;

				info.write_u8(frame_type)?;
				info.write_u16::<BigEndian>(offset_delta.truncate())?;
			}
			StackMapFrame::SameFrameExtended { .. } => {
				info.write_u8(251)?;
				info.write_u16::<BigEndian>(offset_delta.truncate())?;
			}
			StackMapFrame::AppendFrame {
				locals: AppendFrameLocals(locals),
				..
			} => {
				let n_locals = locals.len().truncate::<u8>();
				let frame_type = 251 + n_locals;
				debug_assert!(
					frame_type >= 252 && frame_type <= 254,
					"Invalid AppendFrame frame_type: {}",
					frame_type
				);

				info.write_u8(frame_type)?;
				info.write_u16::<BigEndian>(offset_delta.truncate())?;
				for local in locals {
					local.write(cp, info, label_positions)?;
				}
			}
			StackMapFrame::FullFrame { locals, stack, .. } => {
				info.write_u8(255)?;
				info.write_u16::<BigEndian>(offset_delta.truncate())?;
				info.write_u16::<BigEndian>(locals.len().truncate())?;
				for local in locals {
					local.write(cp, info, label_positions)?;
				}
				info.write_u16::<BigEndian>(stack.len().truncate())?;
				for s in stack {
					s.write(cp, info, label_positions)?;
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
	pub fn parse<B: BytesReadExt>(
		buffer: &mut B,
		cp: &ConstantPool,
		allocate_label: &mut impl FnMut(u32) -> LIRResolvedLabel,
	) -> Result<Self> {
		let entries_count = usize::from(
			buffer
				.read_u16()
				.wrap_err("failed to read number_of_entries from StackMapTable attribute")?,
		);

		let mut entries = Vec::with_capacity(entries_count);
		let mut prev_frame_pc = u32::MAX;

		for _ in 0..entries_count {
			let (frame, absolute_pc) = StackMapFrame::parse(buffer, cp, allocate_label, prev_frame_pc)?;
			prev_frame_pc = absolute_pc;
			entries.push(frame);
		}

		Ok(StackMapTableAttribute { entries })
	}

	pub fn write<W: BytesWriteExt>(
		&self,
		cp: &mut ConstantPool,
		info: &mut W,
		label_positions: &HashMap<LIRResolvedLabel, u32>,
	) -> Result<()> {
		info.write_u16::<BigEndian>(self.entries.len().truncate())?;

		let mut prev_frame_pc = u32::MAX;
		for entry in &self.entries {
			entry.write(cp, info, label_positions, prev_frame_pc)?;
			prev_frame_pc = *label_positions
				.get(&entry.pc())
				.ok_or_else(|| unreachable!("label was referenced but not attached to an instruction"))?;
		}
		Ok(())
	}
}
