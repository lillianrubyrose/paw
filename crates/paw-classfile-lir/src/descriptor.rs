use std::{collections::VecDeque, fmt::Write as _, str::FromStr};

use thiserror::Error;

#[derive(Debug, Clone)]
pub enum Descriptor {
	Byte,
	Char,
	Double,
	Float,
	Int,
	Long,
	Short,
	Boolean,
	Object(String),
	Array(Box<Descriptor>),
}

impl FromStr for Descriptor {
	type Err = DescriptorParseErr;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		DescriptorReader::new(s)
			.next()
			.ok_or(DescriptorParseErr::EndOfInput)
			.flatten()
	}
}

impl Descriptor {
	pub fn jvm_repr(&self) -> String {
		match self {
			Descriptor::Byte => "B".to_string(),
			Descriptor::Char => "C".to_string(),
			Descriptor::Double => "D".to_string(),
			Descriptor::Float => "F".to_string(),
			Descriptor::Int => "I".to_string(),
			Descriptor::Long => "J".to_string(),
			Descriptor::Short => "S".to_string(),
			Descriptor::Boolean => "Z".to_string(),
			Descriptor::Object(name) => format!("L{name};"),
			Descriptor::Array(desc) => format!("[{}", desc.jvm_repr()),
		}
	}
}

/// reads a sequence of descriptors
pub struct DescriptorReader {
	inner: VecDeque<char>,
}

impl DescriptorReader {
	pub fn new(inner: impl AsRef<str>) -> Self {
		Self {
			inner: inner.as_ref().chars().collect(),
		}
	}
}

impl Iterator for DescriptorReader {
	type Item = Result<Descriptor, DescriptorParseErr>;

	fn next(&mut self) -> Option<Result<Descriptor, DescriptorParseErr>> {
		let s = self.inner.pop_front()?;
		match s {
			'B' => Some(Ok(Descriptor::Byte)),
			'C' => Some(Ok(Descriptor::Char)),
			'D' => Some(Ok(Descriptor::Double)),
			'F' => Some(Ok(Descriptor::Float)),
			'I' => Some(Ok(Descriptor::Int)),
			'J' => Some(Ok(Descriptor::Long)),
			'S' => Some(Ok(Descriptor::Short)),
			'Z' => Some(Ok(Descriptor::Boolean)),
			'L' => {
				let mut name = String::new();
				loop {
					match self.inner.pop_front() {
						Some(';') => break,
						Some(c) => name.push(c),
						// a descriptor was started but not finished
						None => return Some(Err(DescriptorParseErr::IncompleteObject)),
					}
				}
				Some(Ok(Descriptor::Object(name)))
			}
			'[' => match self.next()? {
				Ok(desc) => Some(Ok(Descriptor::Array(Box::new(desc)))),
				e @ Err(_) => Some(e),
			},
			c => Some(Err(DescriptorParseErr::UnknownDescriptorStart(c))),
		}
	}
}

#[derive(Debug, Error)]
pub enum DescriptorParseErr {
	#[error("unkown start of descriptor character: {0}")]
	UnknownDescriptorStart(char),
	#[error("object descriptor missing ending ;")]
	IncompleteObject,
	#[error("end of input when parsing descriptor")]
	EndOfInput,
	#[error("method descriptor did not start with a '('")]
	MissingStartParen,
	#[error("method descriptor missing closing ')'")]
	MissingEndParen,
}

#[derive(Debug, Clone)]
pub struct MethodDescriptor {
	pub ret: Option<Descriptor>,
	pub args: Vec<Descriptor>,
}

impl FromStr for MethodDescriptor {
	type Err = DescriptorParseErr;

	// (III[[Ljava/lang/String;)V
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		// (ILjava/lang/String;)V -> ILjava/lang/String;)V
		let Some(arg_start) = s.strip_prefix('(') else {
			return Err(DescriptorParseErr::MissingStartParen);
		};
		// ILjava/lang/String;)V -> [ILjava/lang/String;, V]
		let Some((args_raw, ret)) = arg_start.split_once(')') else {
			return Err(DescriptorParseErr::MissingEndParen);
		};

		let args = DescriptorReader::new(args_raw).collect::<Result<Vec<_>, _>>()?;
		let ret = if ret == "V" { None } else { Some(ret.parse()?) };

		Ok(Self { ret, args })
	}
}

impl MethodDescriptor {
	pub fn jvm_repr(&self) -> String {
		let mut s = String::new();
		s.push('(');
		for a in self.args.iter() {
			let _ = write!(&mut s, "{}", a.jvm_repr());
		}
		match &self.ret {
			Some(ret) => {
				let _ = write!(&mut s, "{}", ret.jvm_repr());
			}
			None => s.push('V'),
		}
		s
	}
}
