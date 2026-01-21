use std::{collections::HashMap, io::Cursor};

use bytemuck::AnyBitPattern;
use byteorder::{BigEndian, WriteBytesExt};
use eyre::{Context, Result, bail};
use num_conv::Truncate;
use paw_classfile_format::{
	AttributeInfo, CPTag, InnerClassAccessFlags, ModuleAccessFlags, ModuleExportAccessFlags, ModuleOpenAccessFlags,
	ModuleRequireAccessFlags, ParameterAccessFlags,
	class_pool::{self, ConstantPool, MethodTypeTag},
	descriptor::{Descriptor, MethodDescriptor},
	ext::ReadBytesExt,
};

use crate::{
	instruction::{Instruction, LIRLabel, LIRResolvedLabel},
	method::{LIRHandleDescriptor, LIRMethodHandle},
};

#[derive(Debug, Clone)]
pub enum LIRClassAttribute {
	SourceFile(SourceFileAttribute),
	InnerClasses(InnerClassesAttribute),
	EnclosingMethod(EnclosingMethodAttribute),
	SourceDebugExtension(DebugExtensionAttribute),
	BootstrapMethods(BootstrapMethodsAttribute),
	Module(ModuleAttribute),
	ModulePackages(ModulePackagesAttribute),
	ModuleMainClass(ModuleMainClassAttribute),
	NestHost(NestHostAttribute),
	NestMembers(NestMembersAttribute),
	Record(RecordAttribute),
	PermittedSubclasses(PermittedSubclassesAttribute),
	Synthetic,
	Deprecated,
	Signature(SignatureAttribute),
	RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	Unknown(String),
}

impl LIRClassAttribute {
	#[must_use]
	pub fn name(&self) -> &str {
		match self {
			LIRClassAttribute::SourceFile(..) => "SourceFile",
			LIRClassAttribute::InnerClasses(..) => "InnerClasses",
			LIRClassAttribute::EnclosingMethod(..) => "EnclosingMethod",
			LIRClassAttribute::SourceDebugExtension(..) => "SourceDebugExtension",
			LIRClassAttribute::BootstrapMethods(..) => "BootstrapMethods",
			LIRClassAttribute::Module(..) => "Module",
			LIRClassAttribute::ModulePackages(..) => "ModulePackages",
			LIRClassAttribute::ModuleMainClass(..) => "ModuleMainClass",
			LIRClassAttribute::NestHost(..) => "NestHost",
			LIRClassAttribute::NestMembers(..) => "NestMembers",
			LIRClassAttribute::Record(..) => "Record",
			LIRClassAttribute::PermittedSubclasses(..) => "PermittedSubclasses",
			LIRClassAttribute::Synthetic => "Synthetic",
			LIRClassAttribute::Deprecated => "Deprecated",
			LIRClassAttribute::Signature(..) => "Signature",
			LIRClassAttribute::RuntimeVisibleAnnotations(..) => "RuntimeVisibleAnnotations",
			LIRClassAttribute::RuntimeInvisibleAnnotations(..) => "RuntimeInvisibleAnnotations",
			LIRClassAttribute::RuntimeVisibleTypeAnnotations(..) => "RuntimeVisibleTypeAnnotations",
			LIRClassAttribute::RuntimeInvisibleTypeAnnotations(..) => "RuntimeInvisibleTypeAnnotations",
			LIRClassAttribute::Unknown(name) => name.as_str(),
		}
	}

	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"InnerClasses" => LIRClassAttribute::InnerClasses(InnerClassesAttribute::parse(&mut buffer, cp)?),
			"EnclosingMethod" => LIRClassAttribute::EnclosingMethod(EnclosingMethodAttribute::parse(&mut buffer, cp)?),
			"Synthetic" => LIRClassAttribute::Synthetic,
			"Signature" => LIRClassAttribute::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"SourceFile" => LIRClassAttribute::SourceFile(SourceFileAttribute::parse(&mut buffer, cp)?),
			"SourceDebugExtension" => {
				LIRClassAttribute::SourceDebugExtension(DebugExtensionAttribute::parse(&mut buffer)?)
			}
			"Deprecated" => LIRClassAttribute::Deprecated,
			"RuntimeVisibleAnnotations" => {
				LIRClassAttribute::RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleAnnotations" => {
				LIRClassAttribute::RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeVisibleTypeAnnotations" => LIRClassAttribute::RuntimeVisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleTypeAnnotations" => LIRClassAttribute::RuntimeInvisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"BootstrapMethods" => {
				LIRClassAttribute::BootstrapMethods(BootstrapMethodsAttribute::parse(&mut buffer, cp)?)
			}
			"Module" => LIRClassAttribute::Module(ModuleAttribute::parse(&mut buffer, cp)?),
			"ModulePackages" => LIRClassAttribute::ModulePackages(ModulePackagesAttribute::parse(&mut buffer, cp)?),
			"ModuleMainClass" => LIRClassAttribute::ModuleMainClass(ModuleMainClassAttribute::parse(&mut buffer, cp)?),
			"NestHost" => LIRClassAttribute::NestHost(NestHostAttribute::parse(&mut buffer, cp)?),
			"NestMembers" => LIRClassAttribute::NestMembers(NestMembersAttribute::parse(&mut buffer, cp)?),
			"Record" => LIRClassAttribute::Record(RecordAttribute::parse(&mut buffer, cp)?),
			"PermittedSubclasses" => {
				LIRClassAttribute::PermittedSubclasses(PermittedSubclassesAttribute::parse(&mut buffer, cp)?)
			}
			_ => {
				eprintln!("WARN: unknown attribute {}", name);
				LIRClassAttribute::Unknown(name.clone())
			}
		};

		let remaining = buffer.len();
		if remaining != 0 {
			bail!("{} extra attribute bytes in {} class attribute data", remaining, name);
		}
		Ok(kind)
	}

	pub fn write(&self, cp: &mut ConstantPool) -> Result<AttributeInfo> {
		let name = self.name();
		let attribute_name_index = cp.add_utf8(name.to_string());
		let mut info = Vec::new();

		match self {
			LIRClassAttribute::SourceFile(s) => {
				s.write(cp, &mut info)?;
			}
			LIRClassAttribute::InnerClasses(ic) => {
				ic.write(cp, &mut info)?;
			}
			LIRClassAttribute::EnclosingMethod(em) => {
				em.write(cp, &mut info)?;
			}
			LIRClassAttribute::SourceDebugExtension(sde) => {
				sde.write(&mut info)?;
			}
			LIRClassAttribute::BootstrapMethods(bsm) => {
				bsm.write(cp, &mut info)?;
			}
			LIRClassAttribute::Module(m) => {
				m.write(cp, &mut info)?;
			}
			LIRClassAttribute::ModulePackages(mp) => {
				mp.write(cp, &mut info)?;
			}
			LIRClassAttribute::ModuleMainClass(mc) => {
				mc.write(cp, &mut info)?;
			}
			LIRClassAttribute::NestHost(nh) => {
				nh.write(cp, &mut info)?;
			}
			LIRClassAttribute::NestMembers(nm) => {
				nm.write(cp, &mut info)?;
			}
			LIRClassAttribute::Record(r) => {
				r.write(cp, &mut info)?;
			}
			LIRClassAttribute::PermittedSubclasses(ps) => {
				ps.write(cp, &mut info)?;
			}
			LIRClassAttribute::Synthetic | LIRClassAttribute::Deprecated => {}
			LIRClassAttribute::Signature(sig) => {
				sig.write(cp, &mut info)?;
			}
			LIRClassAttribute::RuntimeVisibleAnnotations(ra) | LIRClassAttribute::RuntimeInvisibleAnnotations(ra) => {
				ra.write(cp, &mut info)?;
			}
			LIRClassAttribute::RuntimeVisibleTypeAnnotations(rta)
			| LIRClassAttribute::RuntimeInvisibleTypeAnnotations(rta) => {
				rta.write(cp, &mut info)?;
			}
			LIRClassAttribute::Unknown(_) => unreachable!("Class Attribute 'Unknown' should never be written"),
		}

		Ok(AttributeInfo {
			attribute_name_index,
			info,
		})
	}
}

#[derive(Debug, Clone)]
pub enum LIRFieldAttribute {
	ConstantValue(ConstantValueAttribute),
	Synthetic,
	Deprecated,
	Signature(SignatureAttribute),
	RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	Unknown(String),
}

impl LIRFieldAttribute {
	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"ConstantValue" => LIRFieldAttribute::ConstantValue(ConstantValueAttribute::parse(&mut buffer, cp)?),
			"Synthetic" => LIRFieldAttribute::Synthetic,
			"Signature" => LIRFieldAttribute::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"Deprecated" => LIRFieldAttribute::Deprecated,
			"RuntimeVisibleAnnotations" => {
				LIRFieldAttribute::RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleAnnotations" => {
				LIRFieldAttribute::RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeVisibleTypeAnnotations" => LIRFieldAttribute::RuntimeVisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleTypeAnnotations" => LIRFieldAttribute::RuntimeInvisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			_ => {
				eprintln!("WARN: unknown attribute {}", name);
				LIRFieldAttribute::Unknown(name.clone())
			}
		};

		let remaining = buffer.len();
		if remaining != 0 {
			bail!("{} extra attribute bytes in {} field attribute data", remaining, name);
		}
		Ok(kind)
	}
}

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
	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool, class_attrs: &[LIRClassAttribute]) -> Result<Self> {
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
}

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
	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"StackMapTable" => LIRCodeAttribute::StackMapTable(StackMapTableAttribute::parse(&mut buffer, cp)?),
			"LineNumberTable" => LIRCodeAttribute::LineNumberTable(LineNumberTableAttribute::parse(&mut buffer)?),
			"LocalVariableTable" => {
				LIRCodeAttribute::LocalVariableTable(LocalVariableTableAttribute::parse(&mut buffer, cp)?)
			}
			"LocalVariableTypeTable" => {
				LIRCodeAttribute::LocalVariableTypeTable(LocalVariableTypeTableAttribute::parse(&mut buffer, cp)?)
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
	}

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

#[derive(Debug, Clone)]
pub enum LIRRecordComponentAttribute {
	Signature(SignatureAttribute),
	RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute),
	RuntimeVisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	RuntimeInvisibleTypeAnnotations(RuntimeTypeAnnotationsAttribute),
	Unknown(String),
}

impl LIRRecordComponentAttribute {
	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"Signature" => LIRRecordComponentAttribute::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"RuntimeVisibleAnnotations" => LIRRecordComponentAttribute::RuntimeVisibleAnnotations(
				RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleAnnotations" => LIRRecordComponentAttribute::RuntimeInvisibleAnnotations(
				RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeVisibleTypeAnnotations" => LIRRecordComponentAttribute::RuntimeVisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleTypeAnnotations" => LIRRecordComponentAttribute::RuntimeInvisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			_ => {
				eprintln!("WARN: unknown attribute {}", name);
				LIRRecordComponentAttribute::Unknown(name.clone())
			}
		};

		let remaining = buffer.len();
		if remaining != 0 {
			bail!("{} extra attribute bytes in {} record attribute data", remaining, name);
		}
		Ok(kind)
	}

	pub fn write(&self, cp: &mut ConstantPool) -> Result<AttributeInfo> {
		let name = match self {
			LIRRecordComponentAttribute::Signature(_) => "Signature",
			LIRRecordComponentAttribute::RuntimeVisibleAnnotations(_) => "RuntimeVisibleAnnotations",
			LIRRecordComponentAttribute::RuntimeInvisibleAnnotations(_) => "RuntimeInvisibleAnnotations",
			LIRRecordComponentAttribute::RuntimeVisibleTypeAnnotations(_) => "RuntimeVisibleTypeAnnotations",
			LIRRecordComponentAttribute::RuntimeInvisibleTypeAnnotations(_) => "RuntimeInvisibleTypeAnnotations",
			LIRRecordComponentAttribute::Unknown(_) => unreachable!("Code attribute 'Unknown' should never be written"),
		};
		let attribute_name_index = cp.add_utf8(name.to_string());
		let mut info = Vec::new();

		match self {
			LIRRecordComponentAttribute::Signature(s) => {
				s.write(cp, &mut info)?;
			}
			LIRRecordComponentAttribute::RuntimeVisibleAnnotations(ra)
			| LIRRecordComponentAttribute::RuntimeInvisibleAnnotations(ra) => {
				ra.write(cp, &mut info)?;
			}
			LIRRecordComponentAttribute::RuntimeVisibleTypeAnnotations(rta)
			| LIRRecordComponentAttribute::RuntimeInvisibleTypeAnnotations(rta) => {
				rta.write(cp, &mut info)?;
			}
			LIRRecordComponentAttribute::Unknown(_) => {
				unreachable!("Record component attribute 'Unknown' cannot be written")
			}
		}

		Ok(AttributeInfo {
			attribute_name_index,
			info,
		})
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let index = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read tag index from ConstantValue attribute")?;
		let tag = cp.get_tag(index).wrap_err("invalid ConstantAttribute tag index")?;
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

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = match self {
			ConstantValueAttribute::Int(v) => cp.add_integer(v.cast_unsigned()),
			ConstantValueAttribute::Float(v) => cp.add_float(*v),
			ConstantValueAttribute::Long(v) => cp.add_long(v.cast_unsigned()),
			ConstantValueAttribute::Double(v) => cp.add_double(*v),
			ConstantValueAttribute::String(v) => cp.add_string(v.clone()),
		};
		info.write_u16::<BigEndian>(idx)?;
		Ok(())
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
	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = self.catch_type.as_ref().map_or(0, |t| cp.add_class(t.clone()));
		info.write_u16::<BigEndian>(self.start_pc)?;
		info.write_u16::<BigEndian>(self.end_pc)?;
		info.write_u16::<BigEndian>(self.handler_pc)?;
		info.write_u16::<BigEndian>(idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct CodeAttribute {
	pub max_stack: u16,
	pub max_locals: u16,
	/// offset (pc), instruction, resolved labels pairs
	pub code: Vec<(u32, Instruction, Vec<LIRLabel>)>,
	pub exception_table: Vec<CodeAttributeException>,
	pub attributes: Vec<LIRCodeAttribute>,
}

impl CodeAttribute {
	pub fn parse<B: ReadBytesExt>(
		buffer: &mut B,
		cp: &ConstantPool,
		class_attrs: &[LIRClassAttribute],
	) -> Result<Self> {
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
				catch_type: {
					let idx = reader
						.read_u16::<BigEndian>()
						.wrap_err("failed to read exception_table catch_type from Code attribute")?;
					if idx == 0 {
						None
					} else {
						let class = cp.get_class(idx)?;
						Some(cp.resolve_class_name(class)?)
					}
				},
			})
		})?;
		let attrs_count = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read attributes_count from Code attribute")?,
		);
		let attributes = buffer.read_vec_with(attrs_count, |b| {
			let raw = AttributeInfo::read(b).wrap_err("failed to read attribute from Code attribute")?;
			LIRCodeAttribute::parse(&raw, cp)
		})?;

		let mut code_buffer = Cursor::new(code);
		let mut code: Vec<(u32, Instruction, Vec<LIRLabel>)> = Vec::new();
		while code_buffer.position() < code_buffer.get_ref().len() as u64 {
			#[allow(
				clippy::cast_possible_truncation,
				reason = "length shouldnt be >u32::MAX per jvm spec"
			)]
			let pc = code_buffer.position() as u32;
			let instruction = Instruction::parse(&mut code_buffer, cp, class_attrs, pc)?;
			code.push((pc, instruction, Vec::new()));
		}

		let mut pc_to_label: HashMap<u32, LIRResolvedLabel> = HashMap::new();
		let mut next_label = 0;
		let mut allocate_label = |target_pc: u32| -> LIRResolvedLabel {
			*pc_to_label.entry(target_pc).or_insert_with(|| {
				let label = unsafe { LIRResolvedLabel::new_unchecked(next_label) };
				next_label += 1;
				label
			})
		};

		let mut resolve_unresolved_label = |label: &mut LIRLabel| {
			if let LIRLabel::Unresolved(pc) = label {
				*label = LIRLabel::Resolved(allocate_label(pc.cast_unsigned()));
			}
		};

		for (_, inst, _) in &mut code {
			match inst {
				Instruction::Goto { target }
				| Instruction::IfEq { target }
				| Instruction::IfNe { target }
				| Instruction::IfLt { target }
				| Instruction::IfGe { target }
				| Instruction::IfGt { target }
				| Instruction::IfLe { target }
				| Instruction::IfICmpEq { target }
				| Instruction::IfICmpNe { target }
				| Instruction::IfICmpLt { target }
				| Instruction::IfICmpGe { target }
				| Instruction::IfICmpGt { target }
				| Instruction::IfICmpLe { target }
				| Instruction::IfACmpEq { target }
				| Instruction::IfACmpNe { target }
				| Instruction::IfNull { target }
				| Instruction::IfNonNull { target }
				| Instruction::Jsr { target } => {
					resolve_unresolved_label(target);
				}

				Instruction::LookupSwitch { default_target, pairs } => {
					resolve_unresolved_label(default_target);
					for (_, target) in pairs {
						resolve_unresolved_label(target);
					}
				}

				Instruction::TableSwitch {
					default_target,
					targets,
					..
				} => {
					resolve_unresolved_label(default_target);
					for target in targets {
						resolve_unresolved_label(target);
					}
				}
				_ => {}
			}
		}

		for exception in &exception_table {
			allocate_label(u32::from(exception.handler_pc));
		}

		for (pc, _, labels) in &mut code {
			if let Some(resolved_label) = pc_to_label.get(pc) {
				labels.push(LIRLabel::Resolved(*resolved_label));
			}
		}

		Ok(CodeAttribute {
			max_stack,
			max_locals,
			code,
			exception_table,
			attributes,
		})
	}

	pub fn write<W: WriteBytesExt>(
		&self,
		cp: &mut ConstantPool,
		info: &mut W,
		bsm_pool: &[BootstrapMethod],
	) -> Result<()> {
		info.write_u16::<BigEndian>(self.max_stack)?;
		info.write_u16::<BigEndian>(self.max_locals)?;

		let mut label_map = HashMap::new();
		for (pc, _, labels) in &self.code {
			for label in labels {
				if let LIRLabel::Resolved(r) = label {
					label_map.insert(*r, *pc);
				}
			}
		}

		let mut code_buf = Vec::new();
		for (pc, inst, _) in &self.code {
			inst.write(&mut code_buf, cp, *pc, &label_map, bsm_pool)?;
		}
		#[allow(clippy::cast_possible_truncation, reason = "aaaa")]
		info.write_u32::<BigEndian>(code_buf.len() as u32)?;
		info.write_all(&code_buf)?;

		info.write_u16::<BigEndian>(self.exception_table.len().truncate())?;
		for exc in &self.exception_table {
			exc.write(cp, info)?;
		}

		info.write_u16::<BigEndian>(self.attributes.len().truncate())?;
		for attr in &self.attributes {
			attr.write(cp)?.write(info)?;
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

	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
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
				offset_delta: buffer.read_u16::<BigEndian>()?,
				stack: VerificationTypeInfo::parse(buffer, cp)?,
			},
			248..=250 => Self::ChopFrame {
				/*
				   The frame type chop_frame is represented by tags in the range [248-250]. If the frame_type is chop_frame,-
				   it means that the operand stack is empty and the current locals are the same as the locals in the previous frame,-
				   except that the k last locals are absent. The value of k is given by the formula 251 - frame_type.
				*/
				absent_locals_count: ChopFrameAbsentLocals::new(251 - frame_type)?,
				offset_delta: buffer.read_u16::<BigEndian>()?,
			},
			251 => Self::SameFrameExtended {
				offset_delta: buffer.read_u16::<BigEndian>()?,
			},
			252..=254 => {
				let offset_delta = buffer.read_u16::<BigEndian>()?;

				let n_locals = (frame_type - 251) as usize;
				let locals = buffer.read_vec_with(n_locals, |b| VerificationTypeInfo::parse(b, cp))?;
				Self::AppendFrame {
					offset_delta,
					locals: AppendFrameLocals::from_vec(locals)?,
				}
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

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
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
				info.write_u16::<BigEndian>(*offset_delta)?;
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
				info.write_u16::<BigEndian>(*offset_delta)?;
			}
			StackMapFrame::SameFrameExtended { offset_delta } => {
				info.write_u8(251)?;
				info.write_u16::<BigEndian>(*offset_delta)?;
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
				info.write_u16::<BigEndian>(*offset_delta)?;
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
				info.write_u16::<BigEndian>(*offset_delta)?;
				info.write_u16::<BigEndian>(locals.len().truncate())?;
				for local in locals {
					local.write(cp, info)?;
				}
				info.write_u16::<BigEndian>(stack.len().truncate())?;
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let entries_count = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read number_of_entries from StackMapTable attribute")?,
		);
		let entries = buffer.read_vec_with(entries_count, |b| StackMapFrame::parse(b, cp))?;
		Ok(StackMapTableAttribute { entries })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.entries.len().truncate())?;
		for entry in &self.entries {
			entry.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct ExceptionsAttribute {
	pub exception_classes: Vec<String>,
}

impl ExceptionsAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let num_exceptions = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read number_of_exceptions from Exceptions attribute")?,
		);
		let exception_classes = buffer.read_vec_with(num_exceptions, |b| {
			let index = b
				.read_u16::<BigEndian>()
				.wrap_err("failed to read exception_index from Exceptions attribute")?;
			Ok(cp.resolve_class_name(cp.get_class(index)?)?)
		})?;
		Ok(Self { exception_classes })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.exception_classes.len().truncate())?;
		for ele in &self.exception_classes {
			let idx = cp.add_class(ele.clone());
			info.write_u16::<BigEndian>(idx)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct InnerClassesAttributeClass {
	pub inner_class_info: String,         // ClassRef
	pub outer_class_info: Option<String>, // ClassRef
	pub inner_name: Option<String>,       // Utf8Ref
	pub inner_class_access_flags: InnerClassAccessFlags,
}

impl InnerClassesAttributeClass {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let inner_info_idx = buffer.read_u16::<BigEndian>()?;
		let outer_info_idx = buffer.read_u16::<BigEndian>()?;
		let inner_name_idx = buffer.read_u16::<BigEndian>()?;
		let inner_class_access_flags = InnerClassAccessFlags::try_from(buffer.read_u16::<BigEndian>()?)?;

		let inner_class_info = cp.resolve_class_name(cp.get_class(inner_info_idx)?)?;
		let outer_class_info = if outer_info_idx != 0 {
			Some(cp.resolve_class_name(cp.get_class(outer_info_idx)?)?)
		} else {
			None
		};
		let inner_name = if inner_name_idx != 0 {
			Some(cp.get_utf8(inner_name_idx)?)
		} else {
			None
		};

		Ok(Self {
			inner_class_info,
			outer_class_info,
			inner_name,
			inner_class_access_flags,
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let inner_info_idx = cp.add_class(self.inner_class_info.clone());
		let outer_info_idx = if let Some(ref outer) = self.outer_class_info {
			cp.add_class(outer.clone())
		} else {
			0
		};
		let inner_name_idx = if let Some(ref name) = self.inner_name {
			cp.add_utf8(name.clone())
		} else {
			0
		};
		info.write_u16::<BigEndian>(inner_info_idx)?;
		info.write_u16::<BigEndian>(outer_info_idx)?;
		info.write_u16::<BigEndian>(inner_name_idx)?;
		info.write_u16::<BigEndian>(self.inner_class_access_flags.bits())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct InnerClassesAttribute {
	pub classes: Vec<InnerClassesAttributeClass>,
}

impl InnerClassesAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_classes = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read number_of_classes from InnerClasses attribute")?,
		);
		let classes = buffer.read_vec_with(n_classes, |b| InnerClassesAttributeClass::parse(b, cp))?;
		Ok(InnerClassesAttribute { classes })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.classes.len().truncate())?;
		for class in &self.classes {
			class.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct EnclosingMethodAttribute {
	pub class: String,
	pub method_name: Option<String>,
	pub method_descriptor: Option<MethodDescriptor>,
}

impl EnclosingMethodAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let class_idx = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read class_index from EnclosingMethod attribute")?;
		let method_idx = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read method_index from EnclosingMethod attribute")?;

		let class = cp.resolve_class_name(cp.get_class(class_idx)?)?;
		let (method_name, method_descriptor) = if method_idx == 0 {
			(None, None)
		} else {
			let method_name_and_ty = cp.get_name_and_type(method_idx)?;
			let method_name = cp.get_utf8(method_name_and_ty.name_index)?;
			let method_descriptor = cp.get_utf8(method_name_and_ty.descriptor_index)?.parse()?;
			(Some(method_name), Some(method_descriptor))
		};

		Ok(EnclosingMethodAttribute {
			class,
			method_name,
			method_descriptor,
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let class_idx = cp.add_class(self.class.clone());
		let method_name_and_ty = if let Some(method_name) = &self.method_name
			&& let Some(method_desc) = &self.method_descriptor
		{
			cp.add_name_and_type(method_name.clone(), method_desc.jvm_repr())
		} else {
			0
		};
		info.write_u16::<BigEndian>(class_idx)?;
		info.write_u16::<BigEndian>(method_name_and_ty)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct SignatureAttribute {
	// FIXME: more structured data?
	pub signature: String,
}

impl SignatureAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let signature_index = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read signature_index from Signature attribute")?;
		let signature = cp.get_utf8(signature_index)?;
		Ok(Self { signature })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_utf8(self.signature.clone());
		info.write_u16::<BigEndian>(idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct SourceFileAttribute {
	pub source_file: String,
}

impl SourceFileAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let sourcefile_index = buffer
			.read_u16::<BigEndian>()
			.wrap_err("failed to read sourcefile_index from SourceFile attribute")?;
		let source_file = cp.get_utf8(sourcefile_index)?;
		Ok(Self { source_file })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_utf8(self.source_file.clone());
		info.write_u16::<BigEndian>(idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct DebugExtensionAttribute {
	pub debug_data: String,
}

impl DebugExtensionAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B) -> Result<Self> {
		// the debug extension just uses the entire length of the attribute data
		let mut buf = Vec::new();
		buffer.read_to_end(&mut buf)?;
		let debug_data = paw_mutf8::decode(&buf)?.to_string();
		Ok(Self { debug_data })
	}

	pub fn write<W: WriteBytesExt>(&self, info: &mut W) -> Result<()> {
		let encoded = paw_mutf8::encode(&self.debug_data);
		info.write_u16::<BigEndian>(encoded.len().truncate())?;
		info.write_all(&encoded)?;
		Ok(())
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

	pub fn write<W: WriteBytesExt>(&self, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.table.len().truncate())?;
		for ele in &self.table {
			info.write_u16::<BigEndian>(ele.start_pc)?;
			info.write_u16::<BigEndian>(ele.line_number)?;
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let start_pc = buffer.read_u16::<BigEndian>()?;
		let len = buffer.read_u16::<BigEndian>()?;
		let name_idx = buffer.read_u16::<BigEndian>()?;
		let name = cp.get_utf8(name_idx)?;
		let descriptor_idx = buffer.read_u16::<BigEndian>()?;
		let descriptor = cp.get_utf8(descriptor_idx)?.parse()?;
		let local_idx = buffer.read_u16::<BigEndian>()?;
		Ok(LocalVariableTableEntry {
			start_pc,
			len,
			name,
			descriptor,
			local_idx,
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.start_pc)?;
		info.write_u16::<BigEndian>(self.len)?;

		let name_idx = cp.add_utf8(self.name.clone());
		info.write_u16::<BigEndian>(name_idx)?;

		let descriptor_idx = cp.add_utf8(self.descriptor.jvm_repr());
		info.write_u16::<BigEndian>(descriptor_idx)?;

		info.write_u16::<BigEndian>(self.local_idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTableAttribute {
	pub table: Vec<LocalVariableTableEntry>,
}

impl LocalVariableTableAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let num_entries = usize::from(
			buffer
				.read_u16::<BigEndian>()
				.wrap_err("failed to read local_variable_table_length from LocalVariableTable attribute")?,
		);
		let table = buffer.read_vec_with(num_entries, |b| LocalVariableTableEntry::parse(b, cp))?;
		Ok(Self { table })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.table.len().truncate())?;
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
	pub fn read<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let start_pc = buffer.read_u16::<BigEndian>()?;
		let len = buffer.read_u16::<BigEndian>()?;
		let name_idx = buffer.read_u16::<BigEndian>()?;
		let name = cp.get_utf8(name_idx)?;
		let signature_idx = buffer.read_u16::<BigEndian>()?;
		let signature = cp.get_utf8(signature_idx)?;
		let local_idx = buffer.read_u16::<BigEndian>()?;
		Ok(LocalVariableTypeTableEntry {
			start_pc,
			len,
			name,
			signature,
			local_idx,
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.start_pc)?;
		info.write_u16::<BigEndian>(self.len)?;

		let name_idx = cp.add_utf8(self.name.clone());
		info.write_u16::<BigEndian>(name_idx)?;

		let signature_idx = cp.add_utf8(self.signature.clone());
		info.write_u16::<BigEndian>(signature_idx)?;

		info.write_u16::<BigEndian>(self.local_idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct LocalVariableTypeTableAttribute {
	pub table: Vec<LocalVariableTypeTableEntry>,
}

impl LocalVariableTypeTableAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let num_entries = usize::from(buffer.read_u16::<BigEndian>()?);
		let table = buffer.read_vec_with(num_entries, |reader| LocalVariableTypeTableEntry::read(reader, cp))?;
		Ok(Self { table })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.table.len().truncate())?;
		for ele in &self.table {
			ele.write(cp, info)?;
		}
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let tag = buffer.read_u8()?;
		Ok(match tag {
			b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => {
				let index = buffer.read_u16::<BigEndian>()?;
				let tag = cp
					.get_tag(index)
					.wrap_err("runtime annotation value constant value tag index is invalid")?;
				Self::ConstValueIndex(match tag {
					CPTag::Integer(i) => ConstantValueAttribute::Int(i.cast_signed()),
					CPTag::Float(f) => ConstantValueAttribute::Float(*f),
					CPTag::Long(l) => ConstantValueAttribute::Long(l.cast_signed()),
					CPTag::Double(d) => ConstantValueAttribute::Double(*d),
					CPTag::Utf8(class_pool::Utf8Tag { value }) => ConstantValueAttribute::String(value.clone()),
					tag => bail!("invalid RuntimeAnnotationValue attribute tag {:?}", tag),
				})
			}

			b'e' => Self::EnumConstValue {
				type_name: cp.get_utf8(buffer.read_u16::<BigEndian>()?)?,
				const_name: cp.get_utf8(buffer.read_u16::<BigEndian>()?)?,
			},

			b'c' => Self::ClassInfoIndex(cp.get_utf8(buffer.read_u16::<BigEndian>()?)?),
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

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
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
				info.write_u16::<BigEndian>(idx)?;
			}
			RuntimeAnnotationValue::EnumConstValue { type_name, const_name } => {
				info.write_u8(b'e')?;
				let type_idx = cp.add_utf8(type_name.clone());
				let const_idx = cp.add_utf8(const_name.clone());
				info.write_u16::<BigEndian>(type_idx)?;
				info.write_u16::<BigEndian>(const_idx)?;
			}
			RuntimeAnnotationValue::ClassInfoIndex(name) => {
				info.write_u8(b'c')?;
				let idx = cp.add_utf8(name.clone());
				info.write_u16::<BigEndian>(idx)?;
			}
			RuntimeAnnotationValue::Annotation(a) => {
				info.write_u8(b'@')?;
				a.write(cp, info)?;
			}
			RuntimeAnnotationValue::ArrayValue { values } => {
				info.write_u8(b'[')?;
				info.write_u16::<BigEndian>(values.len().truncate())?;
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let ty = cp.get_utf8(buffer.read_u16::<BigEndian>()?)?.parse()?;

		let n_pairs = buffer.read_u16::<BigEndian>()? as usize;
		let mut pairs = Vec::with_capacity(n_pairs);

		for _ in 0..n_pairs {
			let name = cp.get_utf8(buffer.read_u16::<BigEndian>()?)?;
			pairs.push(RuntimeAnnotationElementValuePair {
				name,
				value: RuntimeAnnotationValue::parse(buffer, cp)?,
			});
		}

		Ok(Self { ty, pairs })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let type_idx = cp.add_utf8(self.ty.jvm_repr());
		info.write_u16::<BigEndian>(type_idx)?;
		info.write_u16::<BigEndian>(self.pairs.len().truncate())?;
		for pair in &self.pairs {
			let name_idx = cp.add_utf8(pair.name.clone());
			info.write_u16::<BigEndian>(name_idx)?;
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_annotations = buffer.read_u16::<BigEndian>()? as usize;
		let annotations = buffer.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?;
		Ok(Self { annotations })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.annotations.len().truncate())?;
		for anno in &self.annotations {
			anno.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeParameterAnnotation {
	pub annotations: Vec<RuntimeAnnotation>,
}

impl RuntimeParameterAnnotation {
	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.annotations.len().truncate())?;
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_params = usize::from(buffer.read_u8()?);
		let param_annotations = buffer.read_vec_with(n_params, |b| {
			let n_annotations = usize::from(b.read_u16::<BigEndian>()?);
			Ok(RuntimeParameterAnnotation {
				annotations: b.read_vec_with(n_annotations, |b| RuntimeAnnotation::parse(b, cp))?,
			})
		})?;
		Ok(Self { param_annotations })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u8(self.param_annotations.len().truncate())?;
		for ele in &self.param_annotations {
			ele.write(cp, info)?;
		}
		Ok(())
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let target_type = buffer.read_u8()?;
		let target_info = RuntimeTypeAnnotationTargetInfo::parse(target_type, buffer)?;
		let target_path = parse_type_path(buffer)?;

		let ty = cp.get_utf8(buffer.read_u16::<BigEndian>()?)?.parse()?;

		let n_pairs = buffer.read_u16::<BigEndian>()? as usize;
		let mut pairs = Vec::with_capacity(n_pairs);

		for _ in 0..n_pairs {
			let name = cp.get_utf8(buffer.read_u16::<BigEndian>()?)?;
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

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		self.target_info.write(info)?;

		info.write_u8(self.target_path.len().truncate())?;
		for part in &self.target_path {
			part.write(info)?;
		}

		let type_idx = cp.add_utf8(self.ty.jvm_repr());
		info.write_u16::<BigEndian>(type_idx)?;

		info.write_u16::<BigEndian>(self.pairs.len().truncate())?;
		for pair in &self.pairs {
			let name_idx = cp.add_utf8(pair.name.clone());
			info.write_u16::<BigEndian>(name_idx)?;
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_annotations = buffer.read_u16::<BigEndian>()? as usize;
		let annotations = buffer.read_vec_with(n_annotations, |b| RuntimeTypeAnnotation::parse(b, cp))?;
		Ok(Self { annotations })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.annotations.len().truncate())?;
		for anno in &self.annotations {
			anno.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub enum BootstrapMethodArgument {
	Int(i32),
	Long(i64),
	Float(f32),
	Double(f64),
	String(String),
	Class(String),
	MethodHandle(LIRMethodHandle),
	MethodType(MethodDescriptor),
}

#[derive(Debug, Clone)]
pub struct BootstrapMethod {
	pub method: LIRMethodHandle,
	pub arguments: Vec<BootstrapMethodArgument>,
}

impl BootstrapMethod {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let method_idx = buffer.read_u16::<BigEndian>()?;
		let n_args = buffer.read_u16::<BigEndian>()? as usize;
		let argument_idxs = buffer.read_vec_with(n_args, |b| Ok(b.read_u16::<BigEndian>()?))?;

		let arguments = argument_idxs
			.into_iter()
			.map(|idx| {
				let tag = cp.get_tag(idx)?;
				Ok(match tag {
					CPTag::Integer(v) => BootstrapMethodArgument::Int(v.cast_signed()),
					CPTag::Long(v) => BootstrapMethodArgument::Long(v.cast_signed()),
					CPTag::Float(v) => BootstrapMethodArgument::Float(*v),
					CPTag::Double(v) => BootstrapMethodArgument::Double(*v),
					CPTag::String(tag) => BootstrapMethodArgument::String(cp.resolve_string(tag)?),
					CPTag::Class(tag) => BootstrapMethodArgument::Class(cp.resolve_class_name(tag)?),
					CPTag::MethodHandle { .. } => {
						BootstrapMethodArgument::MethodHandle(LIRMethodHandle::resolve(cp, idx)?)
					}
					CPTag::MethodType(MethodTypeTag { descriptor_index }) => {
						let desc = cp.get_utf8(*descriptor_index)?;
						BootstrapMethodArgument::MethodType(desc.parse()?)
					}
					_ => bail!("Invalid bootstrap argument tag: {:?}", tag),
				})
			})
			.collect::<Result<Vec<_>>>()?;

		Ok(Self {
			method: LIRMethodHandle::resolve(cp, method_idx)?,
			arguments,
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let method_ref = match &self.method.descriptor {
			LIRHandleDescriptor::Field(f) => {
				cp.add_field_ref(self.method.owner.clone(), self.method.name.clone(), f.jvm_repr())
			}
			LIRHandleDescriptor::Method(d) => {
				if self.method.is_interface {
					cp.add_interface_method_ref(self.method.owner.clone(), self.method.name.clone(), d.jvm_repr())
				} else {
					cp.add_method_ref(self.method.owner.clone(), self.method.name.clone(), d.jvm_repr())
				}
			}
		};

		let handle = cp.add_method_handle(u8::from(self.method.kind), method_ref);
		info.write_u16::<BigEndian>(handle)?;

		info.write_u16::<BigEndian>(self.arguments.len().truncate())?;
		for arg in &self.arguments {
			let idx = match arg {
				BootstrapMethodArgument::Int(v) => cp.add_integer(v.cast_unsigned()),
				BootstrapMethodArgument::Long(v) => cp.add_long(v.cast_unsigned()),
				BootstrapMethodArgument::Float(v) => cp.add_float(*v),
				BootstrapMethodArgument::Double(v) => cp.add_double(*v),
				BootstrapMethodArgument::String(v) => cp.add_string(v.clone()),
				BootstrapMethodArgument::Class(v) => cp.add_class(v.clone()),
				BootstrapMethodArgument::MethodHandle(h) => {
					let r = match &h.descriptor {
						LIRHandleDescriptor::Field(f) => {
							cp.add_field_ref(h.owner.clone(), h.name.clone(), f.jvm_repr())
						}
						LIRHandleDescriptor::Method(d) => {
							if h.is_interface {
								cp.add_interface_method_ref(h.owner.clone(), h.name.clone(), d.jvm_repr())
							} else {
								cp.add_method_ref(h.owner.clone(), h.name.clone(), d.jvm_repr())
							}
						}
					};
					cp.add_method_handle(u8::from(h.kind), r)
				}
				BootstrapMethodArgument::MethodType(t) => cp.add_method_type(t.jvm_repr()),
			};
			info.write_u16::<BigEndian>(idx)?;
		}

		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct BootstrapMethodsAttribute {
	pub methods: Vec<BootstrapMethod>,
}

impl BootstrapMethodsAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_methods = usize::from(buffer.read_u16::<BigEndian>()?);
		let methods = buffer.read_vec_with(n_methods, |b| BootstrapMethod::parse(b, cp))?;
		Ok(Self { methods })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.methods.len().truncate())?;
		for method in &self.methods {
			method.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct MethodParameterEntry {
	pub name: Option<String>,
	// FIXME: document/enforce restrictions
	pub access: ParameterAccessFlags,
}

impl MethodParameterEntry {
	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let name_idx = self.name.as_ref().map_or(0, |name| cp.add_utf8(name.clone()));
		info.write_u16::<BigEndian>(name_idx)?;
		info.write_u16::<BigEndian>(self.access.bits())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct MethodParametersAttribute {
	pub parameters: Vec<MethodParameterEntry>,
}

impl MethodParametersAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let param_count = usize::from(buffer.read_u8()?);
		let parameters = buffer.read_vec_with(param_count, |b| {
			let name_idx = b.read_u16::<BigEndian>()?;
			let access = ParameterAccessFlags::try_from(b.read_u16::<BigEndian>()?)?;
			let name = if name_idx != 0 {
				Some(cp.get_utf8(name_idx)?)
			} else {
				None
			};
			Ok(MethodParameterEntry { name, access })
		})?;
		Ok(Self { parameters })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u8(self.parameters.len().truncate())?;
		for ele in &self.parameters {
			ele.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct ModuleRequire {
	pub module: String,
	pub flags: ModuleRequireAccessFlags,
	pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleExport {
	pub package: String,
	pub flags: ModuleExportAccessFlags,
	pub exports_to: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleOpen {
	pub package: String,
	pub flags: ModuleOpenAccessFlags,
	pub opens_to: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleProvide {
	pub service: String,
	pub providers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleAttribute {
	pub name: String,
	pub flags: ModuleAccessFlags,
	pub version: Option<String>,
	pub requires: Vec<ModuleRequire>,
	pub exports: Vec<ModuleExport>,
	pub opens: Vec<ModuleOpen>,
	pub uses: Vec<String>,
	pub provides: Vec<ModuleProvide>,
}

impl ModuleAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let index = buffer.read_u16::<BigEndian>()?;
		let name = cp.resolve_module_name(cp.get_module(index)?)?;

		let flags = ModuleAccessFlags::try_from(buffer.read_u16::<BigEndian>()?)?;

		let version_index = buffer.read_u16::<BigEndian>()?;
		let version = if version_index == 0 {
			None
		} else {
			Some(cp.get_utf8(version_index)?)
		};

		let requires_count = buffer.read_u16::<BigEndian>()?;
		let requires = buffer.read_vec_with(requires_count as usize, |b| {
			let requires_index = b.read_u16::<BigEndian>()?;
			let requires_module = cp.get_module(requires_index)?;
			let requires_flags = ModuleRequireAccessFlags::try_from(b.read_u16::<BigEndian>()?)?;
			let requires_version_index = b.read_u16::<BigEndian>()?;
			Ok(ModuleRequire {
				module: cp.resolve_module_name(requires_module)?,
				flags: requires_flags,
				version: if requires_version_index == 0 {
					None
				} else {
					Some(cp.get_utf8(requires_version_index)?)
				},
			})
		})?;

		let exports_count = buffer.read_u16::<BigEndian>()?;
		let exports = buffer.read_vec_with(exports_count as usize, |b| {
			let exports_index = b.read_u16::<BigEndian>()?;
			let exports_package = cp.get_package(exports_index)?;
			let exports_flags = ModuleExportAccessFlags::try_from(b.read_u16::<BigEndian>()?)?;
			let exports_to_count = b.read_u16::<BigEndian>()?;
			let exports_to = b.read_vec_with(exports_to_count as usize, |b2| {
				let exports_to_index = b2.read_u16::<BigEndian>()?;
				let exports_to_module = cp.get_module(exports_to_index)?;
				Ok(cp.resolve_module_name(exports_to_module)?)
			})?;
			Ok(ModuleExport {
				package: cp.resolve_package_name(exports_package)?,
				flags: exports_flags,
				exports_to,
			})
		})?;

		let opens_count = buffer.read_u16::<BigEndian>()?;
		let opens = buffer.read_vec_with(opens_count as usize, |b| {
			let opens_index = b.read_u16::<BigEndian>()?;
			let opens_flags = ModuleOpenAccessFlags::try_from(b.read_u16::<BigEndian>()?)?;
			let opens_to_count = b.read_u16::<BigEndian>()?;
			let opens_to = b.read_vec_with(opens_to_count as usize, |b2| {
				let opens_to_index = b2.read_u16::<BigEndian>()?;
				let opens_to_module = cp.get_module(opens_to_index)?;
				Ok(cp.resolve_module_name(opens_to_module)?)
			})?;

			let opens_package = cp.get_package(opens_index)?;
			Ok(ModuleOpen {
				package: cp.resolve_package_name(opens_package)?,
				flags: opens_flags,
				opens_to,
			})
		})?;

		let uses_count = buffer.read_u16::<BigEndian>()?;
		let uses = buffer.read_vec_with(uses_count as usize, |b| {
			let uses_index = b.read_u16::<BigEndian>()?;
			let uses_class = cp.get_class(uses_index)?;
			Ok(cp.resolve_class_name(uses_class)?)
		})?;

		let provides_count = buffer.read_u16::<BigEndian>()?;
		let provides = buffer.read_vec_with(provides_count as usize, |b| {
			let provides_index = b.read_u16::<BigEndian>()?;
			let provides_class = cp.get_class(provides_index)?;
			let provides_with_count = b.read_u16::<BigEndian>()?;
			let providers = b.read_vec_with(provides_with_count as usize, |b2| {
				let provides_with_index = b2.read_u16::<BigEndian>()?;
				let provides_with_class = cp.get_class(provides_with_index)?;
				Ok(cp.resolve_class_name(provides_with_class)?)
			})?;
			Ok(ModuleProvide {
				service: cp.resolve_class_name(provides_class)?,
				providers,
			})
		})?;

		Ok(Self {
			name,
			flags,
			version,
			requires,
			exports,
			opens,
			uses,
			provides,
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let name_idx = cp.add_module(self.name.clone());
		info.write_u16::<BigEndian>(name_idx)?;
		info.write_u16::<BigEndian>(self.flags.bits())?;

		let version_idx = self.version.as_ref().map_or(0, |v| cp.add_utf8(v.clone()));
		info.write_u16::<BigEndian>(version_idx)?;

		info.write_u16::<BigEndian>(self.requires.len().truncate())?;
		for r in &self.requires {
			let requires_idx = cp.add_module(r.module.clone());
			info.write_u16::<BigEndian>(requires_idx)?;
			info.write_u16::<BigEndian>(r.flags.bits())?;

			let version_idx = r.version.as_ref().map_or(0, |v| cp.add_utf8(v.clone()));
			info.write_u16::<BigEndian>(version_idx)?;
		}

		info.write_u16::<BigEndian>(self.exports.len().truncate())?;
		for e in &self.exports {
			let exports_idx = cp.add_package(e.package.clone());
			info.write_u16::<BigEndian>(exports_idx)?;
			info.write_u16::<BigEndian>(e.flags.bits())?;

			info.write_u16::<BigEndian>(e.exports_to.len().truncate())?;
			for ele in &e.exports_to {
				let exports_to_idx = cp.add_module(ele.clone());
				info.write_u16::<BigEndian>(exports_to_idx)?;
			}
		}

		info.write_u16::<BigEndian>(self.opens.len().truncate())?;
		for o in &self.opens {
			let opens_index = cp.add_package(o.package.clone());
			info.write_u16::<BigEndian>(opens_index)?;
			info.write_u16::<BigEndian>(o.flags.bits())?;

			info.write_u16::<BigEndian>(o.opens_to.len().truncate())?;
			for ot in &o.opens_to {
				let opens_to_index = cp.add_module(ot.clone());
				info.write_u16::<BigEndian>(opens_to_index)?;
			}
		}

		info.write_u16::<BigEndian>(self.uses.len().truncate())?;
		for u in &self.uses {
			let uses_index = cp.add_class(u.clone());
			info.write_u16::<BigEndian>(uses_index)?;
		}

		info.write_u16::<BigEndian>(self.provides.len().truncate())?;
		for p in &self.provides {
			let provides_index = cp.add_class(p.service.clone());
			info.write_u16::<BigEndian>(provides_index)?;

			info.write_u16::<BigEndian>(p.providers.len().truncate())?;
			for ele in &p.providers {
				let provider_idx = cp.add_class(ele.clone());
				info.write_u16::<BigEndian>(provider_idx)?;
			}
		}

		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct ModulePackagesAttribute {
	pub packages: Vec<String>,
}

impl ModulePackagesAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let package_count = buffer.read_u16::<BigEndian>()?;
		let packages = buffer.read_vec_with(package_count as usize, |b| {
			let package_index = b.read_u16::<BigEndian>()?;
			let package = cp.get_package(package_index)?;
			Ok(cp.resolve_package_name(package)?)
		})?;
		Ok(Self { packages })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.packages.len().truncate())?;
		for p in &self.packages {
			let package_idx = cp.add_package(p.clone());
			info.write_u16::<BigEndian>(package_idx)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct ModuleMainClassAttribute {
	pub main_class: String,
}

impl ModuleMainClassAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let main_class = cp.resolve_class_name(cp.get_class(buffer.read_u16::<BigEndian>()?)?)?;
		Ok(Self { main_class })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_class(self.main_class.clone());
		info.write_u16::<BigEndian>(idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct NestHostAttribute {
	pub host_class: String,
}

impl NestHostAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let host_class = cp.resolve_class_name(cp.get_class(buffer.read_u16::<BigEndian>()?)?)?;
		Ok(Self { host_class })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_class(self.host_class.clone());
		info.write_u16::<BigEndian>(idx)?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct NestMembersAttribute {
	pub member_classes: Vec<String>,
}

impl NestMembersAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_classes = usize::from(buffer.read_u16::<BigEndian>()?);
		let member_classes = buffer.read_vec_with(n_classes, |b| {
			Ok(cp.resolve_class_name(cp.get_class(b.read_u16::<BigEndian>()?)?)?)
		})?;
		Ok(Self { member_classes })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.member_classes.len().truncate())?;
		for c in &self.member_classes {
			let idx = cp.add_class(c.clone());
			info.write_u16::<BigEndian>(idx)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RecordComponent {
	pub name: String,
	pub descriptor: Descriptor,
	pub attributes: Vec<LIRRecordComponentAttribute>,
}

impl RecordComponent {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let name_idx = buffer.read_u16::<BigEndian>()?;
		let descriptor_idx = buffer.read_u16::<BigEndian>()?;
		let attr_count = usize::from(buffer.read_u16::<BigEndian>()?);

		let name = cp.get_utf8(name_idx)?;
		let descriptor = cp.get_utf8(descriptor_idx)?.parse()?;
		let attributes = buffer.read_vec_with(attr_count, |b| {
			let attr_raw = AttributeInfo::read(b)?;
			LIRRecordComponentAttribute::parse(&attr_raw, cp)
		})?;
		Ok(Self {
			name,
			descriptor,
			attributes,
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let name_idx = cp.add_utf8(self.name.clone());
		let desc_idx = cp.add_utf8(self.descriptor.jvm_repr());
		info.write_u16::<BigEndian>(name_idx)?;
		info.write_u16::<BigEndian>(desc_idx)?;

		info.write_u16::<BigEndian>(self.attributes.len().truncate())?;
		for attr in &self.attributes {
			attr.write(cp)?.write(info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct RecordAttribute {
	pub components: Vec<RecordComponent>,
}

impl RecordAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_components = usize::from(buffer.read_u16::<BigEndian>()?);
		let components = buffer.read_vec_with(n_components, |b| RecordComponent::parse(b, cp))?;
		Ok(Self { components })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.components.len().truncate())?;
		for c in &self.components {
			c.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct PermittedSubclassesAttribute {
	pub subclasses: Vec<String>,
}

impl PermittedSubclassesAttribute {
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_classes = buffer.read_u16::<BigEndian>()? as usize;
		let subclasses = buffer.read_vec_with(n_classes, |b| {
			let index = b.read_u16::<BigEndian>()?;
			let class = cp.get_class(index)?;
			Ok(cp.resolve_class_name(class)?)
		})?;
		Ok(Self { subclasses })
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16::<BigEndian>(self.subclasses.len().truncate())?;
		for ele in &self.subclasses {
			let idx = cp.add_class(ele.clone());
			info.write_u16::<BigEndian>(idx)?;
		}
		Ok(())
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
	pub fn parse<B: ReadBytesExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
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
				class_name: cp.resolve_class_name(cp.get_class(buffer.read_u16::<BigEndian>()?)?)?,
			},
			8 => Self::UninitializedVariableInfo {
				offset: buffer.read_u16::<BigEndian>()?,
			},
			tag => bail!("Unrecognized verification type info tag: {tag}"),
		})
	}

	pub fn write<W: WriteBytesExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
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
				info.write_u16::<BigEndian>(idx)?;
			}
			VerificationTypeInfo::UninitializedVariableInfo { offset } => {
				info.write_u8(8)?;
				info.write_u16::<BigEndian>(*offset)?;
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

	pub fn write<W: WriteBytesExt>(&self, info: &mut W) -> Result<()> {
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
	pub fn parse<B: ReadBytesExt>(target_type: u8, buffer: &mut B) -> Result<Self> {
		Ok(match target_type {
			// 4.7.20-A
			0x00 | 0x01 => RuntimeTypeAnnotationTargetInfo::TypeParameterTarget {
				target_type,
				type_param_index: buffer.read_u8()?,
			},
			0x10 => RuntimeTypeAnnotationTargetInfo::SupertypeTarget {
				supertype_index: buffer.read_u16::<BigEndian>()?,
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

				RuntimeTypeAnnotationTargetInfo::LocalvarTarget { target_type, table }
			}
			0x42 => RuntimeTypeAnnotationTargetInfo::CatchTarget {
				exception_table_index: buffer.read_u16::<BigEndian>()?,
			},
			0x43..=0x46 => RuntimeTypeAnnotationTargetInfo::OffsetTarget {
				target_type,
				offset: buffer.read_u16::<BigEndian>()?,
			},
			0x47..=0x4B => RuntimeTypeAnnotationTargetInfo::TypeArgumentTarget {
				target_type,
				offset: buffer.read_u16::<BigEndian>()?,
				type_argument_index: buffer.read_u8()?,
			},

			target_type => bail!("Unknown RuntimeTypeAnnotationTargetInfo target_type: {}", target_type),
		})
	}

	pub fn write<W: WriteBytesExt>(&self, info: &mut W) -> Result<()> {
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
				info.write_u16::<BigEndian>(*supertype_index)?;
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
				info.write_u16::<BigEndian>(*throws_type_index)?;
			}
			Self::LocalvarTarget { target_type, table } => {
				info.write_u8(*target_type)?;
				info.write_u16::<BigEndian>(table.len().truncate())?;
				for entry in table {
					info.write_u16::<BigEndian>(entry.start_pc)?;
					info.write_u16::<BigEndian>(entry.length)?;
					info.write_u16::<BigEndian>(entry.index)?;
				}
			}
			Self::CatchTarget { exception_table_index } => {
				info.write_u8(0x42)?;
				info.write_u16::<BigEndian>(*exception_table_index)?;
			}
			Self::OffsetTarget { target_type, offset } => {
				info.write_u8(*target_type)?;
				info.write_u16::<BigEndian>(*offset)?;
			}
			Self::TypeArgumentTarget {
				target_type,
				offset,
				type_argument_index,
			} => {
				info.write_u8(*target_type)?;
				info.write_u16::<BigEndian>(*offset)?;
				info.write_u8(*type_argument_index)?;
			}
		}
		Ok(())
	}
}

#[must_use]
pub fn bsm_eq(bsm: &BootstrapMethod, handle: &LIRMethodHandle, args: &[BootstrapMethodArgument]) -> bool {
	if &bsm.method != handle {
		return false;
	}
	if bsm.arguments.len() != args.len() {
		return false;
	}
	for (a, b) in bsm.arguments.iter().zip(args.iter()) {
		let match_arg = match (a, b) {
			(BootstrapMethodArgument::Int(x), BootstrapMethodArgument::Int(y)) => x == y,
			(BootstrapMethodArgument::Long(x), BootstrapMethodArgument::Long(y)) => x == y,
			(BootstrapMethodArgument::Float(x), BootstrapMethodArgument::Float(y)) => x.to_bits() == y.to_bits(),
			(BootstrapMethodArgument::Double(x), BootstrapMethodArgument::Double(y)) => x.to_bits() == y.to_bits(),
			(BootstrapMethodArgument::String(x), BootstrapMethodArgument::String(y))
			| (BootstrapMethodArgument::Class(x), BootstrapMethodArgument::Class(y)) => x == y,
			(BootstrapMethodArgument::MethodHandle(x), BootstrapMethodArgument::MethodHandle(y)) => x == y,
			(BootstrapMethodArgument::MethodType(x), BootstrapMethodArgument::MethodType(y)) => x == y,
			_ => false,
		};
		if !match_arg {
			return false;
		}
	}
	true
}
