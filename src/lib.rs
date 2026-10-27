//! A tree-walking interpreter for Noir's monomorphized AST.
//!
//! Integers are kept as native `BigInt` values with explicit width and signedness; a `Field` value
//! carries the field it belongs to, taken from the label the monomorphized program was compiled
//! under. One build therefore interprets a program under any supported field, and tests compare
//! the field-independent results (integers, bools, arrays, tuples, structs) across two of them.

#[cfg(feature = "bn254-crypto")]
mod bn254_crypto;
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
mod loader;
#[cfg(all(test, feature = "mavros-oracle"))]
mod mavros_oracle;
#[cfg(test)]
mod noir_oracle;
#[cfg(test)]
mod status;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validation_frontend;
#[cfg(test)]
mod value_proptest;

pub use diff::{
    ComparableError, CrossFieldDump, DUMP_FORMAT_VERSION, DiffOutcome, DiffValue, DumpProvenance,
    FailureKind, RunRecord, StepOutcome, comparable_error_of, outcome_is_tolerated,
    outcomes_equivalent, values_equivalent,
};
pub use error::InterpretError;
pub use input::{expected_return_from_prover_toml, inputs_from_bytes, inputs_from_prover_toml};
pub use projection::{PROJECTION_VERSION, canonical_text, projection_hash};
pub use value::{IntValue, Value};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use acvm::{FieldConfig, FieldId};
use noirc_frontend::monomorphization::ast::{FuncId, Function, GlobalId, LocalId, Program, Type};

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
    /// The field this program was compiled under; every `Field` value it makes belongs to it.
    field: FieldConfig,
    globals: HashMap<GlobalId, GlobalState>,
    /// Whether the current function is unconstrained; drives `is_unconstrained()`. The monomorphizer
    /// emits a separate variant per function, so this is just the current function's own flag.
    unconstrained: bool,
    /// How many calls are in progress, bounded by [`MAX_CALL_DEPTH`].
    call_depth: usize,
}

/// Bound the host stack. Reaching this limit is an interpreter coverage gap, since Noir can
/// execute deeper finite recursion and its stack limit depends on the compiled frames.
const MAX_CALL_DEPTH: usize = 256;

/// Interpret `program`'s entry point with no inputs (for self-checking programs whose `main`
/// takes no parameters).
///
/// `field` must be the field the program was compiled under, which
/// `MonomorphizationOutput::field_id` records.
pub fn interpret(program: &Program, field: FieldId) -> Result<Value, InterpretError> {
    interpret_with_inputs(program, Vec::new(), field)
}

/// Interpret `program`'s entry point, binding `inputs` to `main`'s parameters in order.
///
/// `field` must match the compiled program. Every `Field` input must belong to that field, and
/// every integer input must be a value of the width and signedness it declares.
/// Use [`inputs_from_prover_toml`] to build `inputs` from a `Prover.toml` file and the ABI.
pub fn interpret_with_inputs(
    program: &Program,
    inputs: Vec<Value>,
    field: FieldId,
) -> Result<Value, InterpretError> {
    let mut interp = Interpreter::new(program, field);
    let main = main_function_of(program)?;
    if main.parameters.len() != inputs.len() {
        return Err(InterpretError::InvalidInput(format!(
            "entry point expects {} input(s), got {}",
            main.parameters.len(),
            inputs.len()
        )));
    }
    input::validate_inputs(&inputs, field)?;
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
    fn new(program: &'p Program, field: FieldId) -> Self {
        Interpreter {
            program,
            field: FieldConfig::new(field),
            globals: HashMap::new(),
            unconstrained: false,
            call_depth: 0,
        }
    }

    fn function(&self, id: FuncId) -> Result<&'p Function, InterpretError> {
        function_of(self.program, id)
    }

    fn call_function(&mut self, id: FuncId, args: Vec<Value>) -> Result<Value, InterpretError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(InterpretError::Unsupported(format!(
                "call depth exceeds {MAX_CALL_DEPTH}"
            )));
        }
        let func = self.function(id)?;
        let lined_up = func.parameters.len() == args.len()
            && (func.parameters.iter())
                .zip(&args)
                .all(|((_, _, _, typ, _), arg)| tuple_shape_matches(arg, typ));
        let args = if lined_up {
            args
        } else {
            regroup_arguments(func, args)?
        };
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
        self.call_depth += 1;
        let flow = self.eval(&func.body, &mut frame);
        self.call_depth -= 1;
        self.unconstrained = outer_unconstrained;
        match flow? {
            Flow::Normal(value) => Ok(value),
            Flow::Break | Flow::Continue => Err(InterpretError::Internal(
                "break/continue escaped a function body".to_string(),
            )),
        }
    }
}

/// Whether every tuple in `value` has the arity its place in `typ` names, all the way down.
fn tuple_shape_matches(value: &Value, typ: &Type) -> bool {
    match (value, typ) {
        (Value::Tuple(cells), Type::Tuple(types)) => {
            cells.len() == types.len()
                && cells
                    .iter()
                    .zip(types)
                    .all(|(cell, typ)| tuple_shape_matches(&cell.borrow(), typ))
        }
        (Value::Tuple(_), _) | (_, Type::Tuple(_)) => false,
        _ => true,
    }
}

/// The monomorphizer turns a tuple or struct pattern parameter into one parameter per leaf, while
/// a call still passes the whole value: SSA flattens tuples on both sides of a call. Flatten the
/// arguments' tuples the same way and regroup the leaves by the parameter types.
fn regroup_arguments(func: &Function, args: Vec<Value>) -> Result<Vec<Value>, InterpretError> {
    fn flatten(value: Value, leaves: &mut Vec<Value>) {
        match value {
            Value::Tuple(cells) => {
                for cell in cells {
                    flatten(cell.borrow().deep_copy(), leaves);
                }
            }
            leaf => leaves.push(leaf),
        }
    }
    fn regroup(typ: &Type, leaves: &mut impl Iterator<Item = Value>) -> Option<Value> {
        match typ {
            Type::Tuple(types) => types
                .iter()
                .map(|typ| regroup(typ, leaves))
                .collect::<Option<_>>()
                .map(Value::tuple),
            _ => leaves.next(),
        }
    }
    let leaf_count_differs = |which: &str| {
        InterpretError::Internal(format!(
            "the arguments of function {} have {which} leaves than its parameters",
            func.id
        ))
    };

    let mut leaves = Vec::new();
    for arg in args {
        flatten(arg, &mut leaves);
    }
    let mut leaves = leaves.into_iter();
    let regrouped = func
        .parameters
        .iter()
        .map(|(_, _, _, typ, _)| regroup(typ, &mut leaves))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| leaf_count_differs("fewer"))?;
    if leaves.next().is_some() {
        return Err(leaf_count_differs("more"));
    }
    Ok(regrouped)
}
