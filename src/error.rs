use noirc_errors::Location;

/// A failure encountered while interpreting the monomorphized AST.
///
/// `AssertionFailed` is the oracle's signal: a program's own `assert`s encode its expected results,
/// so a failed assertion means the AST does not compute what the source claims.
#[derive(Debug)]
#[non_exhaustive]
pub enum InterpretError {
    /// `assert`/`constrain` evaluated to false.
    AssertionFailed {
        location: Location,
        message: Option<String>,
    },
    /// Checked integer arithmetic overflowed the operand type.
    Overflow(String),
    /// Integer or field division by zero.
    DivisionByZero,
    /// Runtime value outside the range its operation admits.
    ValueOutOfRange(String),
    /// Caller input (`Prover.toml` / ABI) that does not match the program.
    InvalidInput(String),
    /// Runtime type mismatch indicating an invalid AST or interpreter bug.
    Type(String),
    /// AST construct the interpreter does not yet handle.
    Unsupported(String),
    /// Interpreter invariant violated.
    Internal(String),
}

impl std::fmt::Display for InterpretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpretError::AssertionFailed { message, .. } => match message {
                Some(m) => write!(f, "assertion failed: {m}"),
                None => write!(f, "assertion failed"),
            },
            InterpretError::Overflow(op) => write!(f, "integer overflow in {op}"),
            InterpretError::DivisionByZero => write!(f, "division by zero"),
            InterpretError::ValueOutOfRange(m) => write!(f, "value out of range: {m}"),
            InterpretError::InvalidInput(m) => write!(f, "invalid input: {m}"),
            InterpretError::Type(m) => write!(f, "type error: {m}"),
            InterpretError::Unsupported(m) => write!(f, "unsupported construct: {m}"),
            InterpretError::Internal(m) => write!(f, "internal interpreter error: {m}"),
        }
    }
}

impl std::error::Error for InterpretError {}
