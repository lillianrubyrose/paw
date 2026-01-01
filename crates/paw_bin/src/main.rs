use std::{
	fs,
	io::Cursor,
	path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use eyre::{Context, OptionExt, Result};
use paw_classfile_format::{CPTag, ClassFile};
use paw_classfile_lir::class::LIRClass;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Validates a single class or all classes in a directory
	Validate { path: PathBuf },
	/// Dumps full information about a class or classes
	Class { path: PathBuf },
	/// Lists all methods in a class
	Methods { path: PathBuf },
	/// Lists all fields in a class
	Fields { path: PathBuf },
	/// Dumps information about a specific method
	Method {
		path: PathBuf,
		name: String,
		/// The method descriptor (e.g., "(Ljava/lang/String;)V")
		descriptor: String,
	},
	/// Dumps information about a specific field
	Field { path: PathBuf, name: String },
	/// Dumps the Constant Pool information
	Cp { path: PathBuf },
	/// Dumps basic class file header information
	Classinfo { path: PathBuf },
}

fn main() -> Result<()> {
	color_eyre::install()?;
	let cli = Cli::parse();

	match cli.command {
		Commands::Validate { path } => validate_path(&path)?,
		Commands::Class { path } => dump_class_path(&path)?,
		Commands::Methods { path } => list_methods(&path)?,
		Commands::Fields { path } => list_fields(&path)?,
		Commands::Method {
			path,
			name,
			descriptor: desc,
		} => dump_method(&path, &name, &desc)?,
		Commands::Field { path, name } => dump_field(&path, &name)?,
		Commands::Cp { path } => dump_cp(&path)?,
		Commands::Classinfo { path } => dump_classinfo(&path)?,
	}

	Ok(())
}

fn validate_path(path: &Path) -> Result<()> {
	if path.is_file() {
		validate_file(path);
	} else if path.is_dir() {
		for entry in fs::read_dir(path)? {
			let entry = entry?;
			validate_path(&entry.path())?;
		}
	}
	Ok(())
}

fn validate_file(path: &Path) {
	if path.extension().is_some_and(|e| e == "class") {
		print!("Validating {}", path.display());
		match parse_lir(path) {
			Ok(_) => println!("OK"),
			Err(e) => println!("FAIL\nError: {e}"),
		}
	}
}

fn read_format(path: &Path) -> Result<ClassFile> {
	let data = fs::read(path).wrap_err_with(|| format!("Failed to read {}", path.display()))?;
	let mut data = Cursor::new(data);
	ClassFile::read(&mut data).wrap_err("Failed to parse raw ClassFile")
}

fn parse_lir(path: &Path) -> Result<LIRClass> {
	let cf = read_format(path)?;
	LIRClass::parse(cf).wrap_err("Failed to parse LIRClass")
}

fn dump_class_path(path: &Path) -> Result<()> {
	if path.is_file() {
		let class = parse_lir(path)?;
		println!("{}", fmt::format_class(&class));
	} else if path.is_dir() {
		for entry in fs::read_dir(path)? {
			let entry = entry?;
			let path = entry.path();
			if path.is_file() && path.extension().map_or_else(|| false, |e| e == "class") {
				println!("======{}======", path.display());
				dump_class_path(&path)?;
				println!();
			}
		}
	}
	Ok(())
}

fn list_methods(path: &Path) -> Result<()> {
	let class = parse_lir(path)?;
	println!("Class: {}", class.this_class.replace('/', "."));
	println!("Methods:");
	for m in &class.methods {
		println!("  {}", fmt::format_method_sig(m));
	}
	Ok(())
}

fn list_fields(path: &Path) -> Result<()> {
	let class = parse_lir(path)?;
	println!("Class: {}", class.this_class.replace('/', "."));
	println!("Fields:");
	for f in &class.fields {
		println!("  {}", fmt::format_field_sig(f));
	}
	Ok(())
}

fn dump_method(path: &Path, name: &str, desc: &str) -> Result<()> {
	let class = parse_lir(path)?;
	let method = class
		.methods
		.iter()
		.find(|m| m.name == name && m.descriptor.jvm_repr() == desc)
		.ok_or_eyre(format!("Method not found: {name} {desc}"))?;

	println!("Signature: {}", fmt::format_method_sig(method));
	if !method.attributes.is_empty() {
		print!("Attributes: ");
		for attr in &method.attributes {
			println!("{:#?}", attr);
		}
	}
	Ok(())
}

fn dump_field(path: &Path, name: &str) -> Result<()> {
	let class = parse_lir(path)?;
	let field = class
		.fields
		.iter()
		.find(|f| f.name == name)
		.ok_or_eyre(format!("Field not found: {}", name))?;

	println!("Signature: {}", fmt::format_field_sig(field));
	println!("Attributes: {:#?}", field.attributes);
	Ok(())
}

fn dump_cp(path: &Path) -> Result<()> {
	let cf = read_format(path)?;
	let cp = &cf.cp;
	println!("Constant Pool ({}):", cp.len());
	for idx in 1..=cp.len() {
		#[allow(clippy::cast_possible_truncation)]
		let idx = idx as u16;
		match cp.get_tag(idx) {
			Ok(tag) => {
				print!("  #{:02} = {:<15}", idx, tag.name());
				match tag {
					CPTag::Utf8(_) => println!(" \"{}\"", cp.get_utf8(idx)?),
					CPTag::Class(_) => {
						println!(" {}", cp.resolve_class_name(cp.get_class(idx)?)?);
					}
					CPTag::String(_) => {
						println!(" \"{}\"", cp.resolve_string(cp.get_string(idx)?)?);
					}
					CPTag::Integer(v) => println!(" {v}"),
					CPTag::Float(v) => println!(" {v}"),
					CPTag::Long(v) => println!(" {v}"),
					CPTag::Double(v) => println!(" {v}"),
					_ => println!(" {:?}", tag),
				}
			}
			Err(_) => {
				println!("  #{idx:02} = (unusable)");
			}
		}
	}
	Ok(())
}

fn dump_classinfo(path: &Path) -> Result<()> {
	let cf = read_format(path)?;
	let cp = &cf.cp;

	println!("Version: {}.{}", cf.version.major, cf.version.minor);

	let access = format!("{:?}", cf.access_flags).replace(" | ", " ");
	let access = access
		.strip_prefix("ClassAccessFlags(")
		.unwrap_or(&access)
		.strip_suffix(")")
		.unwrap_or(&access)
		.to_lowercase();
	println!("Access Flags: {}", access);

	let this_class = cp.resolve_class_name(cp.get_class(cf.this_class)?)?;
	println!("This Class: {}", this_class);

	if cf.super_class != 0 {
		let super_class = cp.resolve_class_name(cp.get_class(cf.super_class)?)?;
		println!("Super Class: {}", super_class);
	} else {
		println!("Super Class: None (java/lang/Object)");
	}

	println!("Interfaces ({}):", cf.interfaces.len());
	for &iface_idx in &cf.interfaces {
		let iface = cp.resolve_class_name(cp.get_class(iface_idx)?)?;
		println!("- {}", iface);
	}

	println!("Constant Pool Count: {}", cp.len());
	println!("Fields Count: {}", cf.fields.len());
	println!("Methods Count: {}", cf.methods.len());
	println!("Attributes Count: {}", cf.attributes.len());

	Ok(())
}

mod fmt {
	use std::fmt::Write;

	use paw_classfile_format::descriptor::Descriptor;
	use paw_classfile_lir::{class::LIRClass, field::LIRField, method::LIRMethod};

	pub fn format_class(c: &LIRClass) -> String {
		let mut s = String::new();
		let _ = writeln!(
			s,
			"class {} extends {}",
			c.this_class.replace('/', "."),
			c.super_class.as_deref().unwrap_or("java/lang/Object").replace('/', ".")
		);
		let _ = writeln!(s, "\tversion: {:?}", c.version);

		let access = format!("{:?}", c.access_flags).replace(" | ", " ");
		let access = access
			.strip_prefix("ClassAccessFlags(")
			.unwrap_or(&access)
			.strip_suffix(")")
			.unwrap_or(&access)
			.to_lowercase();
		let _ = writeln!(s, "\tflags: {}", access);

		if !c.interfaces.is_empty() {
			let _ = writeln!(s, "\timplements:");
			for iface in &c.interfaces {
				let _ = writeln!(s, "\t{}", iface.replace('/', "."));
			}
		}

		let _ = writeln!(s, "\n\t# Fields");
		for f in &c.fields {
			let _ = writeln!(s, "\t{};", format_field_sig(f));
		}

		let _ = writeln!(s, "\n\t# Methods");
		for m in &c.methods {
			let _ = writeln!(s, "\t{};", format_method_sig(m));
		}
		s
	}

	pub fn format_method_sig(m: &LIRMethod) -> String {
		let access = format!("{:?}", m.access_flags).replace(" | ", " ");
		let access = access
			.strip_prefix("MethodAccessFlags(")
			.unwrap_or(&access)
			.strip_suffix(")")
			.unwrap_or(&access)
			.to_lowercase();
		let ret = m
			.descriptor
			.ret
			.as_ref()
			.map_or_else(|| "void".to_string(), format_descriptor);
		let args = m
			.descriptor
			.args
			.iter()
			.map(format_descriptor)
			.collect::<Vec<_>>()
			.join(", ");
		format!("{} {} {}({})", access, ret, m.name, args)
	}

	pub fn format_field_sig(f: &LIRField) -> String {
		let access = format!("{:?}", f.access_flags).replace(" | ", " ");
		let access = access
			.strip_prefix("FieldAccessFlags(")
			.unwrap_or(&access)
			.strip_suffix(")")
			.unwrap_or(&access)
			.to_lowercase();
		let ty = format_descriptor(&f.descriptor);
		format!("{} {} {}", access, ty, f.name)
	}

	fn format_descriptor(d: &Descriptor) -> String {
		match d {
			Descriptor::Byte => "byte".into(),
			Descriptor::Char => "char".into(),
			Descriptor::Double => "double".into(),
			Descriptor::Float => "float".into(),
			Descriptor::Int => "int".into(),
			Descriptor::Long => "long".into(),
			Descriptor::Short => "short".into(),
			Descriptor::Boolean => "boolean".into(),
			Descriptor::Object(name) => name.replace('/', "."),
			Descriptor::Array(inner) => format!("{}[]", format_descriptor(inner)),
		}
	}
}
