//! A tree-walking interpreter for Noir's monomorphized AST.
//!
//! Integers are kept as native `BigInt` values with explicit width and signedness; `Field` values
//! use the compiled-in [`acvm::FieldElement`]. Running the same program under bn254 and Goldilocks
//! then lets tests compare the field-independent results (integers, bools, arrays, tuples, structs).

#[cfg(feature = "mavros-oracle")]
compile_error!(
    "the `mavros-oracle` feature needs the mavros-compiler dependency, blocked on the Mavros Goldilocks branch"
);

mod diff;
mod error;
mod eval;
mod input;
mod value;

#[cfg(test)]
mod loader;
#[cfg(test)]
mod noir_oracle;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validation_frontend;
#[cfg(test)]
mod value_proptest;

pub use diff::{
    CrossFieldDump, DUMP_FORMAT_VERSION, DiffOutcome, DiffValue, DumpProvenance, FailureKind,
    failure_kind_of, outcome_is_tolerated, outcomes_equivalent, values_equivalent,
};
pub use error::InterpretError;
pub use input::{expected_return_from_prover_toml, inputs_from_prover_toml};
pub use value::{IntValue, Value};

use std::collections::HashMap;

use noirc_frontend::monomorphization::ast::{FuncId, Function, GlobalId, LocalId, Program};

/// Per-call-frame local environment: each `LocalId` is unique within a monomorphized function.
type Frame = HashMap<LocalId, Value>;

/// A value, or a loop-control signal that unwinds to the nearest enclosing loop.
enum Flow {
    Normal(Value),
    Break,
    Continue,
}

pub(crate) struct Interpreter<'p> {
    program: &'p Program,
    globals: HashMap<GlobalId, Value>,
}

/// Interpret `program`'s entry point with no inputs (for self-checking programs whose `main`
/// takes no parameters).
pub fn interpret(program: &Program) -> Result<Value, InterpretError> {
    interpret_with_inputs(program, Vec::new())
}

/// Interpret `program`'s entry point, binding `inputs` to `main`'s parameters in order.
///
/// Use [`inputs_from_prover_toml`] to build `inputs` from a `Prover.toml` file and the ABI.
pub fn interpret_with_inputs(
    program: &Program,
    inputs: Vec<Value>,
) -> Result<Value, InterpretError> {
    let mut interp = Interpreter::new(program);
    let main = interp.main_function()?;
    if main.parameters.len() != inputs.len() {
        return Err(InterpretError::Unsupported(format!(
            "entry point expects {} input(s), got {}",
            main.parameters.len(),
            inputs.len()
        )));
    }
    interp.call_function(main.id, inputs)
}

impl<'p> Interpreter<'p> {
    fn new(program: &'p Program) -> Self {
        Interpreter {
            program,
            globals: HashMap::new(),
        }
    }

    /// The program's entry point, looked up by its canonical [`Program::main_id`] rather than by
    /// position.
    fn main_function(&self) -> Result<&'p Function, InterpretError> {
        self.function(Program::main_id())
    }

    fn function(&self, id: FuncId) -> Result<&'p Function, InterpretError> {
        self.program
            .functions
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| InterpretError::Internal(format!("unknown function id {id}")))
    }

    fn call_function(&mut self, id: FuncId, args: Vec<Value>) -> Result<Value, InterpretError> {
        let func = self.function(id)?;
        if func.parameters.len() != args.len() {
            return Err(InterpretError::Internal(format!(
                "function {id} expects {} arguments, got {}",
                func.parameters.len(),
                args.len()
            )));
        }
        let mut frame = Frame::new();
        for ((local_id, _mutable, _name, _typ, _vis), arg) in func.parameters.iter().zip(args) {
            frame.insert(*local_id, arg);
        }
        match self.eval(&func.body, &mut frame)? {
            Flow::Normal(value) => Ok(value),
            Flow::Break | Flow::Continue => Err(InterpretError::Internal(
                "break/continue escaped a function body".to_string(),
            )),
        }
    }
}
