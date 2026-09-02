//! Fingerprint monomorphized programs, ignoring source locations, debug data and item IDs.
//! Bump [`PROJECTION_VERSION`] whenever the canonical format changes.

use std::collections::{HashMap, VecDeque};

use noirc_frontend::hir_def::expr::Constructor;
use noirc_frontend::monomorphization::ast::{
    ArrayLiteral, Definition, Expression, FuncId, Function, GlobalId, LValue, Literal, LocalId,
    Program,
};
use noirc_frontend::token::FmtStrFragment;
use noirc_printable_type::PrintableType;
use sha2::{Digest, Sha256};

/// The projection's format version; part of every dump and ledger header.
pub const PROJECTION_VERSION: u32 = 2;

/// The SHA-256 of [`canonical_text`], as lowercase hex.
pub fn projection_hash(program: &Program) -> String {
    let digest = Sha256::digest(canonical_text(program).as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The canonical text of `program`, one s-expression per function and global.
pub fn canonical_text(program: &Program) -> String {
    let mut canonicalizer = Canonicalizer {
        program,
        out: format!("(program v{PROJECTION_VERSION}\n"),
        functions: HashMap::new(),
        function_queue: VecDeque::new(),
        globals: HashMap::new(),
        global_queue: VecDeque::new(),
        locals: HashMap::new(),
    };
    canonicalizer.run();
    canonicalizer.out.push_str(")\n");
    canonicalizer.out
}

struct Canonicalizer<'p> {
    program: &'p Program,
    out: String,
    functions: HashMap<FuncId, u32>,
    function_queue: VecDeque<FuncId>,
    globals: HashMap<GlobalId, u32>,
    global_queue: VecDeque<GlobalId>,
    /// Reset for every function body and every global initializer.
    locals: HashMap<LocalId, u32>,
}

impl<'p> Canonicalizer<'p> {
    fn run(&mut self) {
        self.function_id(Program::main_id());
        self.drain();
        // Unreachable items follow in emission order.
        let leftover_functions: Vec<FuncId> = self
            .program
            .functions
            .iter()
            .map(|f| f.id)
            .filter(|id| !self.functions.contains_key(id))
            .collect();
        let leftover_globals: Vec<GlobalId> = self
            .program
            .globals
            .keys()
            .copied()
            .filter(|id| !self.globals.contains_key(id))
            .collect();
        if !leftover_functions.is_empty() || !leftover_globals.is_empty() {
            self.out.push_str(" (unreachable\n");
            for id in leftover_functions {
                self.function_id(id);
            }
            for id in leftover_globals {
                self.global_id(id);
            }
            self.drain();
            self.out.push_str(" )\n");
        }
    }

    fn drain(&mut self) {
        loop {
            if let Some(id) = self.function_queue.pop_front() {
                self.emit_function(id);
            } else if let Some(id) = self.global_queue.pop_front() {
                self.emit_global(id);
            } else {
                break;
            }
        }
    }

    fn function_id(&mut self, id: FuncId) -> u32 {
        if let Some(n) = self.functions.get(&id) {
            return *n;
        }
        let n = self.functions.len() as u32;
        self.functions.insert(id, n);
        self.function_queue.push_back(id);
        n
    }

    fn global_id(&mut self, id: GlobalId) -> u32 {
        if let Some(n) = self.globals.get(&id) {
            return *n;
        }
        let n = self.globals.len() as u32;
        self.globals.insert(id, n);
        self.global_queue.push_back(id);
        n
    }

    fn local_id(&mut self, id: LocalId) -> u32 {
        if let Some(n) = self.locals.get(&id) {
            return *n;
        }
        let n = self.locals.len() as u32;
        self.locals.insert(id, n);
        n
    }

    fn function_by_id(&self, id: FuncId) -> Option<&'p Function> {
        let program: &'p Program = self.program;
        program
            .functions
            .get(id.0 as usize)
            .filter(|f| f.id == id)
            .or_else(|| program.functions.iter().find(|f| f.id == id))
    }

    fn emit_function(&mut self, id: FuncId) {
        let n = self.functions[&id];
        let Some(function) = self.function_by_id(id) else {
            self.out.push_str(&format!(" (fn f#{n} missing)\n"));
            return;
        };
        self.locals.clear();
        self.out.push_str(&format!(
            " (fn f#{n} {:?} unconstrained={} inline={} entry={} allow_constant_return={} \
             visibility={:?}\n  (params",
            function.name,
            function.unconstrained,
            function.inline_type,
            function.is_entry_point,
            function.allow_constant_return,
            function.return_visibility,
        ));
        for (local, mutable, name, typ, visibility) in &function.parameters {
            let l = self.local_id(*local);
            self.out.push_str(&format!(
                " (l#{l} mut={mutable} {name:?} {:?} {visibility:?})",
                typ
            ));
        }
        self.out
            .push_str(&format!(")\n  -> {:?}\n  ", function.return_type));
        self.expr(&function.body);
        self.out.push_str(")\n");
    }

    fn emit_global(&mut self, id: GlobalId) {
        let n = self.globals[&id];
        let Some((name, typ, expression)) = self.program.globals.get(&id) else {
            self.out.push_str(&format!(" (global g#{n} missing)\n"));
            return;
        };
        self.locals.clear();
        self.out
            .push_str(&format!(" (global g#{n} {name:?} {:?} ", typ));
        self.expr(expression);
        self.out.push_str(")\n");
    }

    fn definition(&mut self, definition: &Definition) -> String {
        match definition {
            Definition::Local(id) => format!("l#{}", self.local_id(*id)),
            Definition::Global(id) => format!("g#{}", self.global_id(*id)),
            Definition::Function(id) => format!("f#{}", self.function_id(*id)),
            Definition::Builtin(name) => format!("builtin:{name:?}"),
            Definition::LowLevel(name) => format!("lowlevel:{name:?}"),
            Definition::Oracle { name, pure } => format!("oracle:{name:?}:pure={pure}"),
        }
    }

    fn exprs(&mut self, expressions: &[Expression]) {
        for expression in expressions {
            self.out.push(' ');
            self.expr(expression);
        }
    }

    fn expr(&mut self, expression: &Expression) {
        match expression {
            Expression::Ident(ident) => {
                let definition = self.definition(&ident.definition);
                self.out.push_str(&format!(
                    "(ident {definition} mut={} {:?} {:?})",
                    ident.mutable, ident.name, ident.typ
                ));
            }
            Expression::Literal(literal) => self.literal(literal),
            Expression::Block(expressions) => {
                self.out.push_str("(block");
                self.exprs(expressions);
                self.out.push(')');
            }
            Expression::Unary(unary) => {
                self.out.push_str(&format!(
                    "(unary {:?} skip={} {:?} ",
                    unary.operator, unary.skip, unary.result_type
                ));
                self.expr(&unary.rhs);
                self.out.push(')');
            }
            Expression::Binary(binary) => {
                self.out
                    .push_str(&format!("(binary {:?} ", binary.operator));
                self.expr(&binary.lhs);
                self.out.push(' ');
                self.expr(&binary.rhs);
                self.out.push(')');
            }
            Expression::Index(index) => {
                self.out
                    .push_str(&format!("(index {:?} ", index.element_type));
                self.expr(&index.collection);
                self.out.push(' ');
                self.expr(&index.index);
                self.out.push(')');
            }
            Expression::Cast(cast) => {
                self.out.push_str(&format!("(cast {:?} ", cast.r#type));
                self.expr(&cast.lhs);
                self.out.push(')');
            }
            Expression::For(for_) => {
                let l = self.local_id(for_.index_variable);
                self.out.push_str(&format!(
                    "(for l#{l} {:?} {:?} inclusive={} ",
                    for_.index_name, for_.index_type, for_.inclusive
                ));
                self.expr(&for_.start_range);
                self.out.push(' ');
                self.expr(&for_.end_range);
                self.out.push(' ');
                self.expr(&for_.block);
                self.out.push(')');
            }
            Expression::Loop(body) => {
                self.out.push_str("(loop ");
                self.expr(body);
                self.out.push(')');
            }
            Expression::While(while_) => {
                self.out.push_str("(while ");
                self.expr(&while_.condition);
                self.out.push(' ');
                self.expr(&while_.body);
                self.out.push(')');
            }
            Expression::If(if_) => {
                self.out.push_str(&format!("(if {:?} ", if_.typ));
                self.expr(&if_.condition);
                self.out.push(' ');
                self.expr(&if_.consequence);
                match &if_.alternative {
                    Some(alternative) => {
                        self.out.push_str(" (else ");
                        self.expr(alternative);
                        self.out.push(')');
                    }
                    None => self.out.push_str(" none"),
                }
                self.out.push(')');
            }
            Expression::Match(match_) => {
                let l = self.local_id(match_.variable_to_match.0);
                self.out.push_str(&format!(
                    "(match l#{l} {:?} {:?}",
                    match_.variable_to_match.1, match_.typ
                ));
                for case in &match_.cases {
                    self.out.push_str(" (case ");
                    self.constructor(&case.constructor);
                    self.out.push_str(" (args");
                    for (local, name) in &case.arguments {
                        let l = self.local_id(*local);
                        self.out.push_str(&format!(" l#{l}:{name:?}"));
                    }
                    self.out.push_str(") ");
                    self.expr(&case.branch);
                    self.out.push(')');
                }
                match &match_.default_case {
                    Some(default) => {
                        self.out.push_str(" (default ");
                        self.expr(default);
                        self.out.push(')');
                    }
                    None => self.out.push_str(" none"),
                }
                self.out.push(')');
            }
            Expression::Tuple(expressions) => {
                self.out.push_str("(tuple");
                self.exprs(expressions);
                self.out.push(')');
            }
            Expression::ExtractTupleField(expression, index) => {
                self.out.push_str(&format!("(field {index} "));
                self.expr(expression);
                self.out.push(')');
            }
            Expression::Call(call) => {
                self.out.push_str(&format!("(call {:?} ", call.return_type));
                self.expr(&call.func);
                self.exprs(&call.arguments);
                self.out.push(')');
            }
            Expression::Let(let_) => {
                let l = self.local_id(let_.id);
                self.out
                    .push_str(&format!("(let l#{l} mut={} {:?} ", let_.mutable, let_.name));
                self.expr(&let_.expression);
                self.out.push(')');
            }
            Expression::Constrain(condition, _, message) => {
                self.out.push_str("(constrain ");
                self.expr(condition);
                match message {
                    Some(message) => {
                        let (expression, typ) = message.as_ref();
                        self.out
                            .push_str(&format!(" (msg {:?} ", PrintableType::from(typ)));
                        self.expr(expression);
                        self.out.push(')');
                    }
                    None => self.out.push_str(" none"),
                }
                self.out.push(')');
            }
            Expression::Assign(assign) => {
                self.out.push_str("(assign ");
                self.lvalue(&assign.lvalue);
                self.out.push(' ');
                self.expr(&assign.expression);
                self.out.push(')');
            }
            Expression::Semi(expression) => {
                self.out.push_str("(semi ");
                self.expr(expression);
                self.out.push(')');
            }
            Expression::Clone(expression) => {
                self.out.push_str("(clone ");
                self.expr(expression);
                self.out.push(')');
            }
            Expression::Drop(expression) => {
                self.out.push_str("(drop ");
                self.expr(expression);
                self.out.push(')');
            }
            Expression::Break => self.out.push_str("break"),
            Expression::Continue => self.out.push_str("continue"),
        }
    }

    fn array(&mut self, tag: &str, literal: &ArrayLiteral) {
        self.out.push_str(&format!("({tag} {:?}", literal.typ));
        self.exprs(&literal.contents);
        self.out.push(')');
    }

    fn literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Array(literal) => self.array("array", literal),
            Literal::Vector(literal) => self.array("vector", literal),
            Literal::Repeated {
                element,
                length,
                is_vector,
                typ,
            } => {
                self.out
                    .push_str(&format!("(repeated {:?} {length} vector={is_vector} ", typ));
                self.expr(element);
                self.out.push(')');
            }
            Literal::Integer(value, typ, _) => {
                self.out.push_str(&format!("(int {value} {:?})", typ));
            }
            Literal::Bool(value) => self.out.push_str(&format!("(bool {value})")),
            Literal::Unit => self.out.push_str("unit"),
            Literal::Str(bytes) => {
                self.out
                    .push_str(&format!("(str \"{}\")", bytes.escape_ascii()));
            }
            Literal::FmtStr(fragments, count, captures) => {
                self.out.push_str(&format!("(fmtstr {count} ["));
                for fragment in fragments {
                    match fragment {
                        FmtStrFragment::String(text) => {
                            self.out.push_str(&format!(" (s {text:?})"));
                        }
                        FmtStrFragment::Interpolation(name, _) => {
                            self.out.push_str(&format!(" (i {name:?})"));
                        }
                    }
                }
                self.out.push_str(" ] ");
                self.expr(captures);
                self.out.push(')');
            }
        }
    }

    fn constructor(&mut self, constructor: &Constructor) {
        match constructor {
            Constructor::True => self.out.push_str("true"),
            Constructor::False => self.out.push_str("false"),
            Constructor::Unit => self.out.push_str("unit"),
            Constructor::Int(value) => self.out.push_str(&format!("(int {value})")),
            Constructor::Tuple(types) => {
                self.out.push_str(&format!("(tuple {}", types.len()));
                for typ in types {
                    self.out.push_str(&format!(" {:?}", typ.to_string()));
                }
                self.out.push(')');
            }
            Constructor::Variant(typ, index) => {
                self.out.push_str(&format!(
                    "(variant {:?} {index} enum={})",
                    typ.to_string(),
                    constructor.is_enum()
                ));
            }
            Constructor::Range(low, high) => {
                self.out.push_str(&format!("(range {low} {high})"));
            }
        }
    }

    fn lvalue(&mut self, lvalue: &LValue) {
        match lvalue {
            LValue::Ident(ident) => {
                let definition = self.definition(&ident.definition);
                self.out.push_str(&format!(
                    "(lident {definition} mut={} {:?} {:?})",
                    ident.mutable, ident.name, ident.typ
                ));
            }
            LValue::Index {
                array,
                index,
                element_type,
                ..
            } => {
                self.out.push_str(&format!("(lindex {:?} ", element_type));
                self.lvalue(array);
                self.out.push(' ');
                self.expr(index);
                self.out.push(')');
            }
            LValue::MemberAccess {
                object,
                field_index,
            } => {
                self.out.push_str(&format!("(lmember {field_index} "));
                self.lvalue(object);
                self.out.push(')');
            }
            LValue::Dereference {
                reference,
                element_type,
            } => {
                self.out.push_str(&format!("(lderef {:?} ", element_type));
                self.lvalue(reference);
                self.out.push(')');
            }
            LValue::Clone(lvalue) => {
                self.out.push_str("(lclone ");
                self.lvalue(lvalue);
                self.out.push(')');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use noirc_errors::{Location, Span};
    use noirc_frontend::ast::{BinaryOpKind, IntegerBitSize};
    use noirc_frontend::monomorphization::ast::{Binary, Call, Ident, IdentId, InlineType, Type};
    use noirc_frontend::shared::{Signedness, Visibility};
    use num_bigint::BigInt;

    use super::*;

    fn u64_type() -> Type {
        Type::Integer(Signedness::Unsigned, IntegerBitSize::SixtyFour)
    }

    fn location(offset: u32) -> Location {
        Location::new(Span::from(offset..offset + 1), fm::FileId::dummy())
    }

    fn local(id: u32, name: &str) -> Expression {
        Expression::Ident(Ident {
            location: Some(location(id)),
            definition: Definition::Local(LocalId(id)),
            mutable: false,
            name: name.to_string(),
            typ: Rc::new(u64_type()),
            id: IdentId(id * 10),
        })
    }

    fn int(value: u64, offset: u32) -> Expression {
        Expression::Literal(Literal::Integer(
            BigInt::from(value),
            u64_type(),
            location(offset),
        ))
    }

    fn binary(op: BinaryOpKind, lhs: Expression, rhs: Expression, offset: u32) -> Expression {
        Expression::Binary(Binary {
            lhs: Box::new(lhs),
            operator: op,
            rhs: Box::new(rhs),
            location: location(offset),
        })
    }

    fn call(callee: u32, name: &str, argument: Expression, offset: u32) -> Expression {
        Expression::Call(Call {
            func: Box::new(Expression::Ident(Ident {
                location: None,
                definition: Definition::Function(FuncId(callee)),
                mutable: false,
                name: name.to_string(),
                typ: Rc::new(Type::Function(
                    vec![u64_type()],
                    Rc::new(u64_type()),
                    Rc::new(Type::Unit),
                    false,
                )),
                id: IdentId(callee * 100),
            })),
            arguments: vec![argument],
            return_type: u64_type(),
            location: location(offset),
        })
    }

    fn function(id: u32, name: &str, params: &[(u32, &str)], body: Expression) -> Function {
        Function {
            id: FuncId(id),
            name: name.to_string(),
            parameters: params
                .iter()
                .map(|(local, name)| {
                    (
                        LocalId(*local),
                        false,
                        name.to_string(),
                        Rc::new(u64_type()),
                        Visibility::Private,
                    )
                })
                .collect(),
            body,
            return_type: u64_type(),
            return_visibility: Visibility::Public,
            unconstrained: false,
            inline_type: InlineType::default(),
            is_entry_point: id == 0,
            allow_constant_return: false,
        }
    }

    fn program(functions: Vec<Function>) -> Program {
        Program {
            functions,
            ..Default::default()
        }
    }

    /// `main(x, y) = a(x) + b(y)`, `a(v) = v * 2`, `b(v) = v + 3`, with the given numbering of the
    /// helpers, their locals and the source offsets.
    fn sample(a: u32, b: u32, x: u32, y: u32, v: u32, offset: u32) -> Program {
        let main = function(
            0,
            "main",
            &[(x, "x"), (y, "y")],
            binary(
                BinaryOpKind::Add,
                call(a, "a", local(x, "x"), offset),
                call(b, "b", local(y, "y"), offset + 1),
                offset + 2,
            ),
        );
        let helper_a = function(
            a,
            "a",
            &[(v, "v")],
            binary(
                BinaryOpKind::Multiply,
                local(v, "v"),
                int(2, offset + 3),
                offset + 4,
            ),
        );
        let helper_b = function(
            b,
            "b",
            &[(v, "v")],
            binary(
                BinaryOpKind::Add,
                local(v, "v"),
                int(3, offset + 5),
                offset + 6,
            ),
        );
        let mut functions = vec![main, helper_a, helper_b];
        functions.sort_by_key(|f| f.id.0);
        program(functions)
    }

    #[test]
    fn renumbered_functions_and_locals_project_identically() {
        let one = sample(1, 2, 0, 1, 0, 0);
        let other = sample(2, 1, 7, 3, 9, 0);
        assert_eq!(canonical_text(&one), canonical_text(&other));
    }

    #[test]
    fn locations_do_not_affect_the_projection() {
        assert_eq!(
            projection_hash(&sample(1, 2, 0, 1, 0, 0)),
            projection_hash(&sample(1, 2, 0, 1, 0, 500))
        );
    }

    #[test]
    fn a_changed_literal_changes_the_projection() {
        let mut changed = sample(1, 2, 0, 1, 0, 0);
        changed.functions[2].body = binary(BinaryOpKind::Add, local(0, "v"), int(4, 0), 0);
        assert_ne!(
            projection_hash(&sample(1, 2, 0, 1, 0, 0)),
            projection_hash(&changed)
        );
    }

    #[test]
    fn a_swapped_call_order_changes_the_projection() {
        let mut swapped = sample(1, 2, 0, 1, 0, 0);
        swapped.functions[0].body = binary(
            BinaryOpKind::Add,
            call(2, "b", local(1, "y"), 0),
            call(1, "a", local(0, "x"), 0),
            0,
        );
        assert_ne!(
            projection_hash(&sample(1, 2, 0, 1, 0, 0)),
            projection_hash(&swapped)
        );
    }

    #[test]
    fn unreachable_items_do_not_repeat_reachable_items() {
        let mut program = sample(1, 2, 0, 1, 0, 0);
        program
            .functions
            .push(function(3, "unused", &[], int(5, 0)));
        program
            .globals
            .insert(GlobalId(0), ("unused".into(), u64_type(), int(6, 0)));
        let text = canonical_text(&program);
        assert_eq!(text.matches(" (fn ").count(), program.functions.len());
        assert_eq!(text.matches(" (global ").count(), program.globals.len());
        assert!(text.contains("(unreachable"));
    }

    #[test]
    fn distinct_types_have_distinct_projections() {
        let function_type = |ret, env| Type::Function(vec![], Rc::new(ret), Rc::new(env), false);
        let pairs = [
            (Type::Unit, Type::Tuple(vec![])),
            (
                function_type(
                    function_type(Type::Bool, Type::Tuple(vec![Type::Field])),
                    Type::Unit,
                ),
                function_type(
                    function_type(Type::Bool, Type::Unit),
                    Type::Tuple(vec![Type::Field]),
                ),
            ),
        ];
        let hash = |typ| {
            let mut main = function(0, "main", &[(0, "x")], int(7, 0));
            main.parameters[0].3 = Rc::new(typ);
            projection_hash(&program(vec![main]))
        };
        for (a, b) in pairs {
            assert_eq!(a.to_string(), b.to_string());
            assert_ne!(hash(a), hash(b));
        }
    }

    #[test]
    fn assertion_payload_field_names_change_the_projection() {
        let hash = |field| {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir(root.path().join("src")).unwrap();
            std::fs::write(
                root.path().join("Nargo.toml"),
                "[package]\nname = \"projection\"\ntype = \"bin\"\nauthors = []\n",
            )
            .unwrap();
            std::fs::write(
                root.path().join("src/main.nr"),
                format!(
                    "struct Foo {{ {field}: u32 }}\n\
                     fn main(foo: Foo, x: u32) {{ assert(x == 1, f\"{{foo}}\"); }}"
                ),
            )
            .unwrap();
            let project = crate::loader::NoirProject::new(root.path().to_path_buf()).unwrap();
            let compiled = crate::validation_frontend::compile_for_validation(&project).unwrap();
            projection_hash(&compiled.program)
        };
        assert_ne!(hash("a"), hash("b"));
    }

    #[test]
    fn the_canonical_text_format_is_pinned() {
        let program = program(vec![function(0, "main", &[(4, "x")], int(7, 0))]);
        assert_eq!(
            canonical_text(&program),
            "(program v2\n \
             (fn f#0 \"main\" unconstrained=false inline=inline entry=true allow_constant_return=false visibility=Public\n  \
             (params (l#0 mut=false \"x\" Integer(Unsigned, SixtyFour) Private))\n  \
             -> Integer(Unsigned, SixtyFour)\n  \
             (int 7 Integer(Unsigned, SixtyFour)))\n\
             )\n"
        );
    }
}
