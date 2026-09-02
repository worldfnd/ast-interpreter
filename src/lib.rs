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
mod intrinsics;
mod projection;
mod value;

#[cfg(test)]
mod capability;
#[cfg(test)]
mod corpus;
#[cfg(test)]
mod ledger;
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
    ComparableError, CrossFieldDump, DUMP_FORMAT_VERSION, DiffOutcome, DiffValue, DumpProvenance,
    FailureKind, RunRecord, StepOutcome, comparable_error_of, failure_kind_of, is_coverage_gap,
    normalize_text, outcome_is_tolerated, outcomes_equivalent, values_equivalent,
};
pub use error::InterpretError;
pub use input::{expected_return_from_prover_toml, inputs_from_prover_toml};
pub use projection::{PROJECTION_VERSION, canonical_text, projection_hash};
pub use value::{IntValue, Value};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use noirc_frontend::monomorphization::ast::{FuncId, Function, GlobalId, LocalId, Program};

/// Per-call-frame local environment: each `LocalId` is unique within a monomorphized function.
type Frame = HashMap<LocalId, Value>;

/// A value, or a loop-control signal that unwinds to the nearest enclosing loop.
enum Flow {
    Normal(Value),
    Break,
    Continue,
}

enum GlobalState {
    InProgress,
    Done(Value),
}

pub(crate) struct Interpreter<'p> {
    program: &'p Program,
    globals: HashMap<GlobalId, GlobalState>,
    /// Whether the current function is unconstrained; drives `is_unconstrained()`. The monomorphizer
    /// emits a separate variant per function, so this is just the current function's own flag.
    unconstrained: bool,
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
    let main = main_function_of(program)?;
    if main.parameters.len() != inputs.len() {
        return Err(InterpretError::InvalidInput(format!(
            "entry point expects {} input(s), got {}",
            main.parameters.len(),
            inputs.len()
        )));
    }
    interp.call_function(main.id, inputs)
}

pub(crate) fn function_of(program: &Program, id: FuncId) -> Result<&Function, InterpretError> {
    program
        .functions
        .get(id.0 as usize)
        .filter(|f| f.id == id)
        .ok_or_else(|| InterpretError::Internal(format!("unknown function id {id}")))
}

pub(crate) fn main_function_of(program: &Program) -> Result<&Function, InterpretError> {
    function_of(program, Program::main_id())
        .map_err(|_| InterpretError::Internal("program has no entry point".to_string()))
}

impl<'p> Interpreter<'p> {
    fn new(program: &'p Program) -> Self {
        Interpreter {
            program,
            globals: HashMap::new(),
            unconstrained: false,
        }
    }

    fn function(&self, id: FuncId) -> Result<&'p Function, InterpretError> {
        function_of(self.program, id)
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
        for ((local_id, mutable, _name, _typ, _vis), arg) in func.parameters.iter().zip(args) {
            // A `mut` param is a mutable slot; a `&mut`-typed param isn't `mutable`, so it binds
            // directly to the passed `Ref` and aliases the caller.
            let bound = if *mutable {
                Value::Ref(Rc::new(RefCell::new(arg)), true)
            } else {
                arg
            };
            frame.insert(*local_id, bound);
        }
        // Enter the callee's constrained-ness for the duration of its body.
        let outer_unconstrained = self.unconstrained;
        self.unconstrained = func.unconstrained;
        let flow = self.eval(&func.body, &mut frame);
        self.unconstrained = outer_unconstrained;
        match flow? {
            Flow::Normal(value) => Ok(value),
            Flow::Break | Flow::Continue => Err(InterpretError::Internal(
                "break/continue escaped a function body".to_string(),
            )),
        }
    }
}
