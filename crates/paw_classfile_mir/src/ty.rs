#[derive(Debug)]
pub enum Ty {
	// FIXME: should the integral and float types be collapsed
	Void,
	Boolean,
	Byte,
	Char,
	Short,
	Int,
	Long,
	Float,
	Double,
	Object(String),
	Array(Box<Ty>),

	TypeParam(TypeParam),
	Parametric {
		base_class: String,
		args: Vec<GenericArgument>,
	},
}

#[derive(Debug)]
pub enum TypeParam {
	/// Index into the enclosing class' type params
	Class(u16),
	/// Index into the enclosing method's type params
	Method(u16),
}

#[derive(Debug)]
pub struct NamedTypeParam {
	pub name: String,
	pub bounds: Vec<Ty>,
}

#[derive(Debug)]
pub enum GenericArgument {
	Concrete(Ty),
	Wildcard(WildcardBounds),
}

#[derive(Debug)]
pub enum WildcardBounds {
	// ?
	Unbounded,
	// ? extends Something
	Extends(Ty),
	// ? super Something
	Super(Ty),
}
