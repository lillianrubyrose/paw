use std::{
	fs::{File, OpenOptions},
	io::{Cursor, Read, Write},
	path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use eyre::Result;
use paw_classfile::ClassFile;
use zip::{ZipArchive, ZipWriter, write::FileOptions};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Recompile
	Recompile { path: PathBuf },
	/// Recompiles every class inside a JAR and saves to a new JAR
	RecompileJar { path: PathBuf },
}

fn main() -> Result<()> {
	color_eyre::install()?;
	let cli = Cli::parse();

	match cli.command {
		Commands::Recompile { path } => recompile(&path)?,
		Commands::RecompileJar { path } => recompile_jar(&path)?,
	}

	Ok(())
}

fn recompile(path: &Path) -> Result<()> {
	let class = ClassFile::read(&mut Cursor::new(std::fs::read(path)?))?;

	let output = path.parent().unwrap().join(format!(
		"{}.paw.class",
		path.file_name()
			.unwrap()
			.to_string_lossy()
			.to_string()
			.strip_suffix(".class")
			.unwrap()
	));
	let mut output = OpenOptions::new()
		.create(true)
		.truncate(true)
		.write(true)
		.open(output)?;
	class.write(&mut output)?;
	Ok(())
}

fn recompile_jar(path: &Path) -> Result<()> {
	let file = File::open(path)?;
	let out_path = path.with_extension("paw.jar");
	let out_file = File::create(&out_path)?;

	let mut archive = ZipArchive::new(file)?;
	let mut zip_out = ZipWriter::new(out_file);
	let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
	for i in 0..archive.len() {
		let mut entry = archive.by_index(i)?;
		let name = entry.name().to_string();
		zip_out.start_file(&name, options)?;
		if name.ends_with(".class") {
			let mut buffer = Vec::new();
			entry.read_to_end(&mut buffer)?;
			let cursor = Cursor::new(buffer);
			match ClassFile::read(&mut Cursor::new(cursor.get_ref())) {
				Ok(cf) => {
					let mut lir_bytes = Vec::new();
					cf.write(&mut lir_bytes)?;
					zip_out.write_all(&lir_bytes)?;
				}
				Err(_) => {
					zip_out.write_all(cursor.get_ref())?;
				}
			}
		} else {
			std::io::copy(&mut entry, &mut zip_out)?;
		}
	}

	zip_out.finish()?;
	println!("Successfully recompiled JAR to: {}", out_path.display());
	Ok(())
}
