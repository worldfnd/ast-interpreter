use std::cell::RefCell;
use std::rc::Rc;

use acvm::{AcirField, FieldElement};
use num_bigint::{BigInt, Sign};
use num_traits::Zero;

use noirc_errors::Location;
use noirc_frontend::ast::{BinaryOpKind, UnaryOp};
use noirc_frontend::hir_def::expr::Constructor;
use noirc_frontend::hir_def::types::Type as HirType;
use noirc_frontend::monomorphization::ast::{
    Definition, Expression, GlobalId, LValue, Literal, MatchCase, Type,
};
use noirc_frontend::token::FmtStrFragment;
use noirc_printable_type::PrintableType;

use super::error::InterpretError;
use super::value::{IntValue, Value, field_to_bigint};
use super::{Flow, Frame, GlobalState, Interpreter};

/// The resolved target of a call: a user function, or a builtin/foreign intrinsic dispatched by name.
enum Callee<'p> {
    Function(noirc_frontend::monomorphization::ast::FuncId),
    Intrinsic(&'p str),
}

fn index_out_of_bounds(location: Location, index: usize, len: usize) -> InterpretError {
    InterpretError::AssertionFailed {
        location,
        message: Some(format!(
            "Index out of bounds, array has size {len}, but index was {index}"
        )),
    }
}

impl<'p> Interpreter<'p> {
    /// Evaluate an expression in value position, rejecting a stray `break`/`continue`.
    pub(super) fn eval_expr_value(
        &mut self,
        expr: &'p Expression,
        env: &mut Frame,
    ) -> Result<Value, InterpretError> {
        match self.eval(expr, env)? {
            Flow::Normal(value) => Ok(value),
            // e.g. `let x = loop { break v };` — not modeled, so tolerate rather than false-reject.
            Flow::Break | Flow::Continue => Err(InterpretError::Unsupported(
                "break/continue in value position".to_string(),
            )),
        }
    }

    pub(super) fn eval(
        &mut self,
        expr: &'p Expression,
        env: &mut Frame,
    ) -> Result<Flow, InterpretError> {
        let value = match expr {
            Expression::Ident(ident) => match &ident.definition {
                Definition::Local(local) => {
                    let value = env.get(local).cloned().ok_or_else(|| {
                        InterpretError::Internal(format!("unbound local '{}'", ident.name))
                    })?;
                    match value {
                        // Load a mutable slot; both arms `deep_copy` so a read never aliases.
                        Value::Ref(cell, true) => cell.borrow().deep_copy(),
                        other => other.deep_copy(),
                    }
                }
                Definition::Global(global) => self.eval_global(*global)?,
                Definition::Function(id) => Value::Function(*id),
                Definition::Builtin(name)
                | Definition::LowLevel(name)
                | Definition::Oracle { name, pure: _ } => {
                    return Err(InterpretError::Unsupported(format!(
                        "reference to intrinsic '{name}'"
                    )));
                }
            },

            Expression::Literal(literal) => self.eval_literal(literal, env)?,

            Expression::Block(expressions) => {
                let mut last = Value::Unit;
                for expression in expressions {
                    match self.eval(expression, env)? {
                        Flow::Normal(value) => last = value,
                        other => return Ok(other),
                    }
                }
                last
            }

            Expression::Unary(unary) => match &unary.operator {
                // `&`/`&mut`: reuse a slot's cell to alias it, else box the value in a fresh cell.
                // The `skip` flag (set for `&mut a.b.c` taken through a reference field, where
                // member access is elaborated as an offset that already denotes the reference)
                // doesn't change this: `eval_place` peels to the live cell either way, while
                // evaluating the rhs as a value would drop the reference.
                UnaryOp::Reference { .. } => match self.eval_place(&unary.rhs, env)? {
                    Value::Ref(cell, true) => Value::Ref(cell, false),
                    other => Value::Ref(Rc::new(RefCell::new(other)), false),
                },
                // A skipped deref (or other op) returns the operand unchanged.
                _ if unary.skip => return self.eval(&unary.rhs, env),
                UnaryOp::Dereference { .. } => self.eval_expr_value(&unary.rhs, env)?.deref()?,
                _ => {
                    let rhs = self.eval_expr_value(&unary.rhs, env)?;
                    self.eval_unary(&unary.operator, rhs)?
                }
            },

            Expression::Binary(binary) => {
                let lhs = self.eval_expr_value(&binary.lhs, env)?;
                let rhs = self.eval_expr_value(&binary.rhs, env)?;
                self.eval_binary(binary.operator, lhs, rhs)?
            }

            Expression::Index(index) => {
                // Evaluate the index before reading the collection: the index expression may
                // mutate the very array being indexed (e.g. `b[{ b[0] = ...; 0 }]`), and the load
                // must see those stores, matching Noir's SSA (array_get reads the current array).
                let i = self.eval_expr_value(&index.index, env)?.as_index()?;
                let collection = self.eval_expr_value(&index.collection, env)?;
                match collection {
                    Value::Array(elements) => elements
                        .get(i)
                        .cloned()
                        .ok_or_else(|| index_out_of_bounds(index.location, i, elements.len()))?,
                    other => {
                        return Err(InterpretError::Type(format!("cannot index {other:?}")));
                    }
                }
            }

            Expression::Cast(cast) => {
                let value = self.eval_expr_value(&cast.lhs, env)?;
                self.eval_cast(value, &cast.r#type)?
            }

            Expression::For(for_) => {
                let (signed, bits) = int_type(&for_.index_type).ok_or_else(|| {
                    InterpretError::Type("for-loop index is not an integer type".to_string())
                })?;
                let start = self.eval_expr_value(&for_.start_range, env)?;
                let end = self.eval_expr_value(&for_.end_range, env)?;
                let start = start.as_int()?.value.clone();
                let end = end.as_int()?.value.clone();
                let end = if for_.inclusive { end + 1 } else { end };
                let mut i = start;
                while i < end {
                    env.insert(
                        for_.index_variable,
                        Value::Int(IntValue::canonical(signed, bits, i.clone())),
                    );
                    match self.eval(&for_.block, env)? {
                        Flow::Break => break,
                        Flow::Continue | Flow::Normal(_) => {}
                    }
                    i += 1;
                }
                Value::Unit
            }

            Expression::Loop(body) => {
                loop {
                    match self.eval(body, env)? {
                        Flow::Break => break,
                        Flow::Continue | Flow::Normal(_) => {}
                    }
                }
                Value::Unit
            }

            Expression::While(while_) => {
                while self.eval_expr_value(&while_.condition, env)?.as_bool()? {
                    match self.eval(&while_.body, env)? {
                        Flow::Break => break,
                        Flow::Continue | Flow::Normal(_) => {}
                    }
                }
                Value::Unit
            }

            Expression::If(if_) => {
                if self.eval_expr_value(&if_.condition, env)?.as_bool()? {
                    return self.eval(&if_.consequence, env);
                }
                match &if_.alternative {
                    Some(alternative) => return self.eval(alternative, env),
                    None => Value::Unit,
                }
            }

            Expression::Tuple(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(self.eval_expr_value(element, env)?);
                }
                Value::tuple(values)
            }

            Expression::ExtractTupleField(tuple, field) => {
                self.eval_expr_value(tuple, env)?.tuple_field(*field)?
            }

            Expression::Call(call) => {
                let callee = self.resolve_callee(&call.func, env)?;
                let mut args = Vec::with_capacity(call.arguments.len());
                for argument in &call.arguments {
                    args.push(self.eval_expr_value(argument, env)?);
                }
                match callee {
                    Callee::Function(id) => self.call_function(id, args)?,
                    Callee::Intrinsic(name) => {
                        self.call_intrinsic(name, args, &call.return_type, call.location)?
                    }
                }
            }

            Expression::Let(let_) => {
                let value = self.eval_expr_value(&let_.expression, env)?;
                // A `let mut` slot is a shared cell: reads auto-deref, writes store through it.
                let bound = if let_.mutable {
                    Value::Ref(Rc::new(RefCell::new(value)), true)
                } else {
                    value
                };
                env.insert(let_.id, bound);
                Value::Unit
            }

            Expression::Constrain(condition, location, message) => {
                if !self.eval_expr_value(condition, env)?.as_bool()? {
                    let message = match message {
                        Some(boxed) => self.render_assert_message(&boxed.0, &boxed.1, env),
                        None => None,
                    };
                    return Err(InterpretError::AssertionFailed {
                        location: *location,
                        message,
                    });
                }
                Value::Unit
            }

            Expression::Assign(assign) => {
                let value = self.eval_expr_value(&assign.expression, env)?;
                self.store(&assign.lvalue, value, env)?;
                Value::Unit
            }

            Expression::Semi(inner) => match self.eval(inner, env)? {
                Flow::Normal(_) => Value::Unit,
                other => return Ok(other),
            },

            Expression::Clone(inner) => self.eval_expr_value(inner, env)?,

            Expression::Drop(inner) => match self.eval(inner, env)? {
                Flow::Normal(_) => Value::Unit,
                other => return Ok(other),
            },

            Expression::Break => return Ok(Flow::Break),
            Expression::Continue => return Ok(Flow::Continue),

            Expression::Match(m) => {
                let (local, _name) = &m.variable_to_match;
                // The elaborator always binds the scrutinee to a preceding local. An enum is a
                // cell-backed tuple `(tag, payloads…)`.
                let scrutinee = match env.get(local).cloned().ok_or_else(|| {
                    InterpretError::Internal("unbound match scrutinee".to_string())
                })? {
                    Value::Ref(cell, true) => cell.borrow().clone(),
                    other => other,
                };

                // With a default, every case is tag-tested; without one, the last case is the
                // unconditional fallback.
                let tested = if m.default_case.is_some() {
                    m.cases.len()
                } else {
                    m.cases.len().checked_sub(1).ok_or_else(|| {
                        InterpretError::Internal("match with no cases and no default".to_string())
                    })?
                };

                for case in &m.cases[..tested] {
                    if case_matches(&case.constructor, &scrutinee)? {
                        bind_case_arguments(case, &scrutinee, env)?;
                        return self.eval(&case.branch, env);
                    }
                }
                match &m.default_case {
                    Some(default) => return self.eval(default, env),
                    None => {
                        let case = m
                            .cases
                            .last()
                            .expect("no default implies a last fallback case");
                        bind_case_arguments(case, &scrutinee, env)?;
                        return self.eval(&case.branch, env);
                    }
                }
            }
        };
        Ok(Flow::Normal(value))
    }

    /// Evaluate a global on first reference and cache it. Untouched globals may use constructs this
    /// interpreter does not yet support, so they stay lazy.
    fn eval_global(&mut self, id: GlobalId) -> Result<Value, InterpretError> {
        match self.globals.get(&id) {
            Some(GlobalState::Done(value)) => return Ok(value.deep_copy()),
            Some(GlobalState::InProgress) => {
                return Err(InterpretError::Internal(format!(
                    "global {id:?} is defined in terms of itself"
                )));
            }
            None => {}
        }
        let program = self.program;
        let (_, _, expr) = program
            .globals
            .get(&id)
            .ok_or_else(|| InterpretError::Internal(format!("unknown global {id:?}")))?;
        self.globals.insert(id, GlobalState::InProgress);
        let mut frame = Frame::new();
        let value = match self.eval_expr_value(expr, &mut frame) {
            Ok(value) => value,
            Err(e) => {
                self.globals.remove(&id);
                return Err(e);
            }
        };
        self.globals
            .insert(id, GlobalState::Done(value.deep_copy()));
        Ok(value)
    }

    fn eval_literal(
        &mut self,
        literal: &'p Literal,
        env: &mut Frame,
    ) -> Result<Value, InterpretError> {
        let value = match literal {
            // beta.22: the integer literal carries a signed `BigInt` (previously a `SignedField`).
            Literal::Integer(value, typ, _) => match typ {
                // A Field literal's `BigInt` is the signed representative; `bigint_to_field` reduces
                // it into the compiled-in field (a negative value maps to `modulus - |value|`),
                // matching Noir's removed `SignedField::to_field_element`.
                Type::Field => Value::Field(bigint_to_field(value)),
                Type::Integer(signedness, bits) => {
                    let signed = signedness.is_signed();
                    // `canonical` wraps the mathematical value into the type's range (identity for a
                    // well-formed literal), replacing the old `SignedField` → i128/u128 conversion.
                    Value::Int(IntValue::canonical(signed, bits.bit_size(), value.clone()))
                }
                other => {
                    return Err(InterpretError::Type(format!(
                        "integer literal with non-numeric type {other:?}"
                    )));
                }
            },
            Literal::Bool(b) => Value::Bool(*b),
            Literal::Unit => Value::Unit,
            // Noir strings may be non-UTF-8; ours is a Rust `String`, so tolerate that case.
            Literal::Str(s) => Value::Str(String::from_utf8(s.clone()).map_err(|e| {
                InterpretError::Unsupported(format!("non-UTF-8 string literal: {e}"))
            })?),
            Literal::Array(array) | Literal::Vector(array) => {
                let mut elements = Vec::with_capacity(array.contents.len());
                for element in &array.contents {
                    elements.push(self.eval_expr_value(element, env)?);
                }
                Value::Array(elements)
            }
            Literal::Repeated {
                element, length, ..
            } => {
                let value = self.eval_expr_value(element, env)?;
                Value::Array(
                    std::iter::repeat_with(|| value.deep_copy())
                        .take(*length as usize)
                        .collect(),
                )
            }
            Literal::FmtStr(fragments, _count, captures) => {
                let values = self.eval_fmt_captures(captures, env)?;
                if values.iter().any(contains_field) {
                    return Err(InterpretError::Unsupported(
                        "format string interpolating a field".to_string(),
                    ));
                }
                let types = capture_types(captures)?;
                Value::Str(format_fmt_str(fragments, &values, &types)?)
            }
        };
        Ok(value)
    }

    fn resolve_callee(
        &mut self,
        func: &'p Expression,
        env: &mut Frame,
    ) -> Result<Callee<'p>, InterpretError> {
        if let Expression::Ident(ident) = func {
            match &ident.definition {
                Definition::Function(id) => return Ok(Callee::Function(*id)),
                // `#[builtin]`/`#[foreign]` both carry the intrinsic name; dispatch on it.
                Definition::Builtin(name) | Definition::LowLevel(name) => {
                    return Ok(Callee::Intrinsic(name.as_str()));
                }
                Definition::Oracle { name, pure: _ } => {
                    return Err(InterpretError::Unsupported(format!("oracle call '{name}'")));
                }
                Definition::Local(_) | Definition::Global(_) => {}
            }
        }
        match self.eval_expr_value(func, env)? {
            Value::Function(id) => Ok(Callee::Function(id)),
            other => Err(InterpretError::Type(format!(
                "call of non-function {other:?}"
            ))),
        }
    }

    fn eval_unary(&self, op: &UnaryOp, rhs: Value) -> Result<Value, InterpretError> {
        match op {
            UnaryOp::Minus => match rhs {
                Value::Int(int) => Ok(Value::Int(IntValue::checked(
                    int.signed, int.bits, -int.value, "negation",
                )?)),
                Value::Field(field) => Ok(Value::Field(FieldElement::zero() - field)),
                other => Err(InterpretError::Type(format!("cannot negate {other:?}"))),
            },
            UnaryOp::Not => match rhs {
                Value::Bool(b) => Ok(Value::Bool(!b)),
                Value::Int(int) => {
                    // Bitwise NOT: complement the two's-complement bit pattern.
                    let mask = (BigInt::from(1) << int.bits as usize) - 1;
                    let complemented = mask - int.unsigned_repr();
                    Ok(Value::Int(IntValue::canonical(
                        int.signed,
                        int.bits,
                        complemented,
                    )))
                }
                other => Err(InterpretError::Type(format!(
                    "cannot apply `!` to {other:?}"
                ))),
            },
            // Reference/Dereference need the operand expression, so `eval` handles them directly.
            UnaryOp::Reference { .. } | UnaryOp::Dereference { .. } => Err(
                InterpretError::Internal("reference/dereference reached eval_unary".to_string()),
            ),
        }
    }

    fn eval_binary(
        &self,
        op: BinaryOpKind,
        lhs: Value,
        rhs: Value,
    ) -> Result<Value, InterpretError> {
        use BinaryOpKind::*;
        match (lhs, rhs) {
            (Value::Int(a), Value::Int(b)) => eval_int_binary(op, a, b),
            (Value::Field(a), Value::Field(b)) => match op {
                Add => Ok(Value::Field(a + b)),
                Subtract => Ok(Value::Field(a - b)),
                Multiply => Ok(Value::Field(a * b)),
                Divide => {
                    if b == FieldElement::zero() {
                        return Err(InterpretError::DivisionByZero);
                    }
                    Ok(Value::Field(a * b.inverse()))
                }
                Equal => Ok(Value::Bool(a == b)),
                NotEqual => Ok(Value::Bool(a != b)),
                _ => Err(InterpretError::Type(
                    "ordering/bitwise operators are not defined on Field".to_string(),
                )),
            },
            (Value::Bool(a), Value::Bool(b)) => match op {
                And => Ok(Value::Bool(a && b)),
                Or => Ok(Value::Bool(a || b)),
                Xor => Ok(Value::Bool(a ^ b)),
                Equal => Ok(Value::Bool(a == b)),
                NotEqual => Ok(Value::Bool(a != b)),
                // Noir orders bools as false < true (same as Rust's `bool: Ord`).
                Less => Ok(Value::Bool(a < b)),
                LessEqual => Ok(Value::Bool(a <= b)),
                Greater => Ok(Value::Bool(a > b)),
                GreaterEqual => Ok(Value::Bool(a >= b)),
                _ => Err(InterpretError::Type(format!(
                    "operator {op:?} not defined on bool"
                ))),
            },
            // Aggregate `==`/`Ord` lower to an `Eq::eq` call, never a primitive Binary, so a
            // matching-shape pair here is a coverage gap — tolerate it. Ill-typed pairs stay loud.
            (lhs, rhs) if same_aggregate_shape(&lhs, &rhs) => Err(InterpretError::Unsupported(
                format!("binary {op:?} on aggregate operands (should lower to a trait-impl call)"),
            )),
            (lhs, rhs) => Err(InterpretError::Type(format!(
                "binary operator {op:?} on mismatched operands {lhs:?}, {rhs:?}"
            ))),
        }
    }

    fn eval_cast(&self, value: Value, target: &Type) -> Result<Value, InterpretError> {
        match target {
            Type::Field => match value {
                // Noir rejects signed-to-Field casts at type-check (`UnsupportedFieldCast`);
                // inventing a semantics here would silently bless an ill-typed AST.
                Value::Int(int) if int.signed => Err(InterpretError::Type(
                    "cast of a signed integer to Field (rejected by Noir's type checker)"
                        .to_string(),
                )),
                Value::Int(int) => Ok(Value::Field(int.to_field())),
                Value::Bool(b) => Ok(Value::Field(if b {
                    FieldElement::one()
                } else {
                    FieldElement::zero()
                })),
                Value::Field(f) => Ok(Value::Field(f)),
                other => Err(InterpretError::Type(format!(
                    "cannot cast {other:?} to Field"
                ))),
            },
            Type::Integer(signedness, bits) => {
                let signed = signedness.is_signed();
                let width = bits.bit_size();
                let raw = match value {
                    Value::Int(int) => int.value,
                    Value::Bool(b) => BigInt::from(b as u8),
                    // Noir casts Field -> integer by truncating mod 2^bits (see ssa_gen
                    // `insert_safe_cast`). `canonical` does the truncation; `field_to_bigint`
                    // avoids `to_u128`'s panic for field values >= 2^128.
                    Value::Field(f) => field_to_bigint(&f),
                    other => {
                        return Err(InterpretError::Type(format!(
                            "cannot cast {other:?} to an integer"
                        )));
                    }
                };
                Ok(Value::Int(IntValue::canonical(signed, width, raw)))
            }
            Type::Bool => match value {
                Value::Bool(b) => Ok(Value::Bool(b)),
                // Noir rejects numeric-to-bool casts at type-check (`CannotCastNumericToBool`);
                // a `!= 0` semantics here would silently bless an ill-typed AST.
                other => Err(InterpretError::Type(format!(
                    "cannot cast {other:?} to bool (rejected by Noir's type checker)"
                ))),
            },
            other => Err(InterpretError::Unsupported(format!("cast to {other:?}"))),
        }
    }

    /// Render an assertion's failure message. A format string is rendered even when it interpolates
    /// a `Field` — the message is triage text, never compared.
    fn render_assert_message(
        &mut self,
        expr: &'p Expression,
        typ: &HirType,
        env: &mut Frame,
    ) -> Option<String> {
        if let Expression::Literal(Literal::FmtStr(fragments, _count, captures)) = expr {
            let values = self.eval_fmt_captures(captures, env).ok()?;
            let PrintableType::FmtString { typ, .. } = PrintableType::from(typ) else {
                return None;
            };
            let types = printable_capture_types(*typ)?;
            return format_fmt_str(fragments, &values, &types).ok();
        }
        match self.eval_expr_value(expr, env) {
            Ok(Value::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// Evaluate a format string's captures (the monomorphizer packs them into one tuple).
    fn eval_fmt_captures(
        &mut self,
        captures: &'p Expression,
        env: &mut Frame,
    ) -> Result<Vec<Value>, InterpretError> {
        match self.eval_expr_value(captures, env)? {
            Value::Tuple(cells) => Ok(cells.iter().map(|c| c.borrow().deep_copy()).collect()),
            other => Err(InterpretError::Internal(format!(
                "fmt captures were not a tuple: {other:?}"
            ))),
        }
    }

    /// Evaluate `expr` in place position: return a `Ref(cell, true)` when it denotes a shareable
    /// cell (a mutable slot or a tuple field), so `&`/`&mut` can alias it.
    fn eval_place(
        &mut self,
        expr: &'p Expression,
        env: &mut Frame,
    ) -> Result<Value, InterpretError> {
        match expr {
            Expression::Ident(ident) => match &ident.definition {
                Definition::Local(local) => env.get(local).cloned().ok_or_else(|| {
                    InterpretError::Internal(format!("unbound local '{}'", ident.name))
                }),
                // A global/function/intrinsic has no place; the caller boxes the value in a cell.
                _ => self.eval_expr_value(expr, env),
            },
            Expression::ExtractTupleField(inner, i) => {
                // Peel through any depth of references to the live tuple cells (the same helper the
                // write side uses), so `&mut pp.field` through a `&mut &mut S` aliases the real
                // cell rather than a one-level snapshot.
                let cells = tuple_cells_of(self.eval_place(inner, env)?)?;
                let cell = cells.get(*i).cloned().ok_or_else(|| {
                    InterpretError::Type(format!("tuple field {i} out of bounds"))
                })?;
                Ok(Value::Ref(cell, true))
            }
            Expression::Unary(unary) if matches!(unary.operator, UnaryOp::Dereference { .. }) => {
                if unary.skip {
                    return self.eval_place(&unary.rhs, env);
                }
                match self.eval_expr_value(&unary.rhs, env)? {
                    Value::Ref(cell, _) => Ok(Value::Ref(cell, true)),
                    other => Err(InterpretError::Unsupported(format!(
                        "dereference of a non-reference value ({other:?})"
                    ))),
                }
            }
            // `&(non-place expr)`, including immutable `&arr[i]` (Noir snapshots the element; the
            // mutable form is compiler-rejected): the `Reference` arm boxes the value in a fresh cell.
            _ => self.eval_expr_value(expr, env),
        }
    }

    /// Assign `rhs` to `lvalue` by resolving it to a target cell and storing through it, so live
    /// field references see the write. Array elements aren't cells, so those update in place.
    fn store(
        &mut self,
        lvalue: &'p LValue,
        rhs: Value,
        env: &mut Frame,
    ) -> Result<(), InterpretError> {
        match lvalue {
            LValue::Clone(inner) => self.store(inner, rhs, env),
            LValue::Index { .. } => {
                let (array_cell, indices) = self.resolve_array_target(lvalue, env)?;
                let mut array = array_cell.borrow_mut();
                let mut slot: &mut Value = &mut array;
                for (index, location) in indices {
                    let Value::Array(elements) = slot else {
                        return Err(InterpretError::Type(
                            "indexed assignment to a non-array".to_string(),
                        ));
                    };
                    let len = elements.len();
                    slot = elements
                        .get_mut(index)
                        .ok_or_else(|| index_out_of_bounds(location, index, len))?;
                }
                *slot = rhs;
                Ok(())
            }
            _ => {
                let cell = self.lvalue_target_cell(lvalue, env)?;
                store_flattened(&cell, rhs);
                Ok(())
            }
        }
    }

    /// The cell to store an assignment through, for every lvalue whose leaf is a cell (an
    /// `Ident`/`MemberAccess`/`Dereference`). Array-element leaves are handled in [`Self::store`].
    fn lvalue_target_cell(
        &mut self,
        lvalue: &'p LValue,
        env: &mut Frame,
    ) -> Result<Rc<RefCell<Value>>, InterpretError> {
        match lvalue {
            LValue::Ident(ident) => match &ident.definition {
                Definition::Local(local) => {
                    let value = env.get(local).cloned().ok_or_else(|| {
                        InterpretError::Internal("assignment to unbound local".to_string())
                    })?;
                    match value {
                        Value::Ref(cell, _) => Ok(cell),
                        // A real constrained-`main` assignment target is always a mutable slot.
                        _ => Err(InterpretError::Internal(
                            "assignment to a non-mutable local".to_string(),
                        )),
                    }
                }
                _ => Err(InterpretError::Unsupported(
                    "assignment to a non-local binding".to_string(),
                )),
            },
            LValue::MemberAccess {
                object,
                field_index,
            } => {
                let cells = tuple_cells_of(self.lvalue_value(object, env)?)?;
                cells.get(*field_index).cloned().ok_or_else(|| {
                    InterpretError::Type(format!("assignment field {field_index} out of bounds"))
                })
            }
            LValue::Dereference { reference, .. } => match self.lvalue_value(reference, env)? {
                Value::Ref(cell, _) => Ok(cell),
                // The reference model didn't produce a cell for this shape — tolerate it.
                other => Err(InterpretError::Unsupported(format!(
                    "assignment through a dereference of a non-reference ({other:?})"
                ))),
            },
            LValue::Clone(inner) => self.lvalue_target_cell(inner, env),
            // Reached only for an unusual array-element-as-object shape not yet modeled; tolerate it.
            LValue::Index { .. } => Err(InterpretError::Unsupported(
                "assignment through an array-element place".to_string(),
            )),
        }
    }

    /// The current value an lvalue denotes (auto-dereferencing a mutable slot). Returned tuples and
    /// arrays share their cells with the binding, so the caller can reach a live field/element cell.
    fn lvalue_value(
        &mut self,
        lvalue: &'p LValue,
        env: &mut Frame,
    ) -> Result<Value, InterpretError> {
        match lvalue {
            LValue::Ident(ident) => match &ident.definition {
                Definition::Local(local) => {
                    let value = env.get(local).cloned().ok_or_else(|| {
                        InterpretError::Internal(format!(
                            "unbound local '{}' in lvalue",
                            ident.name
                        ))
                    })?;
                    match value {
                        Value::Ref(cell, true) => Ok(cell.borrow().clone()),
                        other => Ok(other),
                    }
                }
                _ => Err(InterpretError::Unsupported(
                    "assignment to a non-local binding".to_string(),
                )),
            },
            LValue::MemberAccess {
                object,
                field_index,
            } => {
                let cells = tuple_cells_of(self.lvalue_value(object, env)?)?;
                cells
                    .get(*field_index)
                    .map(|c| c.borrow().clone())
                    .ok_or_else(|| {
                        InterpretError::Type(format!("field {field_index} out of bounds in lvalue"))
                    })
            }
            LValue::Index {
                array,
                index,
                location,
                ..
            } => {
                let array = self.lvalue_value(array, env)?;
                let i = self.eval_expr_value(index, env)?.as_index()?;
                match array {
                    Value::Array(elements) => elements
                        .get(i)
                        .cloned()
                        .ok_or_else(|| index_out_of_bounds(*location, i, elements.len())),
                    other => Err(InterpretError::Type(format!(
                        "cannot index {other:?} in lvalue"
                    ))),
                }
            }
            // Share the referent's cells (shallow) so a nested field target stays live.
            LValue::Dereference { reference, .. } => match self.lvalue_value(reference, env)? {
                Value::Ref(cell, _) => Ok(cell.borrow().clone()),
                other => Err(InterpretError::Unsupported(format!(
                    "dereference of a non-reference value in lvalue ({other:?})"
                ))),
            },
            LValue::Clone(inner) => self.lvalue_value(inner, env),
        }
    }

    /// Resolve an `Index` lvalue chain to the cell holding the outermost array plus the indices into
    /// it, so a nested `arr[i][j]` update can descend the array in place.
    fn resolve_array_target(
        &mut self,
        lvalue: &'p LValue,
        env: &mut Frame,
    ) -> Result<(Rc<RefCell<Value>>, Vec<(usize, Location)>), InterpretError> {
        match lvalue {
            LValue::Index {
                array,
                index,
                location,
                ..
            } => {
                // The ownership pass wraps the inner array in `Clone`; peel it before recursing.
                let array = strip_clone(array);
                // Noir resolves the array place before evaluating the index expression.
                let (cell, mut indices) = if matches!(array, LValue::Index { .. }) {
                    self.resolve_array_target(array, env)?
                } else {
                    (self.lvalue_target_cell(array, env)?, Vec::new())
                };
                indices.push((self.eval_expr_value(index, env)?.as_index()?, *location));
                Ok((cell, indices))
            }
            LValue::Clone(inner) => self.resolve_array_target(inner, env),
            _ => Err(InterpretError::Internal(
                "resolve_array_target on a non-index lvalue".to_string(),
            )),
        }
    }
}

/// Peel any `Clone` wrappers off an lvalue, returning the underlying place.
fn strip_clone(lvalue: &LValue) -> &LValue {
    let mut current = lvalue;
    while let LValue::Clone(inner) = current {
        current = inner;
    }
    current
}

/// A tuple's shared cells, peeling through any depth of references. The shared peel primitive for
/// both the write side (`MemberAccess` assign) and the ref-take side (`eval_place`), so field
/// access reaches the live cell at any nesting. Any other shape is a reference pattern we don't
/// model, so tolerate it.
fn tuple_cells_of(value: Value) -> Result<Vec<Rc<RefCell<Value>>>, InterpretError> {
    match value {
        Value::Tuple(cells) => Ok(cells),
        Value::Ref(cell, _) => tuple_cells_of(cell.borrow().clone()),
        other => Err(InterpretError::Unsupported(format!(
            "field access on a non-tuple value ({other:?})"
        ))),
    }
}

/// Write `rhs` into `target`. When both are tuples, write through the target's existing field cells
/// rather than replacing them, so live `&mut field` references keep pointing at the new value.
fn store_flattened(target: &Rc<RefCell<Value>>, rhs: Value) {
    let recursed = {
        let borrowed = target.borrow();
        match (&*borrowed, &rhs) {
            (Value::Tuple(target_cells), Value::Tuple(rhs_cells))
                if target_cells.len() == rhs_cells.len() =>
            {
                for (t, r) in target_cells.iter().zip(rhs_cells) {
                    store_flattened(t, r.borrow().clone());
                }
                true
            }
            _ => false,
        }
    };
    if !recursed {
        *target.borrow_mut() = rhs;
    }
}

/// Whether two values are aggregates of the same kind (the shapes that lower to an `Eq`/`Ord` call).
fn same_aggregate_shape(a: &Value, b: &Value) -> bool {
    matches!(
        (a, b),
        (Value::Array(_), Value::Array(_))
            | (Value::Tuple(_), Value::Tuple(_))
            | (Value::Str(_), Value::Str(_))
            | (Value::Unit, Value::Unit)
    )
}

fn capture_types(captures: &Expression) -> Result<Vec<PrintableType>, InterpretError> {
    let Type::Tuple(types) =
        captures.return_type().as_deref().cloned().ok_or_else(|| {
            InterpretError::Internal("fmt captures have no return type".to_string())
        })?
    else {
        return Err(InterpretError::Internal(
            "fmt captures do not have a tuple type".to_string(),
        ));
    };
    if types.iter().any(type_contains_erased_aggregate) {
        return Err(InterpretError::Unsupported(
            "format string interpolating an aggregate with erased type metadata".to_string(),
        ));
    }
    Ok(types.iter().map(printable_type).collect())
}

fn type_contains_erased_aggregate(typ: &Type) -> bool {
    match typ {
        Type::Tuple(_) => true,
        Type::Array(_, element) | Type::Vector(element) | Type::Reference(element, _) => {
            type_contains_erased_aggregate(element)
        }
        Type::Function(arguments, return_type, environment, _) => {
            arguments.iter().any(type_contains_erased_aggregate)
                || type_contains_erased_aggregate(return_type)
                || type_contains_erased_aggregate(environment)
        }
        Type::Field
        | Type::Integer(..)
        | Type::Bool
        | Type::String(_)
        | Type::FmtString(..)
        | Type::Unit => false,
    }
}

fn printable_capture_types(typ: PrintableType) -> Option<Vec<PrintableType>> {
    match typ {
        PrintableType::Tuple { types } => Some(types),
        PrintableType::Unit => Some(Vec::new()),
        _ => None,
    }
}

fn printable_type(typ: &Type) -> PrintableType {
    match typ {
        Type::Field => PrintableType::Field,
        Type::Array(length, element) => PrintableType::Array {
            length: *length,
            typ: Box::new(printable_type(element)),
        },
        Type::Integer(signedness, bits) if signedness.is_signed() => PrintableType::SignedInteger {
            width: bits.bit_size().into(),
        },
        Type::Integer(_, bits) => PrintableType::UnsignedInteger {
            width: bits.bit_size().into(),
        },
        Type::Bool => PrintableType::Boolean,
        Type::String(length) => PrintableType::String { length: *length },
        Type::FmtString(length, captures) => PrintableType::FmtString {
            length: *length,
            typ: Box::new(printable_type(captures)),
        },
        Type::Unit => PrintableType::Unit,
        Type::Tuple(types) => PrintableType::Tuple {
            types: types.iter().map(printable_type).collect(),
        },
        Type::Vector(element) => PrintableType::Vector {
            typ: Box::new(printable_type(element)),
        },
        Type::Reference(element, mutable) => PrintableType::Reference {
            typ: Box::new(printable_type(element)),
            mutable: *mutable,
        },
        Type::Function(arguments, return_type, environment, unconstrained) => {
            PrintableType::Function {
                arguments: arguments.iter().map(printable_type).collect(),
                return_type: Box::new(printable_type(return_type)),
                env: Box::new(printable_type(environment)),
                unconstrained: *unconstrained,
            }
        }
    }
}

fn format_fmt_str(
    fragments: &[FmtStrFragment],
    values: &[Value],
    types: &[PrintableType],
) -> Result<String, InterpretError> {
    if values.len() != types.len() {
        return Err(InterpretError::Internal(format!(
            "fmt-string has {} captures but {} capture types",
            values.len(),
            types.len()
        )));
    }
    let mut out = String::new();
    let mut next = values.iter().zip(types);
    for fragment in fragments {
        match fragment {
            FmtStrFragment::String(s) => out.push_str(s),
            FmtStrFragment::Interpolation(..) => {
                let (value, typ) = next.next().ok_or_else(|| {
                    InterpretError::Internal(
                        "fmt-string has more interpolations than captures".to_string(),
                    )
                })?;
                out.push_str(&format_value(value, typ)?);
            }
        }
    }
    if next.next().is_some() {
        return Err(InterpretError::Internal(
            "fmt-string has fewer interpolations than captures".to_string(),
        ));
    }
    Ok(out)
}

fn format_value(value: &Value, typ: &PrintableType) -> Result<String, InterpretError> {
    let mismatch = || {
        InterpretError::Type(format!(
            "cannot format value {value:?} as printable type {typ:?}"
        ))
    };
    match (value, typ) {
        (Value::Field(field), PrintableType::Field) => Ok(field.to_short_hex()),
        (Value::Int(int), PrintableType::SignedInteger { .. })
        | (Value::Int(int), PrintableType::UnsignedInteger { .. }) => Ok(int.value.to_string()),
        (Value::Bool(value), PrintableType::Boolean) => Ok(value.to_string()),
        (Value::Str(value), PrintableType::String { .. } | PrintableType::FmtString { .. }) => {
            Ok(value.clone())
        }
        (Value::Unit, PrintableType::Unit) => Ok("()".to_string()),
        (Value::Array(values), PrintableType::Array { typ, .. }) => {
            format_sequence(values, typ, "[", "]")
        }
        (Value::Array(values), PrintableType::Vector { typ }) => {
            format_sequence(values, typ, "@[", "]")
        }
        (Value::Tuple(cells), PrintableType::Tuple { types }) if cells.len() == types.len() => {
            let mut values = cells
                .iter()
                .zip(types)
                .map(|(cell, typ)| format_value(&cell.borrow(), typ))
                .collect::<Result<Vec<_>, _>>()?;
            if values.len() == 1 {
                values[0].push(',');
            }
            Ok(format!("({})", values.join(", ")))
        }
        (Value::Tuple(cells), PrintableType::Struct { name, fields })
            if cells.len() == fields.len() =>
        {
            let fields = cells
                .iter()
                .zip(fields)
                .map(|(cell, (field_name, typ))| {
                    Ok(format!(
                        "{field_name}: {}",
                        format_value(&cell.borrow(), typ)?
                    ))
                })
                .collect::<Result<Vec<_>, InterpretError>>()?;
            Ok(format!("{name} {{ {} }}", fields.join(", ")))
        }
        (Value::Tuple(cells), PrintableType::Enum { name, variants }) => {
            let tag_cell = cells.first().ok_or_else(mismatch)?.borrow();
            // The tag is always a `Field` (`case_matches` rejects anything else); `field_to_bigint`
            // avoids `to_u128`'s panic on an oversized tag.
            let tag = match &*tag_cell {
                Value::Field(tag) => {
                    usize::try_from(field_to_bigint(tag)).map_err(|_| mismatch())?
                }
                _ => return Err(mismatch()),
            };
            drop(tag_cell);
            let (variant_name, types) = variants.get(tag).ok_or_else(mismatch)?;
            let Value::Tuple(values) = &*cells.get(tag + 1).ok_or_else(mismatch)?.borrow() else {
                return Err(mismatch());
            };
            if values.len() != types.len() {
                return Err(mismatch());
            }
            let values = values
                .iter()
                .zip(types)
                .map(|(cell, typ)| format_value(&cell.borrow(), typ))
                .collect::<Result<Vec<_>, _>>()?;
            if values.is_empty() {
                Ok(format!("{name}::{variant_name}"))
            } else {
                Ok(format!("{name}::{variant_name}({})", values.join(", ")))
            }
        }
        (Value::Function(_), PrintableType::Function { .. }) => Ok(format!("<<{typ}>>")),
        (Value::Ref(_, _), PrintableType::Reference { mutable: true, .. }) => {
            Ok("<<mutable ref>>".to_string())
        }
        (Value::Ref(_, _), PrintableType::Reference { mutable: false, .. }) => {
            Ok("<<ref>>".to_string())
        }
        (Value::Ref(cell, _), _) => format_value(&cell.borrow(), typ),
        _ => Err(mismatch()),
    }
}

fn format_sequence(
    values: &[Value],
    typ: &PrintableType,
    open: &str,
    close: &str,
) -> Result<String, InterpretError> {
    let values = values
        .iter()
        .map(|value| format_value(value, typ))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{open}{}{close}", values.join(", ")))
}

fn contains_field(value: &Value) -> bool {
    match value {
        Value::Field(_) => true,
        Value::Array(values) => values.iter().any(contains_field),
        Value::Tuple(cells) => cells.iter().any(|cell| contains_field(&cell.borrow())),
        Value::Ref(cell, _) => contains_field(&cell.borrow()),
        Value::Int(_) | Value::Bool(_) | Value::Unit | Value::Str(_) | Value::Function(_) => false,
    }
}

/// Whether `scrutinee` satisfies `constructor`. The tag is the value itself for int/bool/field
/// leaves, and the `Field` at tuple index 0 for an enum.
fn case_matches(constructor: &Constructor, scrutinee: &Value) -> Result<bool, InterpretError> {
    match constructor {
        // Sole case (a bare tuple/unit/struct match): always taken.
        Constructor::Unit | Constructor::Tuple(_) => Ok(true),
        Constructor::Variant(..) if !constructor.is_enum() => Ok(true),
        Constructor::True => scrutinee.as_bool(),
        Constructor::False => Ok(!scrutinee.as_bool()?),
        // Matching on a `Field` is legal (e.g. `match x { 1 => .. }`), so compare its value.
        Constructor::Int(want) => match scrutinee {
            Value::Field(f) => Ok(f == &bigint_to_field(want)),
            _ => Ok(&scrutinee.as_int()?.value == want),
        },
        Constructor::Variant(_, index) => {
            let Value::Tuple(fields) = scrutinee else {
                return Err(InterpretError::Type(format!(
                    "match on non-enum value {scrutinee:?}"
                )));
            };
            let Some(cell) = fields.first() else {
                return Err(InterpretError::Type(
                    "enum value missing its Field tag".to_string(),
                ));
            };
            let tag = match &*cell.borrow() {
                Value::Field(tag) => field_to_bigint(tag),
                _ => {
                    return Err(InterpretError::Type("enum tag is not a Field".to_string()));
                }
            };
            Ok(tag == BigInt::from(*index as u64))
        }
        // Noir SSA lowers a range case as `tag == 0`, not containment; don't guess a semantics.
        Constructor::Range(..) => Err(InterpretError::Unsupported(
            "match range pattern".to_string(),
        )),
    }
}

/// Bind a matched case's argument locals from the scrutinee's payload, `deep_copy`ing out of the
/// cells so a bound variable never aliases the scrutinee.
fn bind_case_arguments(
    case: &MatchCase,
    scrutinee: &Value,
    env: &mut Frame,
) -> Result<(), InterpretError> {
    if case.arguments.is_empty() {
        return Ok(());
    }
    let payload_cells: Vec<Rc<RefCell<Value>>> = if case.constructor.is_enum() {
        let Value::Tuple(variants) = scrutinee else {
            return Err(InterpretError::Type(
                "enum scrutinee is not a tuple".to_string(),
            ));
        };
        // +1 skips the tag; each variant's payload is itself a tuple.
        match variants
            .get(case.constructor.variant_index() + 1)
            .map(|c| c.borrow().clone())
        {
            Some(Value::Tuple(fields)) => fields,
            _ => {
                return Err(InterpretError::Type(
                    "enum variant payload missing".to_string(),
                ));
            }
        }
    } else {
        let Value::Tuple(fields) = scrutinee else {
            return Err(InterpretError::Type(
                "tuple/struct scrutinee is not a tuple".to_string(),
            ));
        };
        fields.clone()
    };
    if payload_cells.len() != case.arguments.len() {
        return Err(InterpretError::Internal(format!(
            "match arity mismatch: {} payload vs {} bindings",
            payload_cells.len(),
            case.arguments.len()
        )));
    }
    for ((arg, _name), cell) in case.arguments.iter().zip(&payload_cells) {
        env.insert(*arg, cell.borrow().deep_copy());
    }
    Ok(())
}

// `pub(crate)` so the value-semantics fuzzer (`value_proptest.rs`) can drive the integer op
// dispatcher directly; visibility only.
pub(crate) fn eval_int_binary(
    op: BinaryOpKind,
    a: IntValue,
    b: IntValue,
) -> Result<Value, InterpretError> {
    use BinaryOpKind::*;
    // Both operands of an integer binary op share one type in a well-typed AST (this Noir's
    // `Shl`/`Shr` traits are `fn shl(self, other: Self)`, so shifts included). Computing a
    // mismatch with the lhs's type would be a silent wrong value, so reject it — defence in
    // depth against an ill-typed AST reaching the oracle.
    if a.signed != b.signed || a.bits != b.bits {
        return Err(InterpretError::Type(format!(
            "binary {op:?} on mismatched integer types: (signed={},{}b) vs (signed={},{}b)",
            a.signed, a.bits, b.signed, b.bits
        )));
    }
    // Comparators read the canonical (sign-aware) values directly.
    match op {
        Equal => return Ok(Value::Bool(a.value == b.value)),
        NotEqual => return Ok(Value::Bool(a.value != b.value)),
        Less => return Ok(Value::Bool(a.value < b.value)),
        LessEqual => return Ok(Value::Bool(a.value <= b.value)),
        Greater => return Ok(Value::Bool(a.value > b.value)),
        GreaterEqual => return Ok(Value::Bool(a.value >= b.value)),
        _ => {}
    }

    let signed = a.signed;
    let bits = a.bits;
    let int = match op {
        Add => IntValue::checked(signed, bits, a.value + b.value, "addition")?,
        Subtract => IntValue::checked(signed, bits, a.value - b.value, "subtraction")?,
        Multiply => IntValue::checked(signed, bits, a.value * b.value, "multiplication")?,
        Divide => {
            if b.value.is_zero() {
                return Err(InterpretError::DivisionByZero);
            }
            IntValue::checked(signed, bits, a.value / b.value, "division")?
        }
        Modulo => {
            if b.value.is_zero() {
                return Err(InterpretError::DivisionByZero);
            }
            // Noir's `%` is Rust `checked_rem`, which overflows (errors) on `i_MIN % -1` even
            // though the mathematical remainder is 0. Mirror that single edge case.
            let (min, _) = IntValue::range(signed, bits);
            if signed && a.value == min && b.value == -BigInt::from(1) {
                return Err(InterpretError::Overflow("modulo".to_string()));
            }
            IntValue::canonical(signed, bits, a.value % b.value)
        }
        And => IntValue::canonical(signed, bits, a.unsigned_repr() & b.unsigned_repr()),
        Or => IntValue::canonical(signed, bits, a.unsigned_repr() | b.unsigned_repr()),
        Xor => IntValue::canonical(signed, bits, a.unsigned_repr() ^ b.unsigned_repr()),
        ShiftLeft => {
            // Noir's `<<` follows Rust's `checked_shl` (see the SSA interpreter `BinaryOp::Shl`):
            // the shift amount must be in range, and the result wraps to the integer width.
            let amount = shift_amount(&b)?;
            if amount >= bits as usize {
                return Err(InterpretError::Overflow(
                    "shift-left amount >= bit width".to_string(),
                ));
            }
            IntValue::canonical(signed, bits, a.unsigned_repr() << amount)
        }
        ShiftRight => {
            // Rust's `checked_shr`: amount must be < width; unsigned is logical and signed is
            // arithmetic. Shifting the canonical (sign-aware) value handles both: a negative
            // BigInt shifts toward negative infinity.
            let amount = shift_amount(&b)?;
            if amount >= bits as usize {
                return Err(InterpretError::Overflow(
                    "shift-right amount >= bit width".to_string(),
                ));
            }
            IntValue::canonical(signed, bits, a.value >> amount)
        }
        Equal | NotEqual | Less | LessEqual | Greater | GreaterEqual => unreachable!(),
    };
    Ok(Value::Int(int))
}

fn shift_amount(b: &IntValue) -> Result<usize, InterpretError> {
    let repr = b.unsigned_repr();
    let (_, digits) = repr.to_u64_digits();
    match digits.as_slice() {
        [] => Ok(0),
        [single] => Ok(*single as usize),
        _ => Err(InterpretError::Overflow("shift amount".to_string())),
    }
}

fn int_type(typ: &Type) -> Option<(bool, u8)> {
    match typ {
        Type::Integer(signedness, bits) => Some((signedness.is_signed(), bits.bit_size())),
        _ => None,
    }
}

/// Reduce a signed `BigInt` literal into the compiled-in field, replicating Noir's removed
/// `SignedField::to_field_element` / `bigint_to_field`: a negative value maps to `modulus - |value|`.
fn bigint_to_field(value: &BigInt) -> FieldElement {
    let (sign, magnitude) = value.to_bytes_be();
    let field = FieldElement::from_be_bytes_reduce(&magnitude);
    if sign == Sign::Minus { -field } else { field }
}

/// Integer semantics covered here:
/// - Add/Sub/Mul are **checked**; overflow is an error.
/// - Signed Div/Mod **truncate toward zero**, remainder takes the dividend's sign
///   (`q_sign = sign_l ^ sign_r`, `r_sign = sign_l` in `lower_signed_divmod`).
/// - Unsigned Div/Mod give `q = floor(a/b)`, `r in [0, divisor)`.
#[cfg(test)]
mod semantics_tests {
    use super::*;

    fn int(signed: bool, bits: u8, v: i128) -> IntValue {
        IntValue::canonical(signed, bits, BigInt::from(v))
    }

    fn eval(op: BinaryOpKind, a: IntValue, b: IntValue) -> Result<Value, InterpretError> {
        eval_int_binary(op, a, b)
    }

    fn as_i128(value: Value) -> i128 {
        let Value::Int(i) = value else {
            panic!("expected integer, got {value:?}");
        };
        let digits = i.value.to_string();
        digits.parse().expect("fits i128")
    }

    #[test]
    fn signed_division_truncates_toward_zero() {
        // Matches `lower_signed_divmod`: magnitude division with q_sign = sign_l ^ sign_r.
        let cases = [
            (-7, 2, -3),
            (7, -2, -3),
            (-7, -2, 3),
            (7, 2, 3),
            (-8, 3, -2),
        ];
        for (a, b, expected) in cases {
            let got =
                as_i128(eval(BinaryOpKind::Divide, int(true, 8, a), int(true, 8, b)).unwrap());
            assert_eq!(got, expected, "{a} / {b}");
        }
    }

    #[test]
    fn signed_remainder_takes_dividend_sign() {
        // Matches `lower_signed_divmod`: r_sign = sign_l (the dividend's sign).
        let cases = [(-7, 2, -1), (7, -2, 1), (-7, -2, -1), (7, 2, 1)];
        for (a, b, expected) in cases {
            let got =
                as_i128(eval(BinaryOpKind::Modulo, int(true, 8, a), int(true, 8, b)).unwrap());
            assert_eq!(got, expected, "{a} % {b}");
        }
    }

    #[test]
    fn signed_modulo_overflows_on_min_mod_neg_one() {
        // Noir's `%` is Rust checked_rem: i_MIN % -1 overflows (errors) despite a math result of 0.
        assert!(matches!(
            eval(BinaryOpKind::Modulo, int(true, 8, -128), int(true, 8, -1)),
            Err(InterpretError::Overflow(_))
        ));
        // A normal signed modulo still works.
        assert_eq!(
            as_i128(eval(BinaryOpKind::Modulo, int(true, 8, -7), int(true, 8, 3)).unwrap()),
            -1
        );
    }

    #[test]
    fn unsigned_division_is_floor() {
        let got =
            as_i128(eval(BinaryOpKind::Divide, int(false, 8, 200), int(false, 8, 7)).unwrap());
        assert_eq!(got, 28); // floor(200/7)
        let rem =
            as_i128(eval(BinaryOpKind::Modulo, int(false, 8, 200), int(false, 8, 7)).unwrap());
        assert_eq!(rem, 4); // 200 - 28*7
    }

    #[test]
    fn unsigned_bitwise_matches_bit_patterns() {
        // 0b1100 & 0b1010 = 0b1000; | = 0b1110; ^ = 0b0110 (witness_bitwise.rs lower_word_bitwise).
        assert_eq!(
            as_i128(eval(BinaryOpKind::And, int(false, 8, 12), int(false, 8, 10)).unwrap()),
            8
        );
        assert_eq!(
            as_i128(eval(BinaryOpKind::Or, int(false, 8, 12), int(false, 8, 10)).unwrap()),
            14
        );
        assert_eq!(
            as_i128(eval(BinaryOpKind::Xor, int(false, 8, 12), int(false, 8, 10)).unwrap()),
            6
        );
    }

    #[test]
    fn shift_right_logical_and_arithmetic() {
        // Unsigned shr is logical (floor); signed shr is arithmetic (sign-preserving).
        assert_eq!(
            as_i128(
                eval(
                    BinaryOpKind::ShiftRight,
                    int(false, 8, 200),
                    int(false, 8, 2)
                )
                .unwrap()
            ),
            50
        );
        // Shift operands share one type in a well-typed AST (Noir's `Shl`/`Shr` are
        // `fn(self, other: Self)`; the SSA interpreter only handles same-type pairs), so the
        // signed shift's amount is the same signed type.
        assert_eq!(
            as_i128(eval(BinaryOpKind::ShiftRight, int(true, 8, -8), int(true, 8, 1)).unwrap()),
            -4
        );
        // Only a shift amount >= the width is an error (Rust checked_shr).
        assert!(matches!(
            eval(BinaryOpKind::ShiftRight, int(false, 8, 1), int(false, 8, 8)),
            Err(InterpretError::Overflow(_))
        ));
    }

    #[test]
    fn shift_left_wraps_value_errors_only_on_overshift() {
        // Noir `<<` (Rust checked_shl): the value wraps to the width; only an amount >= the width
        // is an error. So 0xff << 1 == 0xfe, but 1u8 << 8 overflows.
        assert_eq!(
            as_i128(
                eval(
                    BinaryOpKind::ShiftLeft,
                    int(false, 8, 0xff),
                    int(false, 8, 1)
                )
                .unwrap()
            ),
            0xfe
        );
        assert_eq!(
            as_i128(eval(BinaryOpKind::ShiftLeft, int(false, 8, 1), int(false, 8, 7)).unwrap()),
            128
        );
        assert!(matches!(
            eval(BinaryOpKind::ShiftLeft, int(false, 8, 1), int(false, 8, 8)),
            Err(InterpretError::Overflow(_))
        ));
    }

    #[test]
    fn arithmetic_is_checked_not_wrapping() {
        // Overflow is reported instead of silently wrapping to 44 (u8) / 144 (i8).
        assert!(matches!(
            eval(BinaryOpKind::Add, int(false, 8, 200), int(false, 8, 100)),
            Err(InterpretError::Overflow(_))
        ));
        assert!(matches!(
            eval(BinaryOpKind::Add, int(true, 8, 100), int(true, 8, 44)),
            Err(InterpretError::Overflow(_))
        ));
        assert!(matches!(
            eval(BinaryOpKind::Subtract, int(false, 8, 0), int(false, 8, 1)),
            Err(InterpretError::Overflow(_))
        ));
    }
}

#[cfg(test)]
mod aggregate_and_fmt_tests {
    use super::*;
    use noirc_frontend::ast::IntegerBitSize;
    use noirc_frontend::monomorphization::ast::{Assign, Ident, IdentId, LocalId, Program};
    use noirc_frontend::shared::Signedness;

    fn i32v(n: i128) -> Value {
        Value::Int(IntValue::canonical(true, 32, BigInt::from(n)))
    }

    fn u32v(n: u32) -> Value {
        Value::Int(IntValue::canonical(false, 32, BigInt::from(n)))
    }

    fn ident(local: LocalId, name: &str, typ: Type, id: u32) -> Ident {
        Ident {
            location: None,
            definition: Definition::Local(local),
            mutable: true,
            name: name.to_string(),
            typ: Rc::new(typ),
            id: IdentId(id),
        }
    }

    fn u32_literal(n: u32, typ: &Type) -> Expression {
        Expression::Literal(Literal::Integer(
            BigInt::from(n),
            typ.clone(),
            Location::dummy(),
        ))
    }

    #[test]
    fn aggregate_shape_matches_only_same_kind() {
        assert!(same_aggregate_shape(
            &Value::Array(vec![]),
            &Value::Array(vec![])
        ));
        assert!(same_aggregate_shape(
            &Value::tuple(vec![]),
            &Value::tuple(vec![])
        ));
        assert!(same_aggregate_shape(
            &Value::Str("a".into()),
            &Value::Str("b".into())
        ));
        // Scalars and cross-kind pairs are not aggregate-shaped, so they stay a loud Type error.
        assert!(!same_aggregate_shape(&i32v(1), &i32v(2)));
        assert!(!same_aggregate_shape(
            &Value::Array(vec![]),
            &Value::tuple(vec![])
        ));
    }

    #[test]
    fn nested_lvalue_indices_evaluate_inner_first() {
        let u32_type = Type::Integer(Signedness::Unsigned, IntegerBitSize::ThirtyTwo);
        let row_type = Type::Array(2, Rc::new(u32_type.clone()));
        let array_type = Type::Array(2, Rc::new(row_type.clone()));
        let array_local = LocalId(0);
        let index_local = LocalId(1);
        let array_ident = ident(array_local, "array", array_type, 0);
        let index_ident = ident(index_local, "index", u32_type.clone(), 1);

        let inner_index = Expression::Block(vec![
            Expression::Assign(Assign {
                lvalue: LValue::Ident(index_ident.clone()),
                expression: Box::new(u32_literal(1, &u32_type)),
            }),
            u32_literal(0, &u32_type),
        ]);
        let target = LValue::Index {
            array: Box::new(LValue::Index {
                array: Box::new(LValue::Ident(array_ident)),
                index: Box::new(inner_index),
                element_type: row_type,
                location: Location::dummy(),
            }),
            index: Box::new(Expression::Ident(index_ident)),
            element_type: u32_type,
            location: Location::dummy(),
        };

        let index = Rc::new(RefCell::new(u32v(0)));
        let mut env = Frame::new();
        env.insert(
            array_local,
            Value::Array(vec![
                Value::Array(vec![u32v(0), u32v(1)]),
                Value::Array(vec![u32v(2), u32v(3)]),
            ]),
        );
        env.insert(index_local, Value::Ref(index.clone(), true));

        let program = Program::default();
        let mut interpreter = Interpreter::new(&program);
        assert_eq!(
            interpreter.lvalue_value(&target, &mut env).unwrap(),
            u32v(1)
        );
        assert_eq!(*index.borrow(), u32v(1));
    }

    /// The shapes `renders_assert_message`'s end-to-end string does not already pin: a vector's
    /// `@[..]` prefix and a negative signed integer.
    #[test]
    fn format_value_uses_noir_display_rules() {
        let i32_type = PrintableType::SignedInteger { width: 32 };
        assert_eq!(format_value(&i32v(-7), &i32_type).unwrap(), "-7");
        assert_eq!(
            format_value(
                &Value::Array(vec![i32v(1), i32v(2)]),
                &PrintableType::Vector {
                    typ: Box::new(i32_type)
                }
            )
            .unwrap(),
            "@[1, 2]"
        );
    }
}
