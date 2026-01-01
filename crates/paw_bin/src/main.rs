use std::{
	env, fs,
	io::Cursor,
	path::{Path, PathBuf},
};

use eyre::{Context, OptionExt, Result, bail};
use paw_classfile_format::ClassFile;
use paw_classfile_lir::class::LIRClass;

fn main() -> Result<()> {
	let mut args = env::args();
	let _arg0 = args.next();
	let path = PathBuf::from(args.next().ok_or_eyre("pass a directory or file as first arg")?);
	// if the arg is absolute it replaces the path
	let path = env::current_dir().wrap_err("unable to get current dir")?.join(path);

	if path.is_dir() {
		for entry in fs::read_dir(path)? {
			process_path(entry?.path())?;
		}
	} else if path.is_file() {
		process_path(path)?;
	} else {
		bail!("unable to determine what to do with path {}", path.display())
	}

	Ok(())
}

fn process_path(path: impl AsRef<Path>) -> Result<()> {
	let path = path.as_ref();
	if !path.is_file() {
		eprintln!("skipping non-file path `{}`", path.display());
		return Ok(());
	}
	if path.extension().is_none_or(|ext| ext != "class") {
		eprintln!("skipping non-class file `{}`", path.display());
		return Ok(());
	}
	let data = fs::read(path).unwrap();
	process_class(&data)
}

fn process_class(data: &[u8]) -> Result<()> {
	let mut cursor = Cursor::new(data);
	let cf = ClassFile::read(&mut cursor)?;
	let lir_cf = LIRClass::parse(cf);
	println!("{lir_cf:#?}");
	Ok(())
}
