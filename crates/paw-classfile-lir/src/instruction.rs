use std::str::FromStr;

use byteorder::BigEndian;
use eyre::{OptionExt, Result, bail};
use paw_classfile_format::{CPTag, ext::ReadBytesExt};

use crate::{
	attribute::BootstrapMethod,
	class::{get_class_name_cp_entry, get_utf8_cp_entry},
	descriptor::{Descriptor, MethodDescriptor},
	method::LIRMethodHandle,
};

pub mod opcodes {
	pub const ACONST_NULL: u8 = 0x1;

	pub const BIPUSH: u8 = 0x10;

	pub const LDC: u8 = 0x12;
	pub const LDC_W: u8 = 0x13;
	pub const LDC2_W: u8 = 0x14;

	pub const ALOAD: u8 = 0x19;
	pub const ALOAD_0: u8 = 0x2A;
	pub const ALOAD_1: u8 = 0x2B;
	pub const ALOAD_2: u8 = 0x2C;
	pub const ALOAD_3: u8 = 0x2D;

	pub const ILOAD: u8 = 0x15;
	pub const ILOAD_0: u8 = 0x1A;
	pub const ILOAD_1: u8 = 0x1B;
	pub const ILOAD_2: u8 = 0x1C;
	pub const ILOAD_3: u8 = 0x1D;

	pub const ISTORE: u8 = 0x36;
	pub const ISTORE_0: u8 = 0x3B;
	pub const ISTORE_1: u8 = 0x3C;
	pub const ISTORE_2: u8 = 0x3D;
	pub const ISTORE_3: u8 = 0x3E;

	pub const IADD: u8 = 0x60;

	pub const ARETURN: u8 = 0xB0;
	pub const RETURN: u8 = 0xB1;

	pub const GET_STATIC: u8 = 0xB2;
	pub const PUT_FIELD: u8 = 0xB5;

	pub const INVOKE_VIRTUAL: u8 = 0xB6;
	pub const INVOKE_SPECIAL: u8 = 0xB7;
	pub const INVOKE_STATIC: u8 = 0xB8;
	pub const INVOKE_DYNAMIC: u8 = 0xBA;

	pub const MONITOREXIT: u8 = 0xC3;
}

#[derive(Debug, Clone)]
pub enum LIRLDCConstant {
	Int(i32),
	Long(i64),
	Float(f32),
	Double(f64),
	String(String),
	Class(String),
	MethodType(String),
	MethodHandle(LIRMethodHandle),
}

#[derive(Debug, Clone)]
pub enum Instruction {
	/// Push null
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.aconst_null
	AConstNull,

	/// Push byte
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.bipush
	BIPUSH(i8),

	/// Load reference from local variable
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.aload
	ALOAD {
		lvar_index: u8,
	},

	/// Push item from run-time constant pool
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.ldc
	LDC {
		constant: LIRLDCConstant,
	},

	/// Load int from local variable
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.iload
	ILOAD {
		lvar_index: u8,
	},

	/// Store int into local variable
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.istore
	ISTORE {
		lvar_index: u8,
	},

	/// Add int
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.iadd
	IADD,

	/// Return reference from method
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.areturn
	AReturn,

	/// Return void from method
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.return
	Return,

	/// Get static field from class
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.getstatic
	GetStatic {
		owner: String,
		name: String,
		descriptor: Descriptor,
	},
	/// Set field in object
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.putfield
	PutField {
		owner: String,
		name: String,
		descriptor: Descriptor,
	},

	/// Invoke instance method; dispatch based on class
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.invokevirtual
	InvokeVirtual {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
	},

	/// Invoke instance method; direct invocation of instance initialization methods and methods of the current class and its supertypes
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.invokespecial
	InvokeSpecial {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
		is_interface: bool,
	},

	/// Invoke a class (static) method
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.invokestatic
	InvokeStatic {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
		is_interface: bool,
	},

	/// Invoke a dynamically-computed call site
	/// https://docs.oracle.com/javase/specs///jvms/se25/html/jvms-6.html#jvms-6.5.invokedynamic
	InvokeDynamic {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
		is_interface: bool,
		bsm_args: Vec<CPTag>, // FIXME: Don't store CPTag directly, should be its own union, what tags are valid arguments?
	},

	MonitorExit,
}

impl Instruction {
	pub const fn mnemonic(&self) -> &'static str {
		match self {
			Instruction::AConstNull => "ACONST_NULL",
			Instruction::BIPUSH(_) => "BIPUSH",
			Instruction::LDC { .. } => "LDC",
			Instruction::ALOAD { .. } => "ALOAD",
			Instruction::ILOAD { .. } => "ILOAD",
			Instruction::ISTORE { .. } => "ISTORE",
			Instruction::IADD => "IADD",
			Instruction::GetStatic { .. } => "GETSTATIC",
			Instruction::PutField { .. } => "PUTFIELD",
			Instruction::InvokeVirtual { .. } => "INVOKEVIRTUAL",
			Instruction::InvokeSpecial { .. } => "INVOKESPECIAL",
			Instruction::InvokeStatic { .. } => "INVOKESTATIC",
			Instruction::InvokeDynamic { .. } => "INVOKEDYNAMIC",
			Instruction::AReturn => "ARETURN",
			Instruction::Return => "RETURN",
			Instruction::MonitorExit => "MONITOREXIT",
		}
	}

	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &[CPTag]) -> Result<Self> {
		let opcode = buffer.read_u8()?;
		Ok(match opcode {
			opcodes::ACONST_NULL => Instruction::AConstNull,
			opcodes::BIPUSH => Instruction::BIPUSH(buffer.read_i8()?),

			opcodes::LDC | opcodes::LDC_W => {
				let index = if opcode == opcodes::LDC {
					u16::from(buffer.read_u8()?)
				} else {
					buffer.read_u16::<BigEndian>()?
				};
				let tag = cp.get(index as usize - 1).ok_or_eyre("invalid cp index for ldc")?;
				let constant = match tag {
					CPTag::Integer(v) => LIRLDCConstant::Int(*v as i32),
					CPTag::Float(v) => LIRLDCConstant::Float(*v),
					CPTag::String { utf8_index } => LIRLDCConstant::String(get_utf8_cp_entry(cp, *utf8_index)?),
					CPTag::Class { name_index } => LIRLDCConstant::Class(get_utf8_cp_entry(cp, *name_index)?),
					CPTag::MethodType { descriptor_index } => {
						LIRLDCConstant::MethodType(get_utf8_cp_entry(cp, *descriptor_index)?)
					}
					CPTag::MethodHandle { .. } => LIRLDCConstant::MethodHandle(LIRMethodHandle::resolve(cp, index)?),
					_ => bail!("invalid tag for ldc: {:?}", tag),
				};
				Instruction::LDC { constant }
			}

			opcodes::LDC2_W => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp.get(index as usize - 1).ok_or_eyre("invalid cp index for ldc2_w")?;
				let constant = match tag {
					CPTag::Long(v) => LIRLDCConstant::Long(*v as i64),
					CPTag::Double(v) => LIRLDCConstant::Double(*v),
					_ => bail!("invalid tag for ldc2_w: {:?}", tag),
				};
				Instruction::LDC { constant }
			}

			opcodes::ALOAD => Instruction::ALOAD {
				lvar_index: buffer.read_u8()?,
			},
			opcodes::ALOAD_0 => Instruction::ALOAD { lvar_index: 0 },
			opcodes::ALOAD_1 => Instruction::ALOAD { lvar_index: 1 },
			opcodes::ALOAD_2 => Instruction::ALOAD { lvar_index: 2 },
			opcodes::ALOAD_3 => Instruction::ALOAD { lvar_index: 3 },

			opcodes::ILOAD => Instruction::ILOAD {
				lvar_index: buffer.read_u8()?,
			},
			opcodes::ILOAD_0 => Instruction::ILOAD { lvar_index: 0 },
			opcodes::ILOAD_1 => Instruction::ILOAD { lvar_index: 1 },
			opcodes::ILOAD_2 => Instruction::ILOAD { lvar_index: 2 },
			opcodes::ILOAD_3 => Instruction::ILOAD { lvar_index: 3 },

			opcodes::ISTORE => Instruction::ISTORE {
				lvar_index: buffer.read_u8()?,
			},
			opcodes::ISTORE_0 => Instruction::ISTORE { lvar_index: 0 },
			opcodes::ISTORE_1 => Instruction::ISTORE { lvar_index: 1 },
			opcodes::ISTORE_2 => Instruction::ISTORE { lvar_index: 2 },
			opcodes::ISTORE_3 => Instruction::ISTORE { lvar_index: 3 },

			opcodes::IADD => Instruction::IADD,

			opcodes::ARETURN => Instruction::AReturn,
			opcodes::RETURN => Instruction::Return,

			opcodes::GET_STATIC | opcodes::PUT_FIELD => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp
					.get(index as usize - 1)
					.ok_or_eyre("invalid cp index for getstatic")?;
				let (class_index, name_and_type_index) = match tag {
					CPTag::FieldRef {
						class_index,
						name_and_ty_index,
					} => (*class_index, *name_and_ty_index),
					tag => bail!("Invalid tag for getstatic: {:?}", tag),
				};
				let owner = get_class_name_cp_entry(cp, class_index)?;
				let name_and_type = cp
					.get(name_and_type_index as usize - 1)
					.ok_or_eyre("invalid name_and_type index")?;
				let (name, descriptor) = match name_and_type {
					CPTag::NameAndType {
						name_index,
						descriptor_index,
					} => (
						get_utf8_cp_entry(cp, *name_index)?,
						get_utf8_cp_entry(cp, *descriptor_index)?,
					),
					tag => bail!("Invalid tag for name_and_type: {:?}", tag),
				};
				let descriptor = Descriptor::from_str(&descriptor)?;
				match opcode {
					opcodes::GET_STATIC => Instruction::GetStatic {
						owner,
						name,
						descriptor,
					},
					opcodes::PUT_FIELD => Instruction::PutField {
						owner,
						name,
						descriptor,
					},
					_ => unreachable!(),
				}
			}

			opcodes::INVOKE_VIRTUAL => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp
					.get(index as usize - 1)
					.ok_or_eyre("invalid cp index for invokevirtual")?;
				let (class_index, name_and_type_index) = match tag {
					CPTag::MethodRef {
						class_index,
						name_and_ty_index,
					} => (*class_index, *name_and_ty_index),

					tag => bail!("Invalid tag for invokevirtual: {:?}", tag),
				};
				let owner = get_class_name_cp_entry(cp, class_index)?;
				let name_and_type = cp
					.get(name_and_type_index as usize - 1)
					.ok_or_eyre("invalid name_and_type index")?;
				let (name, descriptor) = match name_and_type {
					CPTag::NameAndType {
						name_index,
						descriptor_index,
					} => (
						get_utf8_cp_entry(cp, *name_index)?,
						get_utf8_cp_entry(cp, *descriptor_index)?,
					),
					tag => bail!("Invalid tag for name_and_type: {:?}", tag),
				};
				let descriptor = MethodDescriptor::from_str(&descriptor)?;
				Instruction::InvokeVirtual {
					owner,
					name,
					descriptor,
				}
			}

			opcodes::INVOKE_SPECIAL | opcodes::INVOKE_STATIC => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp
					.get(index as usize - 1)
					.ok_or_eyre("invalid cp index for invokespecial")?;
				let (class_index, name_and_type_index, is_interface) = match tag {
					CPTag::MethodRef {
						class_index,
						name_and_ty_index,
					} => (*class_index, *name_and_ty_index, false),
					CPTag::InterfaceMethodRef {
						class_index,
						name_and_ty_index,
					} => (*class_index, *name_and_ty_index, true),
					tag => bail!("Invalid tag for invokespecial: {:?}", tag),
				};
				let owner = get_class_name_cp_entry(cp, class_index)?;
				let name_and_type = cp
					.get(name_and_type_index as usize - 1)
					.ok_or_eyre("invalid name_and_type index")?;
				let (name, descriptor) = match name_and_type {
					CPTag::NameAndType {
						name_index,
						descriptor_index,
					} => (
						get_utf8_cp_entry(cp, *name_index)?,
						get_utf8_cp_entry(cp, *descriptor_index)?,
					),
					tag => bail!("Invalid tag for name_and_type: {:?}", tag),
				};
				let descriptor = MethodDescriptor::from_str(&descriptor)?;

				match opcode {
					opcodes::INVOKE_SPECIAL => Instruction::InvokeSpecial {
						owner,
						name,
						descriptor,
						is_interface,
					},
					opcodes::INVOKE_STATIC => Instruction::InvokeStatic {
						owner,
						name,
						descriptor,
						is_interface,
					},
					_ => unreachable!(),
				}
			}
			opcodes::INVOKE_DYNAMIC => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp
					.get(index as usize - 1)
					.ok_or_eyre("invalid cp index for invokedynamic")?;
				let (bootstrap_method_attr_index, name_and_type_index) = match tag {
					CPTag::InvokeDynamic {
						bootstrap_method_attr_index,
						name_and_ty_index,
					} => (*bootstrap_method_attr_index, *name_and_ty_index),
					tag => bail!("Invalid tag for method handle: {:?}", tag),
				};
				todo!("get bsm from class");
				let name_and_type = cp
					.get(name_and_type_index as usize - 1)
					.ok_or_eyre("invalid name_and_type index")?;
				let (name, descriptor) = match name_and_type {
					CPTag::NameAndType {
						name_index,
						descriptor_index,
					} => (
						get_utf8_cp_entry(cp, *name_index)?,
						get_utf8_cp_entry(cp, *descriptor_index)?,
					),
					tag => bail!("Invalid tag for name_and_type: {:?}", tag),
				};
				let descriptor = MethodDescriptor::from_str(&descriptor)?;
				todo!()
			}

			opcodes::MONITOREXIT => Instruction::MonitorExit,

			opcode => bail!("Unrecognized opcode: {} : 0x{:X}", opcode, opcode),
		})
	}
}
