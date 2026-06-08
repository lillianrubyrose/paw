use crate::{
	AttributeInfo, CPTag, InnerClassAccessFlags, MethodHandle, MethodHandleDescriptor, ModuleAccessFlags,
	ModuleExportAccessFlags, ModuleOpenAccessFlags, ModuleRequireAccessFlags,
	attributes::{
		RuntimeAnnotationsAttribute, RuntimeTypeAnnotationsAttribute, SignatureAttribute,
		record::LIRRecordComponentAttribute,
	},
	constant_pool::{ConstantPool, ConstantPoolIndex, MethodTypeTag},
	descriptor::{Descriptor, MethodDescriptor},
	ext::{BytesReadExt, BytesWriteExt},
};
use eyre::{Context, Result, bail};
use num_conv::Truncate;

#[derive(Debug, Clone)]
pub enum ClassAttributeKind {
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

impl ClassAttributeKind {
	#[must_use]
	pub fn name(&self) -> &str {
		match self {
			ClassAttributeKind::SourceFile(..) => "SourceFile",
			ClassAttributeKind::InnerClasses(..) => "InnerClasses",
			ClassAttributeKind::EnclosingMethod(..) => "EnclosingMethod",
			ClassAttributeKind::SourceDebugExtension(..) => "SourceDebugExtension",
			ClassAttributeKind::BootstrapMethods(..) => "BootstrapMethods",
			ClassAttributeKind::Module(..) => "Module",
			ClassAttributeKind::ModulePackages(..) => "ModulePackages",
			ClassAttributeKind::ModuleMainClass(..) => "ModuleMainClass",
			ClassAttributeKind::NestHost(..) => "NestHost",
			ClassAttributeKind::NestMembers(..) => "NestMembers",
			ClassAttributeKind::Record(..) => "Record",
			ClassAttributeKind::PermittedSubclasses(..) => "PermittedSubclasses",
			ClassAttributeKind::Synthetic => "Synthetic",
			ClassAttributeKind::Deprecated => "Deprecated",
			ClassAttributeKind::Signature(..) => "Signature",
			ClassAttributeKind::RuntimeVisibleAnnotations(..) => "RuntimeVisibleAnnotations",
			ClassAttributeKind::RuntimeInvisibleAnnotations(..) => "RuntimeInvisibleAnnotations",
			ClassAttributeKind::RuntimeVisibleTypeAnnotations(..) => "RuntimeVisibleTypeAnnotations",
			ClassAttributeKind::RuntimeInvisibleTypeAnnotations(..) => "RuntimeInvisibleTypeAnnotations",
			ClassAttributeKind::Unknown(name) => name.as_str(),
		}
	}

	pub fn parse(raw: &AttributeInfo, cp: &ConstantPool) -> Result<Self> {
		let name = cp.get_utf8(raw.attribute_name_index)?;

		let mut buffer = raw.info.as_slice();
		let kind = match name.as_ref() {
			"InnerClasses" => ClassAttributeKind::InnerClasses(InnerClassesAttribute::parse(&mut buffer, cp)?),
			"EnclosingMethod" => ClassAttributeKind::EnclosingMethod(EnclosingMethodAttribute::parse(&mut buffer, cp)?),
			"Synthetic" => ClassAttributeKind::Synthetic,
			"Signature" => ClassAttributeKind::Signature(SignatureAttribute::parse(&mut buffer, cp)?),
			"SourceFile" => ClassAttributeKind::SourceFile(SourceFileAttribute::parse(&mut buffer, cp)?),
			"SourceDebugExtension" => {
				ClassAttributeKind::SourceDebugExtension(DebugExtensionAttribute::parse(&mut buffer)?)
			}
			"Deprecated" => ClassAttributeKind::Deprecated,
			"RuntimeVisibleAnnotations" => {
				ClassAttributeKind::RuntimeVisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeInvisibleAnnotations" => {
				ClassAttributeKind::RuntimeInvisibleAnnotations(RuntimeAnnotationsAttribute::parse(&mut buffer, cp)?)
			}
			"RuntimeVisibleTypeAnnotations" => ClassAttributeKind::RuntimeVisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"RuntimeInvisibleTypeAnnotations" => ClassAttributeKind::RuntimeInvisibleTypeAnnotations(
				RuntimeTypeAnnotationsAttribute::parse(&mut buffer, cp)?,
			),
			"BootstrapMethods" => {
				ClassAttributeKind::BootstrapMethods(BootstrapMethodsAttribute::parse(&mut buffer, cp)?)
			}
			"Module" => ClassAttributeKind::Module(ModuleAttribute::parse(&mut buffer, cp)?),
			"ModulePackages" => ClassAttributeKind::ModulePackages(ModulePackagesAttribute::parse(&mut buffer, cp)?),
			"ModuleMainClass" => ClassAttributeKind::ModuleMainClass(ModuleMainClassAttribute::parse(&mut buffer, cp)?),
			"NestHost" => ClassAttributeKind::NestHost(NestHostAttribute::parse(&mut buffer, cp)?),
			"NestMembers" => ClassAttributeKind::NestMembers(NestMembersAttribute::parse(&mut buffer, cp)?),
			"Record" => ClassAttributeKind::Record(RecordAttribute::parse(&mut buffer, cp)?),
			"PermittedSubclasses" => {
				ClassAttributeKind::PermittedSubclasses(PermittedSubclassesAttribute::parse(&mut buffer, cp)?)
			}
			_ => {
				eprintln!("WARN: unknown attribute {}", name);
				ClassAttributeKind::Unknown(name.clone())
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
			ClassAttributeKind::SourceFile(s) => {
				s.write(cp, &mut info)?;
			}
			ClassAttributeKind::InnerClasses(ic) => {
				ic.write(cp, &mut info)?;
			}
			ClassAttributeKind::EnclosingMethod(em) => {
				em.write(cp, &mut info)?;
			}
			ClassAttributeKind::SourceDebugExtension(sde) => {
				sde.write(&mut info)?;
			}
			ClassAttributeKind::BootstrapMethods(bsm) => {
				bsm.write(cp, &mut info)?;
			}
			ClassAttributeKind::Module(m) => {
				m.write(cp, &mut info)?;
			}
			ClassAttributeKind::ModulePackages(mp) => {
				mp.write(cp, &mut info)?;
			}
			ClassAttributeKind::ModuleMainClass(mc) => {
				mc.write(cp, &mut info)?;
			}
			ClassAttributeKind::NestHost(nh) => {
				nh.write(cp, &mut info)?;
			}
			ClassAttributeKind::NestMembers(nm) => {
				nm.write(cp, &mut info)?;
			}
			ClassAttributeKind::Record(r) => {
				r.write(cp, &mut info)?;
			}
			ClassAttributeKind::PermittedSubclasses(ps) => {
				ps.write(cp, &mut info)?;
			}
			ClassAttributeKind::Synthetic | ClassAttributeKind::Deprecated => {}
			ClassAttributeKind::Signature(sig) => {
				sig.write(cp, &mut info)?;
			}
			ClassAttributeKind::RuntimeVisibleAnnotations(ra) | ClassAttributeKind::RuntimeInvisibleAnnotations(ra) => {
				ra.write(cp, &mut info)?;
			}
			ClassAttributeKind::RuntimeVisibleTypeAnnotations(rta)
			| ClassAttributeKind::RuntimeInvisibleTypeAnnotations(rta) => {
				rta.write(cp, &mut info)?;
			}
			ClassAttributeKind::Unknown(_) => unreachable!("Class Attribute 'Unknown' should never be written"),
		}

		Ok(AttributeInfo {
			attribute_name_index,
			info,
		})
	}
}

#[derive(Debug, Clone)]
pub struct SourceFileAttribute {
	pub source_file: String,
}

impl SourceFileAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let sourcefile_index = buffer
			.read_u16()
			.wrap_err("failed to read sourcefile_index from SourceFile attribute")?;
		let source_file = cp.get_utf8(ConstantPoolIndex::new_internal(sourcefile_index))?;
		Ok(Self { source_file })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_utf8(self.source_file.clone());
		info.write_u16(idx.get())?;
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
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let inner_info_idx = buffer.read_u16()?;
		let outer_info_idx = buffer.read_u16()?;
		let inner_name_idx = buffer.read_u16()?;
		let inner_class_access_flags = InnerClassAccessFlags::try_from(buffer.read_u16()?)?;

		let inner_class_info = cp.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(inner_info_idx))?)?;
		let outer_class_info = if outer_info_idx != 0 {
			Some(cp.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(outer_info_idx))?)?)
		} else {
			None
		};
		let inner_name = if inner_name_idx != 0 {
			Some(cp.get_utf8(ConstantPoolIndex::new_internal(inner_name_idx))?)
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

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let inner_info_idx = cp.add_class(self.inner_class_info.clone());
		let outer_info_idx = if let Some(ref outer) = self.outer_class_info {
			cp.add_class(outer.clone())
		} else {
			ConstantPoolIndex::new_internal(0)
		};
		let inner_name_idx = if let Some(ref name) = self.inner_name {
			cp.add_utf8(name.clone())
		} else {
			ConstantPoolIndex::new_internal(0)
		};
		info.write_u16(inner_info_idx.get())?;
		info.write_u16(outer_info_idx.get())?;
		info.write_u16(inner_name_idx.get())?;
		info.write_u16(self.inner_class_access_flags.bits())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct InnerClassesAttribute {
	pub classes: Vec<InnerClassesAttributeClass>,
}

impl InnerClassesAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_classes = usize::from(
			buffer
				.read_u16()
				.wrap_err("failed to read number_of_classes from InnerClasses attribute")?,
		);
		let classes = buffer.read_vec_with(n_classes, |b| InnerClassesAttributeClass::parse(b, cp))?;
		Ok(InnerClassesAttribute { classes })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.classes.len().truncate())?;
		for class in &self.classes {
			class.write(cp, info)?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct EnclosingMethodAttribute {
	pub class: String,               // Class
	pub method_name: Option<String>, // NameAndType
	pub method_descriptor: Option<MethodDescriptor>,
}

impl EnclosingMethodAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let class_idx = buffer
			.read_u16()
			.wrap_err("failed to read class_index from EnclosingMethod attribute")?;
		let method_idx = buffer
			.read_u16()
			.wrap_err("failed to read method_index from EnclosingMethod attribute")?;

		let class = cp.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(class_idx))?)?;
		let (method_name, method_descriptor) = if method_idx == 0 {
			(None, None)
		} else {
			let method_name_and_ty = cp.get_name_and_type(ConstantPoolIndex::new_internal(method_idx))?;
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

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let class_idx = cp.add_class(self.class.clone());
		let method_name_and_ty = if let Some(method_name) = &self.method_name
			&& let Some(method_desc) = &self.method_descriptor
		{
			cp.add_name_and_type(method_name.clone(), method_desc.jvm_repr())
		} else {
			ConstantPoolIndex::new_internal(0)
		};
		info.write_u16(class_idx.get())?;
		info.write_u16(method_name_and_ty.get())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct DebugExtensionAttribute {
	pub debug_data: String,
}

impl DebugExtensionAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B) -> Result<Self> {
		// the debug extension just uses the entire length of the attribute data
		let mut buf = Vec::new();
		buffer.read_to_end(&mut buf)?;
		let debug_data = paw_mutf8::decode(&buf)?.to_string();
		Ok(Self { debug_data })
	}

	pub fn write<W: BytesWriteExt>(&self, info: &mut W) -> Result<()> {
		let encoded = paw_mutf8::encode(&self.debug_data);
		info.write_u16(encoded.len().truncate())?;
		info.write_all(&encoded)?;
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
	MethodHandle(MethodHandle),
	MethodType(MethodDescriptor),
}

#[derive(Debug, Clone)]
pub struct BootstrapMethod {
	pub method: MethodHandle,
	pub arguments: Vec<BootstrapMethodArgument>,
}

impl BootstrapMethod {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let method_idx = ConstantPoolIndex::new_internal(buffer.read_u16()?);
		let n_args = buffer.read_u16()? as usize;
		let argument_idxs = buffer.read_vec_with(n_args, |b| Ok(ConstantPoolIndex::new_internal(b.read_u16()?)))?;

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
						BootstrapMethodArgument::MethodHandle(MethodHandle::resolve(cp, idx)?)
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
			method: MethodHandle::resolve(cp, method_idx)?,
			arguments,
		})
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let method_ref = match &self.method.descriptor {
			MethodHandleDescriptor::Field(f) => {
				cp.add_field_ref(self.method.owner.clone(), self.method.name.clone(), f.jvm_repr())
			}
			MethodHandleDescriptor::Method(d) => {
				if self.method.is_interface {
					cp.add_interface_method_ref(self.method.owner.clone(), self.method.name.clone(), d.jvm_repr())
				} else {
					cp.add_method_ref(self.method.owner.clone(), self.method.name.clone(), d.jvm_repr())
				}
			}
		};

		let handle = cp.add_method_handle(self.method.kind, method_ref);
		info.write_u16(handle.get())?;

		info.write_u16(self.arguments.len().truncate())?;
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
						MethodHandleDescriptor::Field(f) => {
							cp.add_field_ref(h.owner.clone(), h.name.clone(), f.jvm_repr())
						}
						MethodHandleDescriptor::Method(d) => {
							if h.is_interface {
								cp.add_interface_method_ref(h.owner.clone(), h.name.clone(), d.jvm_repr())
							} else {
								cp.add_method_ref(h.owner.clone(), h.name.clone(), d.jvm_repr())
							}
						}
					};
					cp.add_method_handle(h.kind, r)
				}
				BootstrapMethodArgument::MethodType(t) => cp.add_method_type(t.jvm_repr()),
			};
			info.write_u16(idx.get())?;
		}

		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct BootstrapMethodsAttribute {
	pub methods: Vec<BootstrapMethod>,
}

impl BootstrapMethodsAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_methods = usize::from(buffer.read_u16()?);
		let methods = buffer.read_vec_with(n_methods, |b| BootstrapMethod::parse(b, cp))?;
		Ok(Self { methods })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.methods.len().truncate())?;
		for method in &self.methods {
			method.write(cp, info)?;
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
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let index = buffer.read_u16()?;
		let name = cp.resolve_module_name(cp.get_module(ConstantPoolIndex::new_internal(index))?)?;

		let flags = ModuleAccessFlags::try_from(buffer.read_u16()?)?;

		let version_index = buffer.read_u16()?;
		let version = if version_index == 0 {
			None
		} else {
			Some(cp.get_utf8(ConstantPoolIndex::new_internal(version_index))?)
		};

		let requires_count = buffer.read_u16()?;
		let requires = buffer.read_vec_with(requires_count as usize, |b| {
			let requires_index = b.read_u16()?;
			let requires_module = cp.get_module(ConstantPoolIndex::new_internal(requires_index))?;
			let requires_flags = ModuleRequireAccessFlags::try_from(b.read_u16()?)?;
			let requires_version_index = b.read_u16()?;
			Ok(ModuleRequire {
				module: cp.resolve_module_name(requires_module)?,
				flags: requires_flags,
				version: if requires_version_index == 0 {
					None
				} else {
					Some(cp.get_utf8(ConstantPoolIndex::new_internal(requires_version_index))?)
				},
			})
		})?;

		let exports_count = buffer.read_u16()?;
		let exports = buffer.read_vec_with(exports_count as usize, |b| {
			let exports_index = b.read_u16()?;
			let exports_package = cp.get_package(ConstantPoolIndex::new_internal(exports_index))?;
			let exports_flags = ModuleExportAccessFlags::try_from(b.read_u16()?)?;
			let exports_to_count = b.read_u16()?;
			let exports_to = b.read_vec_with(exports_to_count as usize, |b2| {
				let exports_to_index = b2.read_u16()?;
				let exports_to_module = cp.get_module(ConstantPoolIndex::new_internal(exports_to_index))?;
				Ok(cp.resolve_module_name(exports_to_module)?)
			})?;
			Ok(ModuleExport {
				package: cp.resolve_package_name(exports_package)?,
				flags: exports_flags,
				exports_to,
			})
		})?;

		let opens_count = buffer.read_u16()?;
		let opens = buffer.read_vec_with(opens_count as usize, |b| {
			let opens_index = b.read_u16()?;
			let opens_flags = ModuleOpenAccessFlags::try_from(b.read_u16()?)?;
			let opens_to_count = b.read_u16()?;
			let opens_to = b.read_vec_with(opens_to_count as usize, |b2| {
				let opens_to_index = b2.read_u16()?;
				let opens_to_module = cp.get_module(ConstantPoolIndex::new_internal(opens_to_index))?;
				Ok(cp.resolve_module_name(opens_to_module)?)
			})?;

			let opens_package = cp.get_package(ConstantPoolIndex::new_internal(opens_index))?;
			Ok(ModuleOpen {
				package: cp.resolve_package_name(opens_package)?,
				flags: opens_flags,
				opens_to,
			})
		})?;

		let uses_count = buffer.read_u16()?;
		let uses = buffer.read_vec_with(uses_count as usize, |b| {
			let uses_index = b.read_u16()?;
			let uses_class = cp.get_class(ConstantPoolIndex::new_internal(uses_index))?;
			Ok(cp.resolve_class_name(uses_class)?)
		})?;

		let provides_count = buffer.read_u16()?;
		let provides = buffer.read_vec_with(provides_count as usize, |b| {
			let provides_index = b.read_u16()?;
			let provides_class = cp.get_class(ConstantPoolIndex::new_internal(provides_index))?;
			let provides_with_count = b.read_u16()?;
			let providers = b.read_vec_with(provides_with_count as usize, |b2| {
				let provides_with_index = b2.read_u16()?;
				let provides_with_class = cp.get_class(ConstantPoolIndex::new_internal(provides_with_index))?;
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

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let name_idx = cp.add_module(self.name.clone());
		info.write_u16(name_idx.get())?;
		info.write_u16(self.flags.bits())?;

		let version_idx = self.version.as_ref().map_or(0, |v| cp.add_utf8(v.clone()).get());
		info.write_u16(version_idx)?;

		info.write_u16(self.requires.len().truncate())?;
		for r in &self.requires {
			let requires_idx = cp.add_module(r.module.clone());
			info.write_u16(requires_idx.get())?;
			info.write_u16(r.flags.bits())?;

			let version_idx = r.version.as_ref().map_or(0, |v| cp.add_utf8(v.clone()).get());
			info.write_u16(version_idx)?;
		}

		info.write_u16(self.exports.len().truncate())?;
		for e in &self.exports {
			let exports_idx = cp.add_package(e.package.clone());
			info.write_u16(exports_idx.get())?;
			info.write_u16(e.flags.bits())?;

			info.write_u16(e.exports_to.len().truncate())?;
			for ele in &e.exports_to {
				let exports_to_idx = cp.add_module(ele.clone());
				info.write_u16(exports_to_idx.get())?;
			}
		}

		info.write_u16(self.opens.len().truncate())?;
		for o in &self.opens {
			let opens_index = cp.add_package(o.package.clone());
			info.write_u16(opens_index.get())?;
			info.write_u16(o.flags.bits())?;

			info.write_u16(o.opens_to.len().truncate())?;
			for ot in &o.opens_to {
				let opens_to_index = cp.add_module(ot.clone());
				info.write_u16(opens_to_index.get())?;
			}
		}

		info.write_u16(self.uses.len().truncate())?;
		for u in &self.uses {
			let uses_index = cp.add_class(u.clone());
			info.write_u16(uses_index.get())?;
		}

		info.write_u16(self.provides.len().truncate())?;
		for p in &self.provides {
			let provides_index = cp.add_class(p.service.clone());
			info.write_u16(provides_index.get())?;

			info.write_u16(p.providers.len().truncate())?;
			for ele in &p.providers {
				let provider_idx = cp.add_class(ele.clone());
				info.write_u16(provider_idx.get())?;
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
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let package_count = buffer.read_u16()?;
		let packages = buffer.read_vec_with(package_count as usize, |b| {
			let package_index = b.read_u16()?;
			let package = cp.get_package(ConstantPoolIndex::new_internal(package_index))?;
			Ok(cp.resolve_package_name(package)?)
		})?;
		Ok(Self { packages })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.packages.len().truncate())?;
		for p in &self.packages {
			let package_idx = cp.add_package(p.clone());
			info.write_u16(package_idx.get())?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct ModuleMainClassAttribute {
	pub main_class: String,
}

impl ModuleMainClassAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let main_class = cp.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(buffer.read_u16()?))?)?;
		Ok(Self { main_class })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_class(self.main_class.clone());
		info.write_u16(idx.get())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct NestHostAttribute {
	pub host_class: String,
}

impl NestHostAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let host_class = cp.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(buffer.read_u16()?))?)?;
		Ok(Self { host_class })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let idx = cp.add_class(self.host_class.clone());
		info.write_u16(idx.get())?;
		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct NestMembersAttribute {
	pub member_classes: Vec<String>,
}

impl NestMembersAttribute {
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_classes = usize::from(buffer.read_u16()?);
		let member_classes = buffer.read_vec_with(n_classes, |b| {
			Ok(cp.resolve_class_name(cp.get_class(ConstantPoolIndex::new_internal(b.read_u16()?))?)?)
		})?;
		Ok(Self { member_classes })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.member_classes.len().truncate())?;
		for c in &self.member_classes {
			let idx = cp.add_class(c.clone());
			info.write_u16(idx.get())?;
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
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let name_idx = buffer.read_u16()?;
		let descriptor_idx = buffer.read_u16()?;
		let attr_count = usize::from(buffer.read_u16()?);

		let name = cp.get_utf8(ConstantPoolIndex::new_internal(name_idx))?;
		let descriptor = cp.get_utf8(ConstantPoolIndex::new_internal(descriptor_idx))?.parse()?;
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

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		let name_idx = cp.add_utf8(self.name.clone());
		let desc_idx = cp.add_utf8(self.descriptor.jvm_repr());
		info.write_u16(name_idx.get())?;
		info.write_u16(desc_idx.get())?;

		info.write_u16(self.attributes.len().truncate())?;
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
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_components = usize::from(buffer.read_u16()?);
		let components = buffer.read_vec_with(n_components, |b| RecordComponent::parse(b, cp))?;
		Ok(Self { components })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.components.len().truncate())?;
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
	pub fn parse<B: BytesReadExt>(buffer: &mut B, cp: &ConstantPool) -> Result<Self> {
		let n_classes = buffer.read_u16()? as usize;
		let subclasses = buffer.read_vec_with(n_classes, |b| {
			let index = b.read_u16()?;
			let class = cp.get_class(ConstantPoolIndex::new_internal(index))?;
			Ok(cp.resolve_class_name(class)?)
		})?;
		Ok(Self { subclasses })
	}

	pub fn write<W: BytesWriteExt>(&self, cp: &mut ConstantPool, info: &mut W) -> Result<()> {
		info.write_u16(self.subclasses.len().truncate())?;
		for ele in &self.subclasses {
			let idx = cp.add_class(ele.clone());
			info.write_u16(idx.get())?;
		}
		Ok(())
	}
}
