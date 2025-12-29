use std::str::FromStr;

use byteorder::BigEndian;
use eyre::{OptionExt, Result, bail, eyre};
use paw_classfile_format::{
	CPTag,
	class_pool::{ClassTag, ConstantPool, InterfaceMethodRefTag, MethodRefTag, MethodTypeTag, StringTag},
	descriptor::{Descriptor, MethodDescriptor},
	ext::ReadBytesExt,
};

use crate::{
	attribute::{BootstrapMethodArgument, LIRClassAttribute},
	method::LIRMethodHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ArrayType {
	Boolean = 4,
	Char = 5,
	Float = 6,
	Double = 7,
	Byte = 8,
	Short = 9,
	Int = 10,
	Long = 11,
}

impl TryFrom<u8> for ArrayType {
	type Error = eyre::Report;

	fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
		match value {
			4 => Ok(ArrayType::Boolean),
			5 => Ok(ArrayType::Char),
			6 => Ok(ArrayType::Float),
			7 => Ok(ArrayType::Double),
			8 => Ok(ArrayType::Byte),
			9 => Ok(ArrayType::Short),
			10 => Ok(ArrayType::Int),
			11 => Ok(ArrayType::Long),
			_ => Err(eyre!("invalid array primitive type {}", value)),
		}
	}
}

pub mod opcodes {
	pub const AALOAD: u8 = 0x32;
	pub const AASTORE: u8 = 0x53;

	pub const ACONST_NULL: u8 = 0x1;

	pub const ALOAD: u8 = 0x19;
	pub const ALOAD_0: u8 = 0x2A;
	pub const ALOAD_1: u8 = 0x2B;
	pub const ALOAD_2: u8 = 0x2C;
	pub const ALOAD_3: u8 = 0x2D;

	pub const ANEWARRAY: u8 = 0xBD;
	pub const ARETURN: u8 = 0xB0;
	pub const ARRAYLENGTH: u8 = 0xBE;

	pub const ASTORE: u8 = 0x3A;
	pub const ASTORE_0: u8 = 0x4B;
	pub const ASTORE_1: u8 = 0x4C;
	pub const ASTORE_2: u8 = 0x4D;
	pub const ASTORE_3: u8 = 0x4E;

	pub const ATHROW: u8 = 0xBF;

	pub const BALOAD: u8 = 0x33;
	pub const BASTORE: u8 = 0x54;
	pub const BIPUSH: u8 = 0x10;
	pub const CALOAD: u8 = 0x34;
	pub const CASTORE: u8 = 0x55;
	pub const CHECKCAST: u8 = 0xC0;
	pub const D2F: u8 = 0x90;
	pub const D2I: u8 = 0x8E;
	pub const D2L: u8 = 0x8F;
	pub const DADD: u8 = 0x63;
	pub const DALOAD: u8 = 0x31;
	pub const DASTORE: u8 = 0x52;
	pub const DCMPL: u8 = 0x97;
	pub const DCMPG: u8 = 0x98;

	pub const DCONST_0: u8 = 0x0E;
	pub const DCONST_1: u8 = 0x0F;

	pub const DDIV: u8 = 0x6F;

	pub const DLOAD: u8 = 0x18;
	pub const DLOAD_0: u8 = 0x26;
	pub const DLOAD_1: u8 = 0x27;
	pub const DLOAD_2: u8 = 0x28;
	pub const DLOAD_3: u8 = 0x29;

	pub const DMUL: u8 = 0x6B;
	pub const DNEG: u8 = 0x77;
	pub const DREM: u8 = 0x73;
	pub const DRETURN: u8 = 0xAF;

	pub const DSTORE: u8 = 0x39;
	pub const DSTORE_0: u8 = 0x47;
	pub const DSTORE_1: u8 = 0x48;
	pub const DSTORE_2: u8 = 0x49;
	pub const DSTORE_3: u8 = 0x4A;

	pub const DSUB: u8 = 0x67;
	pub const DUP: u8 = 0x59;
	pub const DUP_X1: u8 = 0x5A;
	pub const DUP_X2: u8 = 0x5B;
	pub const DUP2: u8 = 0x5C;
	pub const DUP2_X1: u8 = 0x5D;
	pub const DUP2_X2: u8 = 0x5E;

	pub const F2D: u8 = 0x8D;
	pub const F2I: u8 = 0x8B;
	pub const F2L: u8 = 0x8C;
	pub const FADD: u8 = 0x62;
	pub const FALOAD: u8 = 0x30;
	pub const FASTORE: u8 = 0x51;
	pub const FCMPL: u8 = 0x95;
	pub const FCMPG: u8 = 0x96;

	pub const FCONST_0: u8 = 0x0B;
	pub const FCONST_1: u8 = 0x0C;
	pub const FCONST_2: u8 = 0x0D;

	pub const FDIV: u8 = 0x6E;

	pub const FLOAD: u8 = 0x17;
	pub const FLOAD_0: u8 = 0x22;
	pub const FLOAD_1: u8 = 0x23;
	pub const FLOAD_2: u8 = 0x24;
	pub const FLOAD_3: u8 = 0x25;

	pub const FMUL: u8 = 0x6A;
	pub const FNEG: u8 = 0x76;
	pub const FREM: u8 = 0x72;
	pub const FRETURN: u8 = 0xAE;

	pub const FSTORE: u8 = 0x38;
	pub const FSTORE_0: u8 = 0x43;
	pub const FSTORE_1: u8 = 0x44;
	pub const FSTORE_2: u8 = 0x45;
	pub const FSTORE_3: u8 = 0x46;

	pub const FSUB: u8 = 0x66;

	pub const GETFIELD: u8 = 0xB4;
	pub const GETSTATIC: u8 = 0xB2;
	pub const GOTO: u8 = 0xA7;
	pub const GOTO_W: u8 = 0xC8;

	pub const I2B: u8 = 0x91;
	pub const I2C: u8 = 0x92;
	pub const I2D: u8 = 0x87;
	pub const I2F: u8 = 0x86;
	pub const I2L: u8 = 0x85;
	pub const I2S: u8 = 0x93;

	pub const IADD: u8 = 0x60;
	pub const IALOAD: u8 = 0x2E;
	pub const IAND: u8 = 0x7E;
	pub const IASTORE: u8 = 0x4F;

	pub const ICONST_M1: u8 = 0x2;
	pub const ICONST_0: u8 = 0x3;
	pub const ICONST_1: u8 = 0x4;
	pub const ICONST_2: u8 = 0x5;
	pub const ICONST_3: u8 = 0x6;
	pub const ICONST_4: u8 = 0x7;
	pub const ICONST_5: u8 = 0x8;

	pub const IDIV: u8 = 0x6C;

	pub const IF_ACMPEQ: u8 = 0xA5;
	pub const IF_ACMPNE: u8 = 0xA6;

	pub const IF_ICMPEQ: u8 = 0x9F;
	pub const IF_ICMPNE: u8 = 0xA0;
	pub const IF_ICMPLT: u8 = 0xA1;
	pub const IF_ICMPGE: u8 = 0xA2;
	pub const IF_ICMPGT: u8 = 0xA3;
	pub const IF_ICMPLE: u8 = 0xA4;

	pub const IFEQ: u8 = 0x99;
	pub const IFNE: u8 = 0x9A;
	pub const IFLT: u8 = 0x9B;
	pub const IFGE: u8 = 0x9C;
	pub const IFGT: u8 = 0x9D;
	pub const IFLE: u8 = 0x9E;

	pub const IFNONNULL: u8 = 0xC7;
	pub const IFNULL: u8 = 0xC6;

	pub const IINC: u8 = 0x84;

	pub const ILOAD: u8 = 0x15;
	pub const ILOAD_0: u8 = 0x1A;
	pub const ILOAD_1: u8 = 0x1B;
	pub const ILOAD_2: u8 = 0x1C;
	pub const ILOAD_3: u8 = 0x1D;

	pub const IMUL: u8 = 0x68;
	pub const INEG: u8 = 0x74;

	pub const INSTANCEOF: u8 = 0xC1;

	pub const INVOKE_DYNAMIC: u8 = 0xBA;
	pub const INVOKE_INTERFACE: u8 = 0xB9;
	pub const INVOKE_SPECIAL: u8 = 0xB7;
	pub const INVOKE_STATIC: u8 = 0xB8;
	pub const INVOKE_VIRTUAL: u8 = 0xB6;

	pub const IOR: u8 = 0x80;
	pub const IREM: u8 = 0x70;

	pub const IRETURN: u8 = 0xAC;

	pub const ISHL: u8 = 0x78;
	pub const ISHR: u8 = 0x7A;

	pub const ISTORE: u8 = 0x36;
	pub const ISTORE_0: u8 = 0x3B;
	pub const ISTORE_1: u8 = 0x3C;
	pub const ISTORE_2: u8 = 0x3D;
	pub const ISTORE_3: u8 = 0x3E;

	pub const ISUB: u8 = 0x64;
	pub const IUSHR: u8 = 0x7C;
	pub const IXOR: u8 = 0x82;

	pub const JSR: u8 = 0xA8;
	pub const JSR_W: u8 = 0xC9;

	pub const L2D: u8 = 0x8A;
	pub const L2F: u8 = 0x89;
	pub const L2I: u8 = 0x88;

	pub const LADD: u8 = 0x61;

	pub const LALOAD: u8 = 0x2F;

	pub const LAND: u8 = 0x7F;

	pub const LASTORE: u8 = 0x50;

	pub const LCMP: u8 = 0x94;

	pub const LCONST_0: u8 = 0x09;
	pub const LCONST_1: u8 = 0x0A;

	pub const LDC: u8 = 0x12;
	pub const LDC_W: u8 = 0x13;
	pub const LDC2_W: u8 = 0x14;

	pub const LDIV: u8 = 0x6D;

	pub const LLOAD: u8 = 0x16;
	pub const LLOAD_0: u8 = 0x1E;
	pub const LLOAD_1: u8 = 0x1F;
	pub const LLOAD_2: u8 = 0x20;
	pub const LLOAD_3: u8 = 0x21;

	pub const LMUL: u8 = 0x69;
	pub const LNEG: u8 = 0x75;

	pub const LOOKUPSWITCH: u8 = 0xAB;

	pub const LOR: u8 = 0x81;
	pub const LREM: u8 = 0x71;

	pub const LRETURN: u8 = 0xAD;

	pub const LSHL: u8 = 0x79;
	pub const LSHR: u8 = 0x7B;

	pub const LSTORE: u8 = 0x37;
	pub const LSTORE_0: u8 = 0x3F;
	pub const LSTORE_1: u8 = 0x40;
	pub const LSTORE_2: u8 = 0x41;
	pub const LSTORE_3: u8 = 0x42;

	pub const LSUB: u8 = 0x65;
	pub const LUSHR: u8 = 0x7D;
	pub const LXOR: u8 = 0x83;

	pub const MONITORENTER: u8 = 0xC2;
	pub const MONITOREXIT: u8 = 0xC3;

	pub const MULTIANEWARRAY: u8 = 0xC5;
	pub const NEW: u8 = 0xBB;
	pub const NEWARRAY: u8 = 0xBC;

	pub const NOP: u8 = 0x00;

	pub const POP: u8 = 0x57;
	pub const POP2: u8 = 0x58;

	pub const PUTFIELD: u8 = 0xB5;
	pub const PUTSTATIC: u8 = 0xB3;

	pub const RET: u8 = 0xA9;
	pub const RETURN: u8 = 0xB1;

	pub const SALOAD: u8 = 0x35;
	pub const SASTORE: u8 = 0x56;

	pub const SIPUSH: u8 = 0x11;
	pub const SWAP: u8 = 0x5F;

	pub const TABLESWITCH: u8 = 0xAA;

	pub const WIDE: u8 = 0xC4;
}

#[derive(Debug, Clone, Copy)]
pub struct LIRResolvedLabel(u32);

impl LIRResolvedLabel {
	/// This method performs no unsafe actions and is only unsafe for semantic reasoning
	pub const unsafe fn new_unchecked(id: u32) -> Self {
		Self(id)
	}
}

#[derive(Debug, Clone, Copy)]
pub enum LIRLabel {
	/// An unresolved label contains the address (absoltute bytecode index / pc) of its target instruction.
	/// This is used for labels that are not yet resolved and need to be resolved after all instructions have been parsed.
	Unresolved(i32),

	/// A resolved label has an internal id used for state keeping and comparison
	/// Labels stored among instructions should ALWAYS be resolved and the library will panic otherwise.
	Resolved(LIRResolvedLabel),
}

impl LIRLabel {
	pub const fn is_resolved(&self) -> bool {
		match self {
			LIRLabel::Unresolved(_) => false,
			LIRLabel::Resolved(_) => true,
		}
	}

	pub const fn is_unresolved(&self) -> bool {
		match self {
			LIRLabel::Unresolved(_) => true,
			LIRLabel::Resolved(_) => false,
		}
	}
}

#[derive(Debug, Clone)]
pub enum LIRLDCConstant {
	Int(i32),
	Long(i64),
	Float(f32),
	Double(f64),
	String(String),
	Class(String),
	MethodType(MethodDescriptor),
	MethodHandle(LIRMethodHandle),
}

#[derive(Debug, Clone)]
pub enum Instruction {
	/// Load reference from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.aaload
	AALoad,

	/// Store reference to array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.aastore
	AAStore,

	/// Push null
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.aconst_null
	AConstNull,

	/// Load reference from local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.aload
	/// NOTE: we eagerly expand aload_{0,1,2,3} to this form.
	ALoad {
		local_idx: u16,
	},

	/// Create a new array of refrence
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.anewarray
	ANewArray {
		element_ty: String,
	},

	/// Return reference from method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.areturn
	AReturn,

	/// Get length of array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.arraylength
	ArrayLength,

	/// Store reference into local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.astore
	/// NOTE: we eagerly expand astore_{0,1,2,3} to this form.
	AStore {
		local_idx: u16,
	},

	/// Throw exception or error
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.athrow
	AThrow,

	/// Load byte or boolean from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.baload
	BALoad,

	/// Store into byte or boolean array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.bastore
	BAStore,

	/// Push byte
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.bipush
	BIPush(i8),

	/// Load char from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.caload
	CALoad,

	/// Store into char array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.castore
	CAStore,

	/// Check whether object is of given type
	CheckCast {
		object_ty: String,
	},

	/// Convert double to float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.d2f
	D2F,

	/// Convert double to int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.d2i
	D2I,

	/// Convert double to long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.d2l
	D2L,

	/// Add double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dadd
	DAdd,

	/// Load double from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.daload
	DALoad,

	/// Store into double array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dastore
	DAStore,

	/// Compare double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dcmp_op
	DCmpL,
	DCmpG,

	/// Push double 0
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dconst_d
	DConst0,
	/// Push double 1
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dconst_d
	DConst1,

	/// Divide double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ddiv
	DDiv,

	/// Load double from local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dload
	/// NOTE: we eagerly expand dload_{0,1,2,3} to this form.
	DLoad {
		local_idx: u16,
	},

	/// Multiply double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dmul
	DMul,

	/// Negate double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dneg
	DNeg,

	/// Remainder double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.drem
	DRem,

	/// Return double from method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dreturn
	DReturn,

	/// Store double into local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dstore
	/// NOTE: we eagerly expand dstore_{0,1,2,3} to this form.
	DStore {
		local_idx: u16,
	},

	/// Subtract double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dsub
	DSub,

	/// Duplicate the top operand stack value
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dup
	Dup,

	/// Duplicate the top operand stack value and insert two values down
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dup_x1
	DupX1,

	/// Duplicate the top operand stack value and insert two or three values down
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dup_x1
	DupX2,

	/// Duplicate the top one or two operand stack values
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dup2
	Dup2,

	/// Duplicate the top one or two operand stack values and insert two or three values down
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dup2_x1
	Dup2X1,

	/// Duplicate the top one or two operand stack values and insert two, three, or four values down
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.dup2_x2
	Dup2X2,

	/// Convert float to double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.f2d
	F2D,

	/// Convert float to int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.f2i
	F2I,

	/// Convert float to long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.f2l
	F2L,

	/// Add float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fadd
	FAdd,

	/// Load float from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.faload
	FALoad,

	/// Store into float array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fastore
	FAStore,

	/// Compare float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fcmp_op
	FCmpL,
	FCmpG,

	/// Push float constant
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fconst_f
	FConst0,
	FConst1,
	FConst2,

	/// Divide float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fdiv
	FDiv,

	/// Load float from local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fload
	/// NOTE: we eagerly expand fload_{0,1,2,3} to this form.
	FLoad {
		local_idx: u16,
	},

	/// Multiply float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fmul
	FMul,

	/// Negate float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fneg
	FNeg,

	/// Remainder float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.frem
	FRem,

	/// Return float from method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.freturn
	FReturn,

	/// Store float into local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fstore
	/// NOTE: we eagerly expand fstore_{0,1,2,3} to this form.
	FStore {
		local_idx: u16,
	},

	/// Subtract float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.fsub
	FSub,

	/// Fetch field from object
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.getfield
	GetField {
		class: String,
		name: String,
		descriptor: Descriptor,
	},

	/// Get static field from class
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.getstatic
	GetStatic {
		owner: String,
		name: String,
		descriptor: Descriptor,
	},

	/// Branch always
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.goto
	Goto {
		target: LIRLabel,
	},

	/// Branch always (wide index)
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.goto_w
	GotoW {
		target: LIRLabel,
	},

	/// Convert int to byte
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.i2b
	I2B,

	/// Convert int to char
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.i2c
	I2C,

	/// Convert int to double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.i2d
	I2D,

	/// Convert int to float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.i2f
	I2F,

	/// Convert int to long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.i2l
	I2L,

	/// Convert int to short
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.i2s
	I2S,

	/// Add int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iadd
	IAdd,

	/// Load int from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iaload
	IALoad,

	/// Bitwise AND int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iand
	IAnd,

	/// Store into int array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iastore
	IAStore,

	/// Push int constant
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iconst_i
	/// NOTE: we expand the iconst_<i> forms eagerly
	IConst {
		val: i8, // -1..=5
	},

	/// Divide int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.idiv
	IDiv,

	/// Branch if reference comparison succeeds
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.if_acmp_cond
	IfACmpEq {
		target: LIRLabel,
	},
	IfACmpNe {
		target: LIRLabel,
	},

	/// Branch if int comparison succeeds
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.if_icmp_cond
	IfICmpEq {
		target: LIRLabel,
	},
	IfICmpNe {
		target: LIRLabel,
	},
	IfICmpLt {
		target: LIRLabel,
	},
	IfICmpGt {
		target: LIRLabel,
	},
	IfICmpLe {
		target: LIRLabel,
	},
	IfICmpGe {
		target: LIRLabel,
	},

	/// Branch if int comparison with zero succeeds
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.if_cond
	IfEq {
		target: LIRLabel,
	},
	IfNe {
		target: LIRLabel,
	},
	IfLt {
		target: LIRLabel,
	},
	IfGt {
		target: LIRLabel,
	},
	IfLe {
		target: LIRLabel,
	},
	IfGe {
		target: LIRLabel,
	},

	/// Branch if reference not null
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ifnonnull
	IfNonNull {
		target: LIRLabel,
	},

	/// Branch if reference is null
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ifnull
	IfNull {
		target: LIRLabel,
	},

	/// Increment local variable by constant
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iinc
	IInc {
		local_index: u16,
		val: i16,
	},

	/// Load int from local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iload
	/// NOTE: we eagerly expand the iload_<N> forms
	ILoad {
		local_idx: u16,
	},

	/// Multiply int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.imul
	IMul,

	/// Negate int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ineg
	INeg,

	/// Determine if object is of given type
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.instanceof
	InstanceOf {
		class_type: String,
	},

	/// Invoke a dynamically-computed call site
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.invokedynamic
	InvokeDynamic {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
		is_interface: bool,
		bsm_args: Vec<BootstrapMethodArgument>,
	},

	/// Invoke interface method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.invokeinterface
	InvokeInterface {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
		count: u8,
	},

	/// Invoke instance method; direct invocation of instance initialization methods and methods of the current class and its supertypes
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.invokespecial
	InvokeSpecial {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
		is_interface: bool,
	},

	/// Invoke a class (static) method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.invokestatic
	InvokeStatic {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
		is_interface: bool,
	},

	/// Invoke instance method; dispatch based on class
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.invokevirtual
	InvokeVirtual {
		owner: String,
		name: String,
		descriptor: MethodDescriptor,
	},

	/// Bitwise OR int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ior
	IOr,

	/// Remainder int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.irem
	IRem,

	/// Return int from method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ireturn
	IReturn,

	/// Shift left int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ishl
	IShl,

	/// Arithmetic shift right int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ishr
	IShr,

	/// Store int into local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.istore
	/// NOTE: we eagerly expand the istore_<N> forms
	IStore {
		local_idx: u16,
	},

	/// Subtract int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.isub
	ISub,

	/// Logical shift right int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.iushr
	IUShr,

	/// Bitwise XOR int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ixor
	IXor,

	/// Jump subroutine
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.jsr
	Jsr {
		target: LIRLabel,
	},

	/// Jump subroutine (wide index)
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.jsr_w
	JsrW {
		target: LIRLabel,
	},

	/// Convert long to double
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.l2d
	LongToDouble,

	/// Convert long to float
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.l2f
	LongToFloat,

	/// Convert long to int
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.l2i
	LongToInt,

	/// Add long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ladd
	LAdd,

	/// Load long from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.laload
	LALoad,

	/// Bitwise AND long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.land
	LAnd,

	/// Store into long array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lastore
	LAStore,

	/// Compare long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lcmp
	LCmp,

	/// Push long constant
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lconst_l
	LConst {
		value: i64, // 0..=1
	},

	/// Push item from run-time constant pool
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ldc
	Ldc {
		constant: LIRLDCConstant,
	},

	/// Divide long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ldiv
	LDiv,

	/// Load long from local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lload
	/// NOTE: we eagerly expand the lload_<N> forms
	LLoad {
		local_idx: u8,
	},

	/// Multiply long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lmul
	LMul,

	/// Negate long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lneg
	LNeg,

	/// Access jump table by key match and jump
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lookupswitch
	LookupSwitch {
		default_target: LIRLabel,
		/// List of (match key, target_label)
		pairs: Vec<(i32, LIRLabel)>,
	},

	/// Bitwise OR long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lor
	LOr,

	/// Remainder long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lrem
	LRem,

	/// Return long from method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lreturn
	LReturn,

	/// Shift left long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lshl
	LShl,

	/// Arithmetic shift right long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lshr
	LShr,

	/// Store long into local variable
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lstore
	/// NOTE: we eagerly expand the lstore_<N> forms
	LStore {
		local_idx: u16,
	},

	/// Subtract long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lsub
	LSub,

	/// Logical shift right long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lushr
	LUShr,

	/// Bitwise XOR long
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.lxor
	LXor,

	/// Enter monitor for object
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.monitorenter
	MonitorEnter,

	/// Exit monitor for object
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.monitorexit
	MonitorExit,

	/// Create new multidimensional array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.multianewarray
	MultiANewArray {
		element_ty: String,
		dimensions: u8,
	},

	/// Create new object
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.new
	New {
		object_ty: String,
	},

	/// Create new array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.newarray
	NewArray {
		ty: ArrayType,
	},

	/// Do nothing
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.nop
	Nop,

	/// Pop the top operand stack value
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.pop
	Pop,

	/// Pop the top one or two operand stack values
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.nop2
	Pop2,

	/// Set field in object
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.putfield
	PutField {
		owner: String,
		name: String,
		descriptor: Descriptor,
	},

	/// Set static field in class
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.putstatic
	PutStatic {
		owner: String,
		name: String,
		descriptor: Descriptor,
	},

	/// Return from subroutine
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.ret
	Ret {
		local_idx: u16,
	},

	/// Return void from method
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.return
	Return,

	/// Load short from array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.saload
	SALoad,

	/// Store into short array
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.sastore
	SAStore,

	/// Push short
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.sipush
	SIPush {
		val: i16,
	},

	/// Swap the top two operand stack values
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.swap
	Swap,

	/// Access jump table by index and jump
	/// https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-6.html#jvms-6.5.tableswitch
	TableSwitch {
		default_target: LIRLabel,
		low: i32,
		high: i32,
		targets: Vec<LIRLabel>,
	},
}

impl Instruction {
	pub fn mnemonic(&self) -> &'static str {
		match self {
			Instruction::AConstNull => "ACONST_NULL",
			Instruction::BIPush(_) => "BIPUSH",
			Instruction::Ldc { .. } => "LDC",
			Instruction::ALoad { .. } => "ALOAD",
			Instruction::ILoad { .. } => "ILOAD",
			Instruction::IStore { .. } => "ISTORE",
			Instruction::IAdd => "IADD",
			Instruction::GetStatic { .. } => "GETSTATIC",
			Instruction::PutField { .. } => "PUTFIELD",
			Instruction::InvokeVirtual { .. } => "INVOKEVIRTUAL",
			Instruction::InvokeSpecial { .. } => "INVOKESPECIAL",
			Instruction::InvokeStatic { .. } => "INVOKESTATIC",
			Instruction::InvokeDynamic { .. } => "INVOKEDYNAMIC",
			Instruction::AReturn => "ARETURN",
			Instruction::Return => "RETURN",
			Instruction::MonitorExit => "MONITOREXIT",
			_ => todo!("unimplemented mnemonic for {:?}", self),
		}
	}

	pub fn parse<B: ReadBytesExt>(
		buffer: &mut B,
		cp: &ConstantPool,
		class_attrs: &[LIRClassAttribute],
		pc: u32,
	) -> Result<Self> {
		let mut opcode = buffer.read_u8()?;
		let is_wide = if opcode == opcodes::WIDE {
			opcode = buffer.read_u8()?;
			true
		} else {
			false
		};

		let calc_jmp_target = |offset: i32| -> LIRLabel { LIRLabel::Unresolved((pc as i32).wrapping_add(offset)) };
		let inst = match opcode {
			opcodes::AALOAD => Instruction::AALoad,
			opcodes::AASTORE => Instruction::AAStore,
			opcodes::ACONST_NULL => Instruction::AConstNull,
			opcodes::ALOAD => Instruction::ALoad {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::ALOAD_0 => Instruction::ALoad { local_idx: 0 },
			opcodes::ALOAD_1 => Instruction::ALoad { local_idx: 1 },
			opcodes::ALOAD_2 => Instruction::ALoad { local_idx: 2 },
			opcodes::ALOAD_3 => Instruction::ALoad { local_idx: 3 },
			opcodes::ANEWARRAY => {
				let idx = buffer.read_u16::<BigEndian>()?;
				let name = cp.resolve_class_name(cp.get_class(idx)?)?;
				Instruction::ANewArray { element_ty: name }
			}
			opcodes::ARETURN => Instruction::AReturn,
			opcodes::ARRAYLENGTH => Instruction::ArrayLength,
			opcodes::ASTORE => Instruction::AStore {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::ASTORE_0 => Instruction::AStore { local_idx: 0 },
			opcodes::ASTORE_1 => Instruction::AStore { local_idx: 1 },
			opcodes::ASTORE_2 => Instruction::AStore { local_idx: 2 },
			opcodes::ASTORE_3 => Instruction::AStore { local_idx: 3 },
			opcodes::ATHROW => Instruction::AThrow,
			opcodes::BALOAD => Instruction::BALoad,
			opcodes::BASTORE => Instruction::BAStore,
			opcodes::BIPUSH => Instruction::BIPush(buffer.read_i8()?),
			opcodes::CALOAD => Instruction::CALoad,
			opcodes::CASTORE => Instruction::CAStore,
			opcodes::CHECKCAST => {
				let idx = buffer.read_u16::<BigEndian>()?;
				let name = cp.resolve_class_name(cp.get_class(idx)?)?;
				Instruction::CheckCast { object_ty: name }
			}
			opcodes::D2F => Instruction::D2F,
			opcodes::D2I => Instruction::D2I,
			opcodes::D2L => Instruction::D2L,
			opcodes::DADD => Instruction::DAdd,
			opcodes::DALOAD => Instruction::DALoad,
			opcodes::DASTORE => Instruction::DAStore,
			opcodes::DCMPL => Instruction::DCmpL,
			opcodes::DCMPG => Instruction::DCmpG,
			opcodes::DCONST_0 => Instruction::DConst0,
			opcodes::DCONST_1 => Instruction::DConst1,
			opcodes::DDIV => Instruction::DDiv,
			opcodes::DLOAD => Instruction::DLoad {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::DLOAD_0 => Instruction::DLoad { local_idx: 0 },
			opcodes::DLOAD_1 => Instruction::DLoad { local_idx: 1 },
			opcodes::DLOAD_2 => Instruction::DLoad { local_idx: 2 },
			opcodes::DLOAD_3 => Instruction::DLoad { local_idx: 3 },
			opcodes::DMUL => Instruction::DMul,
			opcodes::DNEG => Instruction::DNeg,
			opcodes::DREM => Instruction::DRem,
			opcodes::DRETURN => Instruction::DReturn,
			opcodes::DSTORE => Instruction::DStore {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::DSTORE_0 => Instruction::DStore { local_idx: 0 },
			opcodes::DSTORE_1 => Instruction::DStore { local_idx: 1 },
			opcodes::DSTORE_2 => Instruction::DStore { local_idx: 2 },
			opcodes::DSTORE_3 => Instruction::DStore { local_idx: 3 },
			opcodes::DSUB => Instruction::DSub,
			opcodes::DUP => Instruction::Dup,
			opcodes::DUP_X1 => Instruction::DupX1,
			opcodes::DUP_X2 => Instruction::Dup2,
			opcodes::DUP2 => Instruction::Dup2,
			opcodes::DUP2_X1 => Instruction::Dup2X1,
			opcodes::DUP2_X2 => Instruction::Dup2X2,
			opcodes::F2D => Instruction::F2D,
			opcodes::F2I => Instruction::F2I,
			opcodes::F2L => Instruction::F2L,
			opcodes::FADD => Instruction::FAdd,
			opcodes::FALOAD => Instruction::FALoad,
			opcodes::FASTORE => Instruction::FAStore,
			opcodes::FCMPL => Instruction::FCmpL,
			opcodes::FCMPG => Instruction::FCmpG,
			opcodes::FCONST_0 => Instruction::FConst0,
			opcodes::FCONST_1 => Instruction::FConst1,
			opcodes::FCONST_2 => Instruction::FConst2,
			opcodes::FDIV => Instruction::FDiv,
			opcodes::FLOAD => Instruction::FLoad {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::FLOAD_0 => Instruction::FLoad { local_idx: 0 },
			opcodes::FLOAD_1 => Instruction::FLoad { local_idx: 1 },
			opcodes::FLOAD_2 => Instruction::FLoad { local_idx: 2 },
			opcodes::FLOAD_3 => Instruction::FLoad { local_idx: 3 },
			opcodes::FMUL => Instruction::FMul,
			opcodes::FNEG => Instruction::FNeg,
			opcodes::FREM => Instruction::FRem,
			opcodes::FRETURN => Instruction::FReturn,
			opcodes::FSTORE => Instruction::FStore {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::FSTORE_0 => Instruction::FStore { local_idx: 0 },
			opcodes::FSTORE_1 => Instruction::FStore { local_idx: 1 },
			opcodes::FSTORE_2 => Instruction::FStore { local_idx: 2 },
			opcodes::FSTORE_3 => Instruction::FStore { local_idx: 3 },
			opcodes::FSUB => Instruction::FSub,
			opcodes::GETFIELD => {
				let index = buffer.read_u16::<BigEndian>()?;

				let field_ref = cp.get_field_ref(index)?;
				let (name, descriptor) =
					cp.resolve_field_name_and_type(cp.get_name_and_type(field_ref.name_and_ty_index)?)?;

				let class = cp.resolve_class_name(cp.get_class(field_ref.class_index)?)?;
				Instruction::GetField {
					class,
					name,
					descriptor,
				}
			}
			opcodes::GETSTATIC => {
				let index = buffer.read_u16::<BigEndian>()?;

				let field_ref = cp.get_field_ref(index)?;
				let (name, descriptor) =
					cp.resolve_field_name_and_type(cp.get_name_and_type(field_ref.name_and_ty_index)?)?;

				let owner = cp.resolve_class_name(cp.get_class(field_ref.class_index)?)?;
				Instruction::GetStatic {
					owner,
					name,
					descriptor,
				}
			}
			opcodes::GOTO => Instruction::Goto {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::GOTO_W => Instruction::GotoW {
				target: calc_jmp_target(buffer.read_i32::<BigEndian>()?),
			},
			opcodes::I2B => Instruction::I2B,
			opcodes::I2C => Instruction::I2C,
			opcodes::I2D => Instruction::I2D,
			opcodes::I2F => Instruction::I2F,
			opcodes::I2L => Instruction::I2L,
			opcodes::I2S => Instruction::I2S,
			opcodes::IADD => Instruction::IAdd,
			opcodes::IALOAD => Instruction::IALoad,
			opcodes::IAND => Instruction::IAnd,
			opcodes::IASTORE => Instruction::IAStore,
			opcodes::ICONST_M1 => Instruction::IConst { val: -1 },
			opcodes::ICONST_0 => Instruction::IConst { val: 0 },
			opcodes::ICONST_1 => Instruction::IConst { val: 1 },
			opcodes::ICONST_2 => Instruction::IConst { val: 2 },
			opcodes::ICONST_3 => Instruction::IConst { val: 3 },
			opcodes::ICONST_4 => Instruction::IConst { val: 4 },
			opcodes::ICONST_5 => Instruction::IConst { val: 5 },
			opcodes::IDIV => Instruction::IDiv,
			opcodes::IF_ACMPEQ => Instruction::IfACmpEq {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IF_ACMPNE => Instruction::IfACmpNe {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IF_ICMPEQ => Instruction::IfICmpEq {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IF_ICMPNE => Instruction::IfICmpNe {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IF_ICMPLT => Instruction::IfICmpLt {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IF_ICMPGE => Instruction::IfICmpGe {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IF_ICMPGT => Instruction::IfICmpGt {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IF_ICMPLE => Instruction::IfICmpLe {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFEQ => Instruction::IfEq {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFNE => Instruction::IfNe {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFLT => Instruction::IfLt {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFGE => Instruction::IfGe {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFGT => Instruction::IfGt {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFLE => Instruction::IfLe {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFNONNULL => Instruction::IfNonNull {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IFNULL => Instruction::IfNull {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::IINC => {
				let (local_index, val) = if is_wide {
					(buffer.read_u16::<BigEndian>()?, buffer.read_i16::<BigEndian>()?)
				} else {
					(u16::from(buffer.read_u8()?), i16::from(buffer.read_i8()?))
				};
				Instruction::IInc { local_index, val }
			}
			opcodes::ILOAD => Instruction::ILoad {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::ILOAD_0 => Instruction::ILoad { local_idx: 0 },
			opcodes::ILOAD_1 => Instruction::ILoad { local_idx: 1 },
			opcodes::ILOAD_2 => Instruction::ILoad { local_idx: 2 },
			opcodes::ILOAD_3 => Instruction::ILoad { local_idx: 3 },
			opcodes::IMUL => Instruction::IMul,
			opcodes::INEG => Instruction::INeg,
			opcodes::INSTANCEOF => {
				let idx = buffer.read_u16::<BigEndian>()?;
				let name = cp.resolve_class_name(cp.get_class(idx)?)?;
				Instruction::InstanceOf { class_type: name }
			}
			opcodes::INVOKE_DYNAMIC => {
				let index = buffer.read_u16::<BigEndian>()?;

				if buffer.read_u8()? != 0 {
					bail!("Invalid opcode for INVOKE_DYNAMIC");
				}
				if buffer.read_u8()? != 0 {
					bail!("Invalid opcode for INVOKE_DYNAMIC");
				}

				let tag = cp.get_invoke_dynamic(index)?;
				let bootstrap_methods = class_attrs
					.iter()
					.find_map(|attr| match attr {
						LIRClassAttribute::BootstrapMethods(attr) => Some(attr),
						_ => None,
					})
					.ok_or_eyre("class had INVOKE_DYNAMIC but did not have a BootstrapMethods attr")?;
				let bsm = bootstrap_methods
					.methods
					.get(tag.bootstrap_method_attr_index as usize)
					.cloned()
					.unwrap();

				let nat = cp.get_name_and_type(tag.name_and_ty_index)?;
				let (name, descriptor) = cp.resolve_method_name_and_type(nat)?;
				Self::InvokeDynamic {
					owner: bsm.method.owner,
					name,
					descriptor,
					is_interface: bsm.method.is_interface,
					bsm_args: bsm.arguments,
				}
			}
			opcodes::INVOKE_INTERFACE => {
				let index = buffer.read_u16::<BigEndian>()?;
				let count = buffer.read_u8()?;
				let zero = buffer.read_u8()?;
				if zero != 0 {
					bail!("Fourth operand of invokeinterface must be 0");
				}

				let method_ref = cp.get_interface_method_ref(index)?;
				let nat = cp.get_name_and_type(method_ref.name_and_ty_index)?;
				let (name, descriptor) = cp.resolve_method_name_and_type(nat)?;

				let owner = cp.get_class(method_ref.class_index)?;
				let owner = cp.resolve_class_name(owner)?;

				Instruction::InvokeInterface {
					owner,
					name,
					descriptor,
					count,
				}
			}
			opcodes::INVOKE_SPECIAL => {
				let index = buffer.read_u16::<BigEndian>()?;

				let tag = cp.get_tag(index)?;
				let (class_index, name_and_type_index, is_interface) = match tag {
					CPTag::MethodRef(MethodRefTag {
						class_index,
						name_and_ty_index,
					}) => (*class_index, *name_and_ty_index, false),
					CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
						class_index,
						name_and_ty_index,
					}) => (*class_index, *name_and_ty_index, true),
					tag => bail!("Invalid tag for invokespecial: {:?}", tag),
				};

				let owner = cp.get_class(class_index)?;
				let owner = cp.resolve_class_name(owner)?;

				let nat = cp.get_name_and_type(name_and_type_index)?;
				let (name, descriptor) = cp.resolve_method_name_and_type(nat)?;
				Instruction::InvokeSpecial {
					owner,
					name,
					descriptor,
					is_interface,
				}
			}
			opcodes::INVOKE_STATIC => {
				let index = buffer.read_u16::<BigEndian>()?;

				let tag = cp.get_tag(index)?;
				let (class_index, name_and_type_index, is_interface) = match tag {
					CPTag::MethodRef(MethodRefTag {
						class_index,
						name_and_ty_index,
					}) => (*class_index, *name_and_ty_index, false),
					CPTag::InterfaceMethodRef(InterfaceMethodRefTag {
						class_index,
						name_and_ty_index,
					}) => (*class_index, *name_and_ty_index, true),
					tag => bail!("Invalid tag for invokespecial: {:?}", tag),
				};

				let owner = cp.get_class(class_index)?;
				let owner = cp.resolve_class_name(owner)?;

				let nat = cp.get_name_and_type(name_and_type_index)?;
				let (name, descriptor) = cp.resolve_method_name_and_type(nat)?;
				Instruction::InvokeStatic {
					owner,
					name,
					descriptor,
					is_interface,
				}
			}
			opcodes::INVOKE_VIRTUAL => {
				let index = buffer.read_u16::<BigEndian>()?;

				let method_ref = cp.get_method_ref(index)?;
				let nat = cp.get_name_and_type(method_ref.name_and_ty_index)?;
				let (name, descriptor) = cp.resolve_method_name_and_type(nat)?;

				let owner = cp.get_class(method_ref.class_index)?;
				let owner = cp.resolve_class_name(owner)?;

				Instruction::InvokeVirtual {
					owner,
					name,
					descriptor,
				}
			}
			opcodes::IOR => Instruction::IOr,
			opcodes::IREM => Instruction::IRem,
			opcodes::IRETURN => Instruction::IReturn,
			opcodes::ISHL => Instruction::IShl,
			opcodes::ISHR => Instruction::IShr,
			opcodes::ISTORE => Instruction::IStore {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::ISTORE_0 => Instruction::IStore { local_idx: 0 },
			opcodes::ISTORE_1 => Instruction::IStore { local_idx: 1 },
			opcodes::ISTORE_2 => Instruction::IStore { local_idx: 2 },
			opcodes::ISTORE_3 => Instruction::IStore { local_idx: 3 },
			opcodes::ISUB => Instruction::ISub,
			opcodes::IUSHR => Instruction::IUShr,
			opcodes::IXOR => Instruction::IXor,
			opcodes::JSR => Instruction::Jsr {
				target: calc_jmp_target(i32::from(buffer.read_i16::<BigEndian>()?)),
			},
			opcodes::JSR_W => Instruction::JsrW {
				target: calc_jmp_target(buffer.read_i32::<BigEndian>()?),
			},
			opcodes::L2D => Instruction::LongToDouble,
			opcodes::L2F => Instruction::LongToFloat,
			opcodes::L2I => Instruction::LongToInt,
			opcodes::LADD => Instruction::LAdd,
			opcodes::LALOAD => Instruction::LALoad,
			opcodes::LAND => Instruction::LAnd,
			opcodes::LASTORE => Instruction::LAStore,
			opcodes::LCMP => Instruction::LCmp,
			opcodes::LCONST_0 => Instruction::LConst { value: 0 },
			opcodes::LCONST_1 => Instruction::LConst { value: 1 },
			opcodes::LDC | opcodes::LDC_W => {
				let index = if opcode == opcodes::LDC {
					u16::from(buffer.read_u8()?)
				} else {
					buffer.read_u16::<BigEndian>()?
				};
				let tag = cp.get_tag(index)?;
				let constant = match tag {
					CPTag::Integer(v) => LIRLDCConstant::Int(*v as i32),
					CPTag::Float(v) => LIRLDCConstant::Float(*v),
					CPTag::String(StringTag { utf8_index }) => LIRLDCConstant::String(cp.get_utf8(*utf8_index)?),
					CPTag::Class(ClassTag { name_index }) => LIRLDCConstant::Class(cp.get_utf8(*name_index)?),
					CPTag::MethodType(MethodTypeTag { descriptor_index }) => {
						let descriptor = cp.get_utf8(*descriptor_index)?;
						LIRLDCConstant::MethodType(MethodDescriptor::from_str(&descriptor)?)
					}
					CPTag::MethodHandle { .. } => LIRLDCConstant::MethodHandle(LIRMethodHandle::resolve(cp, index)?),
					_ => bail!("invalid tag for ldc: {:?}", tag),
				};
				Instruction::Ldc { constant }
			}
			opcodes::LDC2_W => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp.get_tag(index)?;
				let constant = match tag {
					CPTag::Long(v) => LIRLDCConstant::Long(*v as i64),
					CPTag::Double(v) => LIRLDCConstant::Double(*v),
					_ => bail!("invalid tag for ldc2_w: {:?}", tag),
				};
				Instruction::Ldc { constant }
			}
			opcodes::LDIV => Instruction::LDiv,
			opcodes::LLOAD => Instruction::LLoad {
				local_idx: buffer.read_u8()?,
			},
			opcodes::LLOAD_0 => Instruction::LLoad { local_idx: 0 },
			opcodes::LLOAD_1 => Instruction::LLoad { local_idx: 1 },
			opcodes::LLOAD_2 => Instruction::LLoad { local_idx: 2 },
			opcodes::LLOAD_3 => Instruction::LLoad { local_idx: 3 },
			opcodes::LMUL => Instruction::LMul,
			opcodes::LNEG => Instruction::LNeg,
			opcodes::LOOKUPSWITCH => {
				let current_offset = pc + 1;
				let padding = (4 - (current_offset % 4)) % 4;

				for _ in 0..padding {
					buffer.read_u8()?;
				}

				let default_offset = buffer.read_i32::<BigEndian>()?;
				let default_target = LIRLabel::Unresolved((pc as i32).wrapping_add(default_offset));

				let npairs = buffer.read_i32::<BigEndian>()?;
				if npairs < 0 {
					bail!("lookupswitch npairs must be >= 0");
				}

				let mut pairs = Vec::with_capacity(npairs as usize);
				for _ in 0..npairs {
					let match_key = buffer.read_i32::<BigEndian>()?;
					let offset = buffer.read_i32::<BigEndian>()?;
					let target = LIRLabel::Unresolved((pc as i32).wrapping_add(offset));
					pairs.push((match_key, target));
				}

				Instruction::LookupSwitch { default_target, pairs }
			}
			opcodes::LOR => Instruction::LOr,
			opcodes::LREM => Instruction::LRem,
			opcodes::LRETURN => Instruction::LReturn,
			opcodes::LSHL => Instruction::LShl,
			opcodes::LSHR => Instruction::LShr,
			opcodes::LSTORE => Instruction::LStore {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::LSTORE_0 => Instruction::LStore { local_idx: 0 },
			opcodes::LSTORE_1 => Instruction::LStore { local_idx: 1 },
			opcodes::LSTORE_2 => Instruction::LStore { local_idx: 2 },
			opcodes::LSTORE_3 => Instruction::LStore { local_idx: 3 },
			opcodes::LSUB => Instruction::LSub,
			opcodes::LUSHR => Instruction::LUShr,
			opcodes::LXOR => Instruction::LXor,
			opcodes::MONITORENTER => Instruction::MonitorEnter,
			opcodes::MONITOREXIT => Instruction::MonitorExit,
			opcodes::MULTIANEWARRAY => {
				let idx = buffer.read_u16::<BigEndian>()?;
				let dimensions = buffer.read_u8()?;
				let name = cp.resolve_class_name(cp.get_class(idx)?)?;
				Instruction::MultiANewArray {
					element_ty: name,
					dimensions,
				}
			}
			opcodes::NEW => {
				let idx = buffer.read_u16::<BigEndian>()?;
				let name = cp.resolve_class_name(cp.get_class(idx)?)?;
				Instruction::New { object_ty: name }
			}
			opcodes::NEWARRAY => {
				let atype = ArrayType::try_from(buffer.read_u8()?)?;
				Instruction::NewArray { ty: atype }
			}
			opcodes::NOP => Instruction::Nop,
			opcodes::POP => Instruction::Pop,
			opcodes::POP2 => Instruction::Pop2,
			opcodes::PUTFIELD => {
				let index = buffer.read_u16::<BigEndian>()?;

				let field_ref = cp.get_field_ref(index)?;
				let nat = cp.get_name_and_type(field_ref.name_and_ty_index)?;
				let (name, descriptor) = cp.resolve_field_name_and_type(nat)?;

				let owner = cp.get_class(field_ref.class_index)?;
				let owner = cp.resolve_class_name(owner)?;
				Instruction::PutField {
					owner,
					name,
					descriptor,
				}
			}
			opcodes::PUTSTATIC => {
				let index = buffer.read_u16::<BigEndian>()?;

				let field_ref = cp.get_field_ref(index)?;
				let nat = cp.get_name_and_type(field_ref.name_and_ty_index)?;
				let (name, descriptor) = cp.resolve_field_name_and_type(nat)?;

				let owner = cp.get_class(field_ref.class_index)?;
				let owner = cp.resolve_class_name(owner)?;
				Instruction::PutStatic {
					owner,
					name,
					descriptor,
				}
			}
			opcodes::RET => Instruction::Ret {
				local_idx: if is_wide {
					buffer.read_u16::<BigEndian>()?
				} else {
					u16::from(buffer.read_u8()?)
				},
			},
			opcodes::RETURN => Instruction::Return,
			opcodes::SALOAD => Instruction::SALoad,
			opcodes::SASTORE => Instruction::SAStore,
			opcodes::SIPUSH => Instruction::SIPush {
				val: buffer.read_i16::<BigEndian>()?,
			},
			opcodes::SWAP => Instruction::Swap,
			opcodes::TABLESWITCH => {
				let current_offset = pc + 1;
				let padding = (4 - (current_offset % 4)) % 4;

				for _ in 0..padding {
					buffer.read_u8()?;
				}

				let default_offset = buffer.read_i32::<BigEndian>()?;
				let default_target = LIRLabel::Unresolved((pc as i32).wrapping_add(default_offset));

				let low = buffer.read_i32::<BigEndian>()?;
				let high = buffer.read_i32::<BigEndian>()?;

				if low > high {
					bail!("tableswitch low ({}) must be <= high ({})", low, high);
				}

				let count = (high as i64) - (low as i64) + 1;
				if count > 65535 {
					panic!("handle this case")
				}

				let mut targets = Vec::with_capacity(count as usize);
				for _ in 0..count {
					let offset = buffer.read_i32::<BigEndian>()?;
					let target = LIRLabel::Unresolved((pc as i32).wrapping_add(offset));
					targets.push(target);
				}

				Instruction::TableSwitch {
					default_target,
					low,
					high,
					targets,
				}
			}
			opcode => bail!("Unrecognized opcode: {} : 0x{:X}", opcode, opcode),
		};
		Ok(inst)
	}
}
