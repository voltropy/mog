// QBE Backend Code Generator for Mog
// Generates QBE IL from analyzed AST, targeting the QBE compiler backend
// QBE base types: w (i32), l (i64), s (f32), d (f64). All pointers are l.

use crate::ast::*;
use crate::capability::CapabilityDecl;
use crate::types::{FloatKind, IntegerKind, Type};
use std::collections::{HashMap, HashSet};

const CORO_HEADER_SIZE: u32 = 24;

pub struct QBECodeGen {
    output: String,
    data_section: String,
    reg_counter: u32,
    label_counter: u32,
    string_counter: u32,
    string_constants: HashMap<String, String>,
    variable_registers: HashMap<String, String>,
    variable_types: HashMap<String, Type>,
    function_types: HashMap<String, Type>,
    struct_fields: HashMap<String, Vec<(String, Type)>>,
    loop_stack: Vec<(String, String)>,
    try_stack: Vec<String>,
    current_function_return_type: Option<Type>,
    in_async_function: bool,
    capability_decls: HashMap<String, CapabilityDecl>,
    coro_frame: Option<String>,
    coro_future: Option<String>,
    await_counter: u32,
    coro_spilled_vars: Vec<String>,
    _coro_frame_size: u32,
    coro_resume_blocks: Vec<String>,
    plugin_mode: bool,
    plugin_name: Option<String>,
    plugin_version: Option<String>,
    pub_functions: Vec<(String, usize, bool)>, // (name, param_count, is_async)
    lambda_counter: u32,
    lambda_funcs: String,
    block_terminated: bool,
    entry_allocs: Vec<String>,
    current_func_lines: Vec<String>,
    capabilities: HashSet<String>,
    async_functions: HashSet<String>,
    has_async_functions: bool,
    package_prefix: String,
    imported_packages: HashMap<String, String>,
    function_param_info: HashMap<String, Vec<(String, Type)>>,
    /// Tracks registers that hold `d` (f64) values, so we can emit
    /// `stored` instead of `storel` when storing them to memory.
    float_regs: HashSet<String>,
}

impl QBECodeGen {
    pub fn new() -> Self {
        QBECodeGen {
            output: String::new(),
            data_section: String::new(),
            reg_counter: 0,
            label_counter: 0,
            string_counter: 0,
            string_constants: HashMap::new(),
            variable_registers: HashMap::new(),
            variable_types: HashMap::new(),
            function_types: HashMap::new(),
            struct_fields: HashMap::new(),
            loop_stack: Vec::new(),
            try_stack: Vec::new(),
            current_function_return_type: None,
            in_async_function: false,
            capability_decls: HashMap::new(),
            coro_frame: None,
            coro_future: None,
            await_counter: 0,
            coro_spilled_vars: Vec::new(),
            _coro_frame_size: 0,
            coro_resume_blocks: Vec::new(),
            plugin_mode: false,
            plugin_name: None,
            plugin_version: None,
            pub_functions: Vec::new(),
            lambda_counter: 0,
            lambda_funcs: String::new(),
            block_terminated: false,
            entry_allocs: Vec::new(),
            current_func_lines: Vec::new(),
            capabilities: HashSet::new(),
            async_functions: HashSet::new(),
            has_async_functions: false,
            package_prefix: String::new(),
            imported_packages: HashMap::new(),
            function_param_info: HashMap::new(),
            float_regs: HashSet::new(),
        }
    }

    // --- Register & Label Management ---
    fn fresh_reg(&mut self) -> String {
        let r = format!("%v.{}", self.reg_counter);
        self.reg_counter += 1;
        r
    }

    fn fresh_label(&mut self) -> String {
        let l = format!("@L.{}", self.label_counter);
        self.label_counter += 1;
        l
    }

    fn reset_counters(&mut self) {
        self.reg_counter = 0;
        self.label_counter = 0;
        self.float_regs.clear();
    }

    // --- Emit Helpers ---
    fn emit(&mut self, line: &str) {
        self.current_func_lines.push(line.to_string());
    }

    fn emit_data(&mut self, line: &str) {
        self.data_section.push_str(line);
        self.data_section.push('\n');
    }

    fn emit_alloc(&mut self, line: &str) {
        self.entry_allocs.push(line.to_string());
        // Zero-initialize every alloc slot
        if let Some(reg) = line.split('=').next().map(|s| s.trim()) {
            self.entry_allocs.push(format!("  storel 0, {}", reg));
        }
    }

    // --- Type Mapping ---
    fn to_qbe_type(&self, ty: &Type) -> &'static str {
        match ty {
            Type::Float(FloatKind::F32) => "s",
            Type::Float(_) => "d",
            Type::Void => "void",
            Type::Future(inner) => self.to_qbe_type(inner),
            Type::TypeAlias { aliased_type, .. } => self.to_qbe_type(aliased_type),
            _ => "l",
        }
    }

    fn ret_type_str(&self, ty: &Type) -> &'static str {
        let t = self.to_qbe_type(ty);
        if t == "void" {
            ""
        } else {
            t
        }
    }

    /// Return the correct QBE store instruction for a given value.
    /// `stored` for f64 (d), `stores` for f32 (s), `storel` for everything else (l/ptr).
    fn store_op_for_expr(&self, expr: &Expr) -> &'static str {
        if self.is_float_operand(expr) {
            "stored"
        } else {
            "storel"
        }
    }

    /// Emit a store instruction, choosing stored/storel based on whether a
    /// named variable is known to be float-typed.
    fn emit_store_var(&mut self, val: &str, dest: &str, var_name: &str) {
        let is_float = matches!(self.variable_types.get(var_name), Some(t) if t.is_float());
        let op = if is_float { "stored" } else { "storel" };
        self.emit(&format!("    {} {}, {}", op, val, dest));
    }

    /// Return the correct store instruction based on whether a register
    /// was recorded as holding a float value.
    fn store_op_for_reg(&self, reg: &str) -> &'static str {
        if self.float_regs.contains(reg) {
            "stored"
        } else {
            "storel"
        }
    }

    /// Mark a register as holding a float (d) value.
    fn mark_float_reg(&mut self, reg: &str) {
        self.float_regs.insert(reg.to_string());
    }

    /// Ensure a register is `l` type. If it's a float (`d`), cast it to `l`.
    fn ensure_long(&mut self, reg: &str) -> String {
        if self.float_regs.contains(reg) {
            let r = self.fresh_reg();
            self.emit(&format!("  {} =l cast {}", r, reg));
            r
        } else {
            reg.to_string()
        }
    }

    /// Ensure a register is `d` type. If it's `l`, convert it with sltof.
    fn ensure_double(&mut self, reg: &str) -> String {
        if self.float_regs.contains(reg) {
            reg.to_string()
        } else {
            let r = self.fresh_reg();
            self.emit(&format!("  {} =d sltof {}", r, reg));
            self.mark_float_reg(&r);
            r
        }
    }

    // --- String Escaping for QBE ---
    fn escape_string_for_qbe(s: &str) -> String {
        let mut result = String::new();
        for ch in s.chars() {
            match ch {
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                '\0' => result.push_str("\\0"),
                c if (c as u32) >= 32 && (c as u32) <= 126 => result.push(c),
                c => {
                    let mut buf = [0u8; 4];
                    let encoded = c.encode_utf8(&mut buf);
                    for b in encoded.bytes() {
                        result.push_str(&format!("\", b {}, b \"", b));
                    }
                }
            }
        }
        result
    }

    fn register_string(&mut self, value: &str) -> String {
        if let Some(existing) = self.string_constants.get(value) {
            return existing.clone();
        }
        let name = format!("$str.{}", self.string_counter);
        self.string_counter += 1;
        self.string_constants
            .insert(value.to_string(), name.clone());
        let escaped = Self::escape_string_for_qbe(value);
        self.emit_data(&format!("data {} = {{ b \"{}\", b 0 }}", name, escaped));
        name
    }

    // --- Float Handling ---
    fn float_to_qbe(value: f64) -> String {
        // QBE uses fscanf %lf (decimal), not hex encoding
        // Use enough precision to round-trip exactly
        format!("d_{:.17e}", value)
    }

    fn float_to_single_qbe(value: f32) -> String {
        // QBE uses fscanf %f (decimal), not hex encoding
        format!("s_{:.9e}", value)
    }

    // --- Float Detection ---
    fn is_float_operand(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::NumberLiteral {
                literal_type: Some(ty),
                ..
            } => ty.is_float(),
            ExprKind::NumberLiteral {
                value,
                literal_type: None,
            } => value.contains('.'),
            ExprKind::Identifier { name } => {
                name == "PI"
                    || name == "E"
                    || matches!(self.variable_types.get(name.as_str()), Some(t) if t.is_float())
            }
            ExprKind::MemberExpression { object, property } => {
                if let ExprKind::Identifier { name } = &object.kind {
                    if let Some(Type::Struct { name: sname, .. }) =
                        self.variable_types.get(name.as_str())
                    {
                        if let Some(fields) = self.struct_fields.get(sname.as_str()) {
                            return fields.iter().any(|(n, t)| n == property && t.is_float());
                        }
                    }
                }
                false
            }
            ExprKind::IndexExpression { object, .. } => {
                if let ExprKind::Identifier { name } = &object.kind {
                    if let Some(Type::Array { element_type, .. }) =
                        self.variable_types.get(name.as_str())
                    {
                        return element_type.is_float();
                    }
                }
                false
            }
            ExprKind::BinaryExpression { left, right, .. } => {
                self.is_float_operand(left) || self.is_float_operand(right)
            }
            ExprKind::UnaryExpression { operand, .. } => self.is_float_operand(operand),
            ExprKind::CallExpression { callee, .. } => {
                if let ExprKind::Identifier { name } = &callee.kind {
                    let math_funcs: HashSet<&str> = [
                        "sqrt", "sin", "cos", "tan", "asin", "acos", "atan2", "exp", "log", "log2",
                        "floor", "ceil", "round", "fabs", "pow", "fmin", "fmax",
                    ]
                    .iter()
                    .copied()
                    .collect();
                    if math_funcs.contains(name.as_str()) {
                        return true;
                    }
                    if let Some(rt) = self.function_types.get(name.as_str()) {
                        return rt.is_float();
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn is_string_producing_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::StringLiteral { .. } | ExprKind::TemplateLiteral { .. } => true,
            ExprKind::Identifier { name } => match self.variable_types.get(name.as_str()) {
                Some(Type::String) => true,
                Some(Type::Array { element_type, .. }) => {
                    matches!(
                        element_type.as_ref(),
                        Type::Unsigned(crate::types::UnsignedKind::U8)
                    )
                }
                _ => false,
            },
            ExprKind::CallExpression { callee, .. } => {
                if let ExprKind::Identifier { name } = &callee.kind {
                    matches!(
                        name.as_str(),
                        "str"
                            | "i64_to_string"
                            | "u64_to_string"
                            | "f64_to_string"
                            | "input_string"
                    )
                } else if let ExprKind::MemberExpression { object, property } = &callee.kind {
                    // String method calls like s.upper(), s.lower() produce strings
                    matches!(property.as_str(), "upper" | "lower" | "trim" | "replace")
                        && self.is_string_producing_expr(object)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn is_array_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::ArrayLiteral { .. } | ExprKind::ArrayFill { .. } => true,
            ExprKind::Identifier { name } => {
                matches!(
                    self.variable_types.get(name.as_str()),
                    Some(Type::Array { .. })
                )
            }
            _ => false,
        }
    }

    fn is_map_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::MapLiteral { .. } => true,
            ExprKind::Identifier { name } => {
                matches!(
                    self.variable_types.get(name.as_str()),
                    Some(Type::Map { .. })
                )
            }
            _ => false,
        }
    }

    fn is_tensor_producing_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::TensorConstruction { .. } => true,
            ExprKind::Identifier { name } => {
                matches!(
                    self.variable_types.get(name.as_str()),
                    Some(Type::Tensor { .. })
                )
            }
            ExprKind::CallExpression { callee, .. } => {
                // Check for tensor-producing functions
                if let ExprKind::Identifier { name } = &callee.kind {
                    matches!(
                        name.as_str(),
                        "tensor_zeros"
                            | "tensor_ones"
                            | "tensor_randn"
                            | "tensor_add"
                            | "tensor_sub"
                            | "tensor_mul"
                            | "tensor_div"
                            | "tensor_transpose"
                            | "tensor_relu"
                            | "tensor_sigmoid"
                            | "tensor_softmax"
                            | "matmul"
                    )
                } else {
                    false
                }
            }
            ExprKind::BinaryExpression { left, right, .. } => {
                self.is_tensor_producing_expr(left) || self.is_tensor_producing_expr(right)
            }
            _ => false,
        }
    }

    fn _infer_expression_type(&self, expr: &Expr) -> &'static str {
        if self.is_string_producing_expr(expr) {
            "string"
        } else if self.is_float_operand(expr) {
            "float"
        } else {
            "int"
        }
    }

    fn get_identifier_name(expr: &Expr) -> Option<&str> {
        if let ExprKind::Identifier { name } = &expr.kind {
            Some(name.as_str())
        } else {
            None
        }
    }

    /// Look up the return type of a capability call expression.
    /// Handles both `cap.method(...)` and `await cap.method(...)`.
    /// Check if a capability function is declared as async.
    fn is_async_capability_call(&self, expr: &Expr) -> bool {
        if let ExprKind::CallExpression { callee, .. } = &expr.kind {
            if let ExprKind::MemberExpression { object, property } = &callee.kind {
                if let ExprKind::Identifier { name } = &object.kind {
                    if let Some(decl) = self.capability_decls.get(name.as_str()) {
                        for func in &decl.functions {
                            if func.name == *property {
                                return func.is_async;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if an expression is a capability call (object.method(...) where object is a capability).
    fn is_capability_call(&self, expr: &Expr) -> bool {
        if let ExprKind::CallExpression { callee, .. } = &expr.kind {
            if let ExprKind::MemberExpression { object, .. } = &callee.kind {
                if let ExprKind::Identifier { name } = &object.kind {
                    return self.capability_decls.contains_key(name.as_str());
                }
            }
        }
        false
    }

    fn get_capability_return_type(&self, expr: &Expr) -> Option<Type> {
        let call_expr = match &expr.kind {
            ExprKind::AwaitExpression { argument } => argument.as_ref(),
            _ => expr,
        };
        if let ExprKind::CallExpression { callee, .. } = &call_expr.kind {
            if let ExprKind::MemberExpression { object, property } = &callee.kind {
                if let ExprKind::Identifier { name } = &object.kind {
                    if let Some(decl) = self.capability_decls.get(name.as_str()) {
                        for func in &decl.functions {
                            if func.name == *property {
                                return Some(func.return_type.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    // ============================================================
    // EXPRESSION GENERATION
    // ============================================================
    fn gen_expr(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::NumberLiteral {
                value,
                literal_type,
            } => self.gen_number_literal(value, literal_type.as_ref()),
            ExprKind::BooleanLiteral { value } => {
                if *value {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            ExprKind::NoneExpression => {
                let r = self.fresh_reg();
                self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
                self.emit(&format!("    storel 1, {}", r)); // tag=1 for None
                r
            }
            ExprKind::StringLiteral { value } => self.gen_string_literal(value),
            ExprKind::TemplateLiteral { parts } => {
                let parts = parts.clone();
                self.gen_template_literal(&parts)
            }
            ExprKind::Identifier { name } => self.gen_identifier(name),
            ExprKind::BinaryExpression {
                operator,
                left,
                right,
            } => {
                let op = operator.clone();
                let is_float = self.is_float_operand(left) || self.is_float_operand(right);
                let is_str_left = self.is_string_producing_expr(left);
                let is_str_right = self.is_string_producing_expr(right);
                self.gen_binary_expr(&op, left, right, is_float, is_str_left, is_str_right)
            }
            ExprKind::UnaryExpression { operator, operand } => {
                let op = operator.clone();
                let is_float = self.is_float_operand(operand);
                self.gen_unary_expr(&op, operand, is_float)
            }
            ExprKind::CallExpression {
                callee,
                arguments,
                named_args,
            } => {
                let args = arguments.clone();
                let na = named_args.clone();
                self.gen_call_expr(callee, &args, &na)
            }
            ExprKind::AssignmentExpression {
                name,
                target,
                value,
            } => {
                let n = name.clone();
                let t = target.clone();
                self.gen_assignment_expr(&n, &t, value)
            }
            ExprKind::ArrayLiteral { elements } => {
                let elems = elements.clone();
                self.gen_array_literal(&elems)
            }
            ExprKind::ArrayFill { value, count } => self.gen_array_fill(value, count),
            ExprKind::MapLiteral { entries } => {
                let ents = entries.clone();
                self.gen_map_literal(&ents)
            }
            ExprKind::StructLiteral {
                struct_name,
                fields,
            } => {
                let sn = struct_name.clone();
                let fs = fields.clone();
                self.gen_struct_literal(&sn, &fs)
            }
            ExprKind::SoAConstructor {
                struct_name,
                capacity,
            } => {
                let sn = struct_name.clone();
                self.gen_soa_constructor(&sn, *capacity)
            }
            ExprKind::MemberExpression { object, property } => {
                let prop = property.clone();
                self.gen_member_expr(object, &prop)
            }
            ExprKind::IndexExpression { object, index } => self.gen_index_expr(object, index),
            ExprKind::SliceExpression {
                object, start, end, ..
            } => self.gen_slice_expr(object, start, end),
            ExprKind::CastExpression {
                target_type,
                value,
                source_type,
            } => {
                let tt = target_type.clone();
                let st = source_type.clone();
                self.gen_cast_expr(&tt, value, &st)
            }
            ExprKind::IfExpression {
                condition,
                true_branch,
                false_branch,
            } => {
                let tb = true_branch.clone();
                let fb = false_branch.clone();
                self.gen_if_expr(condition, &tb, &fb)
            }
            ExprKind::MatchExpression { subject, arms } => {
                let a = arms.clone();
                self.gen_match_expr(subject, &a)
            }
            ExprKind::Lambda {
                params,
                return_type,
                body,
            } => {
                let p = params.clone();
                let rt = return_type.clone();
                let b = body.clone();
                self.gen_lambda(&p, &rt, &b)
            }
            ExprKind::OkExpression { value } => self.gen_ok_expr(value),
            ExprKind::ErrExpression { value } => self.gen_err_expr(value),
            ExprKind::SomeExpression { value } => self.gen_some_expr(value),
            ExprKind::PropagateExpression { value } => self.gen_propagate_expr(value),
            ExprKind::IsSomeExpression { value, binding } => {
                self.gen_is_some_expr(value, Some(binding))
            }
            ExprKind::IsNoneExpression { value } => self.gen_is_none_expr(value),
            ExprKind::IsOkExpression { value, binding } => {
                self.gen_is_ok_expr(value, Some(binding))
            }
            ExprKind::IsErrExpression { value, binding } => {
                self.gen_is_err_expr(value, Some(binding))
            }
            ExprKind::AwaitExpression { argument } => self.gen_await_expr(argument),
            ExprKind::SpawnExpression { argument } => self.gen_spawn_expr(argument),
            ExprKind::TensorConstruction { args, .. } => {
                let a = args.clone();
                self.gen_tensor_construction(&a)
            }
            ExprKind::BlockExpression { block } => {
                let b = block.clone();
                self.gen_block_stmts(&b.statements);
                "0".to_string()
            }
            _ => "0".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Expression generators
    // -----------------------------------------------------------------------

    fn gen_template_literal(&mut self, parts: &[TemplatePart]) -> String {
        if parts.is_empty() {
            return self.register_string("");
        }
        let mut result: Option<String> = None;
        for part in parts {
            let part_reg = match part {
                TemplatePart::String(s) => self.register_string(s),
                TemplatePart::Expr(e) => {
                    let val = self.gen_expr(e);
                    if self.is_string_producing_expr(e) {
                        val
                    } else if self.is_float_operand(e) || self.float_regs.contains(&val) {
                        let r = self.fresh_reg();
                        self.emit(&format!("    {} =l call $f64_to_string(d {})", r, val));
                        r
                    } else {
                        let r = self.fresh_reg();
                        self.emit(&format!("    {} =l call $i64_to_string(l {})", r, val));
                        r
                    }
                }
            };
            result = Some(match result {
                None => part_reg,
                Some(prev) => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l call $string_concat(l {}, l {})",
                        r, prev, part_reg
                    ));
                    r
                }
            });
        }
        result.unwrap_or_else(|| self.register_string(""))
    }

    fn gen_assignment_expr(
        &mut self,
        name: &Option<String>,
        target: &Option<Box<Expr>>,
        value: &Expr,
    ) -> String {
        let val = self.gen_expr(value);
        let val_store_op = self.store_op_for_expr(value);
        if let Some(n) = name {
            // Simple variable assignment or walrus declaration
            if let Some(reg) = self.variable_registers.get(n).cloned() {
                self.emit(&format!("    {} {}, {}", val_store_op, val, reg));
            } else {
                // New variable declaration via :=
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                self.emit(&format!("    {} {}, {}", val_store_op, val, slot));
                self.variable_registers.insert(n.clone(), slot.clone());
                // Infer type
                if self.is_string_producing_expr(value) {
                    self.variable_types.insert(n.clone(), Type::String);
                } else if self.is_float_operand(value) {
                    self.variable_types
                        .insert(n.clone(), Type::Float(FloatKind::F64));
                } else if self.is_array_expr(value) {
                    self.variable_types.insert(
                        n.clone(),
                        Type::Array {
                            element_type: Box::new(Type::Integer(IntegerKind::I64)),
                            dimensions: vec![],
                        },
                    );
                } else if self.is_map_expr(value) {
                    self.variable_types.insert(
                        n.clone(),
                        Type::Map {
                            key_type: Box::new(Type::String),
                            value_type: Box::new(Type::Integer(IntegerKind::I64)),
                        },
                    );
                } else if let Some(cap_ret) = self.get_capability_return_type(value) {
                    self.variable_types.insert(n.clone(), cap_ret);
                } else if matches!(&value.kind, ExprKind::Lambda { .. }) {
                    // Closures are stored as pointers
                    self.variable_types
                        .insert(n.clone(), Type::Pointer(Some(Box::new(Type::Void))));
                } else if self.is_tensor_producing_expr(value) {
                    self.variable_types.insert(
                        n.clone(),
                        Type::Tensor {
                            dtype: Box::new(Type::Float(FloatKind::F32)),
                            shape: None,
                        },
                    );
                } else if let ExprKind::SoAConstructor {
                    struct_name,
                    capacity,
                } = &value.kind
                {
                    self.variable_types.insert(
                        n.clone(),
                        Type::SOA {
                            struct_type: Box::new(Type::Custom(struct_name.clone())),
                            capacity: Some(capacity.unwrap_or(0)),
                        },
                    );
                }
            }
            val
        } else if let Some(tgt) = target {
            match &tgt.kind {
                ExprKind::IndexExpression { object, index } => {
                    let obj = self.gen_expr(object);
                    let idx = self.gen_expr(index);
                    self.emit(&format!(
                        "    call $array_set(l {}, l {}, l {})",
                        obj, idx, val
                    ));
                    val
                }
                ExprKind::MemberExpression { object, property } => {
                    // SoA transposed field write: soa_var[i].field = value
                    if let ExprKind::IndexExpression {
                        object: inner_obj,
                        index,
                    } = &object.kind
                    {
                        if let ExprKind::Identifier { name } = &inner_obj.kind {
                            if let Some(Type::SOA { struct_type, .. }) =
                                self.variable_types.get(name.as_str())
                            {
                                let soa_struct_name = if let Type::Custom(s) = struct_type.as_ref()
                                {
                                    s.clone()
                                } else {
                                    String::new()
                                };
                                let field_idx = self
                                    .struct_fields
                                    .get(soa_struct_name.as_str())
                                    .and_then(|f| f.iter().position(|(n, _)| n == property))
                                    .unwrap_or(0);
                                let soa = self.gen_expr(inner_obj);
                                let idx_val = self.gen_expr(index);
                                let header_off = (field_idx + 1) * 8;
                                let field_ptr_ptr = self.fresh_reg();
                                self.emit(&format!(
                                    "    {} =l add {}, {}",
                                    field_ptr_ptr, soa, header_off
                                ));
                                let field_arr = self.fresh_reg();
                                self.emit(&format!("    {} =l loadl {}", field_arr, field_ptr_ptr));
                                let elem_off = self.fresh_reg();
                                self.emit(&format!("    {} =l mul {}, 8", elem_off, idx_val));
                                let elem_ptr = self.fresh_reg();
                                self.emit(&format!(
                                    "    {} =l add {}, {}",
                                    elem_ptr, field_arr, elem_off
                                ));
                                self.emit(&format!("    storel {}, {}", val, elem_ptr));
                                return val;
                            }
                        }
                    }
                    let obj = self.gen_expr(object);
                    if let Some(sname) = self.get_struct_type_for_expr(object) {
                        if let Some(fields) = self.struct_fields.get(&sname).cloned() {
                            if let Some(idx) = fields.iter().position(|(n, _)| n == property) {
                                let field_is_float = fields[idx].1.is_float();
                                let op = if field_is_float { "stored" } else { "storel" };
                                let off = self.fresh_reg();
                                self.emit(&format!("    {} =l add {}, {}", off, obj, idx * 8));
                                self.emit(&format!("    {} {}, {}", op, val, off));
                            }
                        }
                    }
                    val
                }
                _ => val,
            }
        } else {
            val
        }
    }

    fn get_struct_type_for_expr(&self, expr: &Expr) -> Option<String> {
        if let ExprKind::Identifier { name } = &expr.kind {
            if let Some(ty) = self.variable_types.get(name) {
                if let Type::Custom(s) = ty {
                    return Some(s.clone());
                }
            }
        }
        None
    }

    /// Emit array_alloc(element_size=8, dimension_count=1, &[capacity])
    fn emit_array_alloc(&mut self, capacity: &str) -> String {
        let dims_slot = self.fresh_reg();
        self.emit_alloc(&format!("    {} =l alloc8 8", dims_slot));
        self.emit(&format!("    storel {}, {}", capacity, dims_slot));
        let arr = self.fresh_reg();
        self.emit(&format!(
            "    {} =l call $array_alloc(l 8, l 1, l {})",
            arr, dims_slot
        ));
        arr
    }

    fn gen_array_literal(&mut self, elements: &[Expr]) -> String {
        // Allocate empty, then push each element
        let arr = self.emit_array_alloc("0");
        for elem in elements {
            let v = self.gen_expr(elem);
            let v = self.ensure_long(&v);
            self.emit(&format!("    call $array_push(l {}, l {})", arr, v));
        }
        arr
    }

    fn gen_array_fill(&mut self, value: &Expr, count: &Expr) -> String {
        let cnt = self.gen_expr(count);
        let arr = self.emit_array_alloc("0");
        let val = self.gen_expr(value);
        let i_slot = self.fresh_reg();
        self.emit_alloc(&format!("    {} =l alloc8 8", i_slot));
        self.emit(&format!("    storel 0, {}", i_slot));
        let loop_lbl = self.fresh_label();
        let end_lbl = self.fresh_label();
        self.emit(&format!("    jmp {}", loop_lbl));
        self.emit(&format!("{}", loop_lbl));
        let i = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", i, i_slot));
        let cmp = self.fresh_reg();
        self.emit(&format!("    {} =w csltl {}, {}", cmp, i, cnt));
        let body_lbl = self.fresh_label();
        self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
        self.emit(&format!("{}", body_lbl));
        let val = self.ensure_long(&val);
        self.emit(&format!("    call $array_push(l {}, l {})", arr, val));
        let inc = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 1", inc, i));
        self.emit(&format!("    storel {}, {}", inc, i_slot));
        self.emit(&format!("    jmp {}", loop_lbl));
        self.emit(&format!("{}", end_lbl));
        arr
    }

    fn gen_map_literal(&mut self, entries: &[MapEntry]) -> String {
        let m = self.fresh_reg();
        let cap = std::cmp::max(entries.len() * 2, 16);
        self.emit(&format!("    {} =l call $map_new(l {})", m, cap));
        for entry in entries {
            let key_label = self.register_string(&entry.key);
            let key_len = entry.key.len();
            let val = self.gen_expr(&entry.value);
            self.emit(&format!(
                "    call $map_set(l {}, l {}, l {}, l {})",
                m, key_label, key_len, val
            ));
        }
        m
    }

    fn gen_struct_literal(
        &mut self,
        struct_name: &Option<String>,
        fields: &[StructFieldInit],
    ) -> String {
        let size = fields.len() * 8;
        let ptr = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l {})", ptr, size.max(8)));
        for (i, field) in fields.iter().enumerate() {
            let val = self.gen_expr(&field.value);
            let op = self.store_op_for_expr(&field.value);
            if i == 0 {
                self.emit(&format!("    {} {}, {}", op, val, ptr));
            } else {
                let off = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, {}", off, ptr, i * 8));
                self.emit(&format!("    {} {}, {}", op, val, off));
            }
        }
        if let Some(sn) = struct_name {
            // Record struct type for this allocation (done at call site via variable_types)
            let _ = sn;
        }
        ptr
    }

    fn gen_soa_constructor(&mut self, struct_name: &str, capacity: Option<usize>) -> String {
        let cap = capacity.unwrap_or(0) as i64;
        let field_count = self
            .struct_fields
            .get(struct_name)
            .map(|f| f.len())
            .unwrap_or(0);
        // Header: [count, field0_ptr, field1_ptr, ...]
        let header_size = (field_count + 1) * 8;
        let header = self.fresh_reg();
        self.emit(&format!(
            "    {} =l call $gc_alloc(l {})",
            header, header_size
        ));
        // Store count at offset 0
        self.emit(&format!("    storel {}, {}", cap, header));
        // For each field, allocate data array and store pointer in header
        for i in 0..field_count {
            let data_size = self.fresh_reg();
            self.emit(&format!("    {} =l mul {}, 8", data_size, cap));
            let field_arr = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $gc_alloc(l {})",
                field_arr, data_size
            ));
            let offset = (i + 1) * 8;
            let ptr = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, {}", ptr, header, offset));
            self.emit(&format!("    storel {}, {}", field_arr, ptr));
        }
        header
    }

    fn gen_member_expr(&mut self, object: &Expr, property: &str) -> String {
        // SoA transposed field access: soa_var[i].field
        // Pattern: MemberExpression { object: IndexExpression { object: Identifier(soa_var), index }, property: field }
        if let ExprKind::IndexExpression {
            object: inner_obj,
            index,
        } = &object.kind
        {
            if let ExprKind::Identifier { name } = &inner_obj.kind {
                if let Some(Type::SOA { struct_type, .. }) = self.variable_types.get(name.as_str())
                {
                    let soa_struct_name = if let Type::Custom(s) = struct_type.as_ref() {
                        s.clone()
                    } else {
                        String::new()
                    };
                    // Get field index
                    let field_idx = self
                        .struct_fields
                        .get(soa_struct_name.as_str())
                        .and_then(|f| f.iter().position(|(n, _)| n == property))
                        .unwrap_or(0);
                    let soa = self.gen_expr(inner_obj);
                    let idx = self.gen_expr(index);
                    // Load field array pointer from header at offset (field_idx + 1) * 8
                    let header_off = (field_idx + 1) * 8;
                    let field_ptr_ptr = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l add {}, {}",
                        field_ptr_ptr, soa, header_off
                    ));
                    let field_arr = self.fresh_reg();
                    self.emit(&format!("    {} =l loadl {}", field_arr, field_ptr_ptr));
                    // Load element at index
                    let elem_off = self.fresh_reg();
                    self.emit(&format!("    {} =l mul {}, 8", elem_off, idx));
                    let elem_ptr = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l add {}, {}",
                        elem_ptr, field_arr, elem_off
                    ));
                    let result = self.fresh_reg();
                    self.emit(&format!("    {} =l loadl {}", result, elem_ptr));
                    return result;
                }
            }
        }
        // Check for .len/.cap on arrays/strings
        if property == "len" {
            let obj = self.gen_expr(object);
            if self.is_string_producing_expr(object) {
                let r = self.fresh_reg();
                self.emit(&format!("    {} =l call $string_length(l {})", r, obj));
                return r;
            }
            if self.is_map_expr(object) {
                let r = self.fresh_reg();
                self.emit(&format!("    {} =l call $map_size(l {})", r, obj));
                return r;
            }
            // Array .len — use runtime function
            let r = self.fresh_reg();
            self.emit(&format!("    {} =l call $array_length(l {})", r, obj));
            return r;
        }
        if property == "cap" {
            let obj = self.gen_expr(object);
            let off = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, 8", off, obj));
            let r = self.fresh_reg();
            self.emit(&format!("    {} =l loadl {}", r, off));
            return r;
        }
        // Struct field access
        let obj = self.gen_expr(object);
        if let Some(sname) = self.get_struct_type_for_expr(object) {
            if let Some(fields) = self.struct_fields.get(&sname).cloned() {
                if let Some(idx) = fields.iter().position(|(n, _)| n == property) {
                    let field_is_float = fields[idx].1.is_float();
                    let (load_op, qbe_ty) = if field_is_float {
                        ("loadd", "d")
                    } else {
                        ("loadl", "l")
                    };
                    if idx == 0 {
                        let r = self.fresh_reg();
                        self.emit(&format!("    {} ={} {} {}", r, qbe_ty, load_op, obj));
                        if field_is_float {
                            self.mark_float_reg(&r);
                        }
                        return r;
                    }
                    let off = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", off, obj, idx * 8));
                    let r = self.fresh_reg();
                    self.emit(&format!("    {} ={} {} {}", r, qbe_ty, load_op, off));
                    if field_is_float {
                        self.mark_float_reg(&r);
                    }
                    return r;
                }
            }
        }
        // Fallback: treat as struct with unknown type
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", r, obj));
        r
    }

    fn gen_index_expr(&mut self, object: &Expr, index: &Expr) -> String {
        let obj = self.gen_expr(object);
        let idx = self.gen_expr(index);
        let r = self.fresh_reg();
        if self.is_map_expr(object) {
            let klen = self.fresh_reg();
            self.emit(&format!("    {} =l call $string_length(l {})", klen, idx));
            self.emit(&format!(
                "    {} =l call $map_get(l {}, l {}, l {})",
                r, obj, idx, klen
            ));
        } else {
            self.emit(&format!(
                "    {} =l call $array_get(l {}, l {})",
                r, obj, idx
            ));
        }
        r
    }

    fn gen_slice_expr(&mut self, object: &Expr, start: &Expr, end: &Expr) -> String {
        let obj = self.gen_expr(object);
        let s = self.gen_expr(start);
        let e = self.gen_expr(end);
        let r = self.fresh_reg();
        if self.is_string_producing_expr(object) {
            self.emit(&format!(
                "    {} =l call $string_slice(l {}, l {}, l {})",
                r, obj, s, e
            ));
        } else {
            self.emit(&format!(
                "    {} =l call $array_slice(l {}, l {}, l {})",
                r, obj, s, e
            ));
        }
        r
    }

    fn gen_cast_expr(
        &mut self,
        target_type: &Type,
        value: &Expr,
        source_type: &Option<Type>,
    ) -> String {
        let val = self.gen_expr(value);
        let is_src_float = source_type.as_ref().map(|t| t.is_float()).unwrap_or(false)
            || self.is_float_operand(value);
        let is_dst_float = target_type.is_float();
        if is_src_float && !is_dst_float {
            let r = self.fresh_reg();
            self.emit(&format!("    {} =l dtosi {}", r, val));
            r
        } else if !is_src_float && is_dst_float {
            let r = self.fresh_reg();
            self.emit(&format!("    {} =d sltof {}", r, val));
            self.mark_float_reg(&r);
            r
        } else {
            val
        }
    }

    fn gen_if_expr(
        &mut self,
        condition: &Expr,
        true_branch: &Block,
        false_branch: &IfElseBranch,
    ) -> String {
        let cond = self.gen_expr(condition);
        let cond_w = self.fresh_reg();
        self.emit(&format!("    {} =w copy {}", cond_w, cond));
        let result_slot = self.fresh_reg();
        self.emit_alloc(&format!("    {} =l alloc8 8", result_slot));
        let true_lbl = self.fresh_label();
        let false_lbl = self.fresh_label();
        let end_lbl = self.fresh_label();
        self.emit(&format!("    jnz {}, {}, {}", cond_w, true_lbl, false_lbl));
        // true branch
        self.emit(&format!("{}", true_lbl));
        let mut last_val = "0".to_string();
        for stmt in &true_branch.statements {
            last_val = self.gen_stmt_value(stmt);
        }
        let true_op = self.store_op_for_reg(&last_val);
        let is_float_result = true_op == "stored";
        self.emit(&format!("    {} {}, {}", true_op, last_val, result_slot));
        self.emit(&format!("    jmp {}", end_lbl));
        // false branch
        self.emit(&format!("{}", false_lbl));
        let mut false_val = "0".to_string();
        match false_branch {
            IfElseBranch::Block(block) => {
                for stmt in &block.statements {
                    false_val = self.gen_stmt_value(stmt);
                }
            }
            IfElseBranch::IfExpression(expr) => {
                false_val = self.gen_expr(expr);
            }
        }
        let false_op = self.store_op_for_reg(&false_val);
        let is_float_result = is_float_result || false_op == "stored";
        self.emit(&format!("    {} {}, {}", false_op, false_val, result_slot));
        self.emit(&format!("    jmp {}", end_lbl));
        // end
        self.emit(&format!("{}", end_lbl));
        let r = self.fresh_reg();
        if is_float_result {
            self.emit(&format!("    {} =d loadd {}", r, result_slot));
            self.mark_float_reg(&r);
        } else {
            self.emit(&format!("    {} =l loadl {}", r, result_slot));
        }
        r
    }

    fn gen_stmt_value(&mut self, stmt: &Statement) -> String {
        match &stmt.kind {
            StatementKind::ExpressionStatement { expression } => self.gen_expr(expression),
            StatementKind::Return { value } => {
                if let Some(v) = value {
                    self.gen_expr(v)
                } else {
                    "0".to_string()
                }
            }
            _ => {
                self.gen_statement(stmt);
                "0".to_string()
            }
        }
    }

    fn gen_match_expr(&mut self, subject: &Expr, arms: &[MatchArm]) -> String {
        let subj = self.gen_expr(subject);
        let result_slot = self.fresh_reg();
        self.emit_alloc(&format!("    {} =l alloc8 8", result_slot));
        let end_lbl = self.fresh_label();
        let mut match_result_is_float = false;
        for (i, arm) in arms.iter().enumerate() {
            let arm_lbl = self.fresh_label();
            let next_lbl = if i + 1 < arms.len() {
                self.fresh_label()
            } else {
                end_lbl.clone()
            };
            match &arm.pattern {
                MatchPattern::WildcardPattern => {
                    self.emit(&format!("    jmp {}", arm_lbl));
                }
                MatchPattern::LiteralPattern(pat) => {
                    let pat_val = self.gen_expr(pat);
                    if matches!(&pat.kind, ExprKind::StringLiteral { .. }) {
                        let cmp = self.fresh_reg();
                        self.emit(&format!(
                            "    {} =l call $string_eq(l {}, l {})",
                            cmp, subj, pat_val
                        ));
                        let cmp_w = self.fresh_reg();
                        self.emit(&format!("    {} =w copy {}", cmp_w, cmp));
                        self.emit(&format!("    jnz {}, {}, {}", cmp_w, arm_lbl, next_lbl));
                    } else {
                        let cmp = self.fresh_reg();
                        self.emit(&format!("    {} =w ceql {}, {}", cmp, subj, pat_val));
                        self.emit(&format!("    jnz {}, {}, {}", cmp, arm_lbl, next_lbl));
                    }
                }
                MatchPattern::VariantPattern { name, binding } => {
                    // Result/Optional variant match: check tag
                    let tag_reg = self.fresh_reg();
                    self.emit(&format!("    {} =l loadl {}", tag_reg, subj));
                    let expected_tag = match name.as_str() {
                        "ok" | "some" => 0i64,
                        "err" | "none" => 1i64,
                        _ => 0,
                    };
                    let cmp = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =w ceql {}, {}",
                        cmp, tag_reg, expected_tag
                    ));
                    self.emit(&format!("    jnz {}, {}, {}", cmp, arm_lbl, next_lbl));
                    // Bind the payload if requested
                    if let Some(bind_name) = binding {
                        self.emit(&format!("{}", arm_lbl));
                        let pay_off = self.fresh_reg();
                        self.emit(&format!("    {} =l add {}, 8", pay_off, subj));
                        let pay_val = self.fresh_reg();
                        self.emit(&format!("    {} =l loadl {}", pay_val, pay_off));
                        let slot = self.fresh_reg();
                        self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                        self.emit(&format!("    storel {}, {}", pay_val, slot));
                        self.variable_registers.insert(bind_name.clone(), slot);
                        // Type the binding: err bindings are String, ok/some inherit inner type
                        if name == "err" {
                            self.variable_types.insert(bind_name.clone(), Type::String);
                        }
                        let body_val = self.gen_expr(&arm.body);
                        let body_op = self.store_op_for_reg(&body_val);
                        if body_op == "stored" {
                            match_result_is_float = true;
                        }
                        self.emit(&format!("    {} {}, {}", body_op, body_val, result_slot));
                        self.emit(&format!("    jmp {}", end_lbl));
                        if i + 1 < arms.len() {
                            self.emit(&format!("{}", next_lbl));
                        }
                        continue;
                    }
                }
            }
            // arm body
            self.emit(&format!("{}", arm_lbl));
            let body_val = self.gen_expr(&arm.body);
            let body_op = self.store_op_for_reg(&body_val);
            if body_op == "stored" {
                match_result_is_float = true;
            }
            self.emit(&format!("    {} {}, {}", body_op, body_val, result_slot));
            self.emit(&format!("    jmp {}", end_lbl));
            if i + 1 < arms.len() && !matches!(arm.pattern, MatchPattern::WildcardPattern) {
                self.emit(&format!("{}", next_lbl));
            }
        }
        self.emit(&format!("{}", end_lbl));
        let r = self.fresh_reg();
        if match_result_is_float {
            self.emit(&format!("    {} =d loadd {}", r, result_slot));
            self.mark_float_reg(&r);
        } else {
            self.emit(&format!("    {} =l loadl {}", r, result_slot));
        }
        r
    }

    fn gen_lambda(&mut self, params: &[FunctionParam], return_type: &Type, body: &Block) -> String {
        let lambda_name = format!("lambda_{}", self.lambda_counter);
        self.lambda_counter += 1;

        // Find captured variables (must do before saving state)
        let captured = self.find_captured_vars(body, params);

        // Save outer variable types for captures before we swap them out
        let mut capture_types: Vec<(String, Type)> = Vec::new();
        for cap in &captured {
            if let Some(t) = self.variable_types.get(cap) {
                capture_types.push((cap.clone(), t.clone()));
            }
        }

        // Save outer variable registers for closure building after restore
        let outer_var_regs = self.variable_registers.clone();

        // Save all codegen state (same pattern as gen_function_decl)
        let saved_regs = std::mem::take(&mut self.variable_registers);
        let saved_types = std::mem::take(&mut self.variable_types);
        let saved_entry = std::mem::take(&mut self.entry_allocs);
        let saved_func = std::mem::take(&mut self.current_func_lines);
        let saved_ret = self.current_function_return_type.take();
        let saved_term = self.block_terminated;
        let saved_reg_counter = self.reg_counter;
        let saved_label_counter = self.label_counter;
        let saved_float_regs = self.float_regs.clone();
        let saved_in_async = self.in_async_function;
        let saved_coro_frame = self.coro_frame.take();
        let saved_coro_future = self.coro_future.take();

        self.block_terminated = false;
        self.in_async_function = false; // lambdas are not async
        self.reset_counters();
        self.current_function_return_type = Some(return_type.clone());

        // Build QBE parameter signature: env first, then user params
        let mut param_strs = vec!["l %env".to_string()];
        for p in params {
            let pty = self.to_qbe_type(&p.param_type);
            param_strs.push(format!("{} %p.{}", pty, p.name));
        }
        let params_joined = param_strs.join(", ");

        // Allocate stack slot for env
        let env_slot = self.fresh_reg();
        self.emit_alloc(&format!("    {} =l alloc8 8", env_slot));
        self.emit("    storel %env, %s_env");
        // Actually use proper slot name
        self.variable_registers
            .insert("env".to_string(), env_slot.clone());

        // Fix: store env into its slot
        self.current_func_lines.pop(); // remove the wrong storel
        self.emit(&format!("    storel %env, {}", env_slot));

        // Allocate stack slots for params
        for p in params {
            let slot = self.fresh_reg();
            self.emit_alloc(&format!("    {} =l alloc8 8", slot));
            let op = if p.param_type.is_float() {
                "stored"
            } else {
                "storel"
            };
            self.emit(&format!("    {} %p.{}, {}", op, p.name, slot));
            self.variable_registers.insert(p.name.clone(), slot);
            self.variable_types
                .insert(p.name.clone(), p.param_type.clone());
        }

        // Restore captured variables from env pointer
        for (i, cap) in captured.iter().enumerate() {
            let slot = self.fresh_reg();
            self.emit_alloc(&format!("    {} =l alloc8 8", slot));
            self.variable_registers.insert(cap.clone(), slot.clone());

            // Copy type info from outer scope
            for (name, typ) in &capture_types {
                if name == cap {
                    self.variable_types.insert(cap.clone(), typ.clone());
                }
            }

            // Load from env: env_ptr + i*8
            let env_ptr = self.fresh_reg();
            self.emit(&format!("    {} =l loadl {}", env_ptr, env_slot));
            if i == 0 {
                let cap_val = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", cap_val, env_ptr));
                self.emit(&format!("    storel {}, {}", cap_val, slot));
            } else {
                let cap_off = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, {}", cap_off, env_ptr, i * 8));
                let cap_val = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", cap_val, cap_off));
                self.emit(&format!("    storel {}, {}", cap_val, slot));
            }
        }

        // Generate body
        self.emit("    call $gc_push_frame()");
        self.gen_block_stmts(&body.statements);
        if !self.block_terminated {
            self.emit("    call $gc_pop_frame()");
            if *return_type == Type::Void {
                self.emit("    ret");
            } else {
                self.emit("    ret 0");
            }
        }

        // Assemble the lambda function
        let ret = self.ret_type_str(return_type);
        let mut func_ir = format!("function {} ${}({}) {{\n", ret, lambda_name, params_joined);
        func_ir.push_str("@start\n");
        for alloc in &self.entry_allocs {
            func_ir.push_str(alloc);
            func_ir.push('\n');
        }
        for line in &self.current_func_lines {
            func_ir.push_str(line);
            func_ir.push('\n');
        }
        func_ir.push_str("}\n\n");
        self.lambda_funcs.push_str(&func_ir);

        // Restore outer state
        self.variable_registers = saved_regs;
        self.variable_types = saved_types;
        self.entry_allocs = saved_entry;
        self.current_func_lines = saved_func;
        self.current_function_return_type = saved_ret;
        self.block_terminated = saved_term;
        self.reg_counter = saved_reg_counter;
        self.label_counter = saved_label_counter;
        self.float_regs = saved_float_regs;
        self.in_async_function = saved_in_async;
        self.coro_frame = saved_coro_frame;
        self.coro_future = saved_coro_future;

        // Build closure {fn_ptr, env_ptr} in outer context
        let cls = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", cls));
        self.emit(&format!("    storel ${}, {}", lambda_name, cls));
        if !captured.is_empty() {
            let env = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $gc_alloc(l {})",
                env,
                captured.len() * 8
            ));
            for (i, cap) in captured.iter().enumerate() {
                if let Some(reg) = outer_var_regs.get(cap).cloned() {
                    let cap_is_float =
                        matches!(self.variable_types.get(cap), Some(t) if t.is_float());
                    let val = self.fresh_reg();
                    if cap_is_float {
                        self.emit(&format!("    {} =d loadd {}", val, reg));
                        self.mark_float_reg(&val);
                    } else {
                        self.emit(&format!("    {} =l loadl {}", val, reg));
                    }
                    let op = if cap_is_float { "stored" } else { "storel" };
                    if i == 0 {
                        self.emit(&format!("    {} {}, {}", op, val, env));
                    } else {
                        let off = self.fresh_reg();
                        self.emit(&format!("    {} =l add {}, {}", off, env, i * 8));
                        self.emit(&format!("    {} {}, {}", op, val, off));
                    }
                }
            }
            let ep = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, 8", ep, cls));
            self.emit(&format!("    storel {}, {}", env, ep));
        } else {
            // No captures: store null env
            let ep = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, 8", ep, cls));
            self.emit(&format!("    storel 0, {}", ep));
        }
        cls
    }

    fn find_captured_vars(&self, block: &Block, params: &[FunctionParam]) -> Vec<String> {
        let param_names: HashSet<_> = params.iter().map(|p| p.name.clone()).collect();
        let mut referenced = HashSet::new();
        self.collect_identifiers_from_block(block, &mut referenced);
        let mut captured = Vec::new();
        for name in &referenced {
            if !param_names.contains(name) && self.variable_registers.contains_key(name) {
                captured.push(name.clone());
            }
        }
        captured.sort(); // deterministic ordering
        captured
    }

    fn collect_identifiers_from_block(&self, block: &Block, ids: &mut HashSet<String>) {
        for stmt in &block.statements {
            self.collect_identifiers_from_stmt(stmt, ids);
        }
    }

    fn collect_identifiers_from_stmt(&self, stmt: &Statement, ids: &mut HashSet<String>) {
        match &stmt.kind {
            StatementKind::VariableDeclaration { value, .. } => {
                if let Some(init) = value {
                    self.collect_identifiers_from_expr(init, ids);
                }
            }
            StatementKind::ExpressionStatement { expression } => {
                self.collect_identifiers_from_expr(expression, ids);
            }
            StatementKind::Return { value } => {
                if let Some(val) = value {
                    self.collect_identifiers_from_expr(val, ids);
                }
            }
            StatementKind::Conditional {
                condition,
                true_branch,
                false_branch,
            } => {
                self.collect_identifiers_from_expr(condition, ids);
                self.collect_identifiers_from_block(true_branch, ids);
                if let Some(alt) = false_branch {
                    self.collect_identifiers_from_block(alt, ids);
                }
            }
            StatementKind::WhileLoop { test, body } => {
                self.collect_identifiers_from_expr(test, ids);
                self.collect_identifiers_from_block(body, ids);
            }
            StatementKind::ForLoop {
                start, end, body, ..
            } => {
                self.collect_identifiers_from_expr(start, ids);
                self.collect_identifiers_from_expr(end, ids);
                self.collect_identifiers_from_block(body, ids);
            }
            StatementKind::ForEachLoop { array, body, .. } => {
                self.collect_identifiers_from_expr(array, ids);
                self.collect_identifiers_from_block(body, ids);
            }
            _ => {}
        }
    }

    fn collect_identifiers_from_expr(&self, expr: &Expr, ids: &mut HashSet<String>) {
        match &expr.kind {
            ExprKind::Identifier { name } => {
                ids.insert(name.clone());
            }
            ExprKind::BinaryExpression { left, right, .. } => {
                self.collect_identifiers_from_expr(left, ids);
                self.collect_identifiers_from_expr(right, ids);
            }
            ExprKind::UnaryExpression { operand, .. } => {
                self.collect_identifiers_from_expr(operand, ids);
            }
            ExprKind::CallExpression {
                callee, arguments, ..
            } => {
                self.collect_identifiers_from_expr(callee, ids);
                for arg in arguments {
                    self.collect_identifiers_from_expr(arg, ids);
                }
            }
            ExprKind::MemberExpression { object, .. } => {
                self.collect_identifiers_from_expr(object, ids);
            }
            ExprKind::IndexExpression { object, index } => {
                self.collect_identifiers_from_expr(object, ids);
                self.collect_identifiers_from_expr(index, ids);
            }
            ExprKind::AssignmentExpression { target, value, .. } => {
                if let Some(t) = target {
                    self.collect_identifiers_from_expr(t, ids);
                }
                self.collect_identifiers_from_expr(value, ids);
            }
            ExprKind::IfExpression {
                condition,
                true_branch,
                false_branch,
            } => {
                self.collect_identifiers_from_expr(condition, ids);
                self.collect_identifiers_from_block(true_branch, ids);
                match false_branch {
                    IfElseBranch::Block(b) => self.collect_identifiers_from_block(b, ids),
                    IfElseBranch::IfExpression(e) => self.collect_identifiers_from_expr(e, ids),
                }
            }
            ExprKind::BlockExpression { block } => {
                self.collect_identifiers_from_block(block, ids);
            }
            ExprKind::MatchExpression { subject, arms } => {
                self.collect_identifiers_from_expr(subject, ids);
                for arm in arms {
                    self.collect_identifiers_from_expr(&arm.body, ids);
                }
            }
            ExprKind::TemplateLiteral { parts } => {
                for part in parts {
                    if let TemplatePart::Expr(e) = part {
                        self.collect_identifiers_from_expr(e, ids);
                    }
                }
            }
            ExprKind::ArrayLiteral { elements } => {
                for e in elements {
                    self.collect_identifiers_from_expr(e, ids);
                }
            }
            ExprKind::MapLiteral { entries } => {
                for e in entries {
                    self.collect_identifiers_from_expr(&e.value, ids);
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for f in fields {
                    self.collect_identifiers_from_expr(&f.value, ids);
                }
            }
            ExprKind::OkExpression { value }
            | ExprKind::ErrExpression { value }
            | ExprKind::SomeExpression { value }
            | ExprKind::PropagateExpression { value }
            | ExprKind::IsSomeExpression { value, .. }
            | ExprKind::IsNoneExpression { value }
            | ExprKind::IsOkExpression { value, .. }
            | ExprKind::IsErrExpression { value, .. }
            | ExprKind::AwaitExpression { argument: value }
            | ExprKind::SpawnExpression { argument: value }
            | ExprKind::CastExpression { value, .. } => {
                self.collect_identifiers_from_expr(value, ids);
            }
            ExprKind::Lambda { body, .. } => {
                self.collect_identifiers_from_block(body, ids);
            }
            _ => {}
        }
    }

    fn gen_ok_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let op = self.store_op_for_expr(value);
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
        self.emit(&format!("    storel 0, {}", r)); // tag=0 for Ok
        let pay = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", pay, r));
        self.emit(&format!("    {} {}, {}", op, val, pay));
        r
    }

    fn gen_err_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let op = self.store_op_for_expr(value);
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
        self.emit(&format!("    storel 1, {}", r)); // tag=1 for Err
        let pay = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", pay, r));
        self.emit(&format!("    {} {}, {}", op, val, pay));
        r
    }

    fn gen_some_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let op = self.store_op_for_expr(value);
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
        self.emit(&format!("    storel 0, {}", r)); // tag=0 for Some
        let pay = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", pay, r));
        self.emit(&format!("    {} {}, {}", op, val, pay));
        r
    }

    fn _gen_none_expr(&mut self) -> String {
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
        self.emit(&format!("    storel 1, {}", r)); // tag=1 for None
        r
    }

    fn gen_propagate_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let is_err = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 1", is_err, tag));
        let ok_lbl = self.fresh_label();
        let err_lbl = self.fresh_label();
        self.emit(&format!("    jnz {}, {}, {}", is_err, err_lbl, ok_lbl));
        // Error path: return the error
        self.emit(&format!("{}", err_lbl));
        if let Some(catch_lbl) = self.try_stack.last().cloned() {
            self.emit(&format!("    jmp {}", catch_lbl));
        } else {
            self.emit(&format!("    ret {}", val));
        }
        // Ok path: extract payload
        self.emit(&format!("{}", ok_lbl));
        let pay_off = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", pay_off, val));
        let payload = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", payload, pay_off));
        payload
    }

    fn gen_is_some_expr(&mut self, value: &Expr, binding: Option<&String>) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 0", cmp_w, tag)); // tag==0 means Some
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        // If there's a binding, extract the payload
        if let Some(name) = binding {
            if !name.is_empty() {
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                let pay_ptr = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 8", pay_ptr, val));
                let pay = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", pay, pay_ptr));
                self.emit(&format!("    storel {}, {}", pay, slot));
                self.variable_registers.insert(name.clone(), slot);
            }
        }
        result
    }

    fn gen_is_none_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 1", cmp_w, tag)); // tag==1 means None
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        result
    }

    fn gen_is_ok_expr(&mut self, value: &Expr, binding: Option<&String>) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 0", cmp_w, tag)); // tag==0 means Ok
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        if let Some(name) = binding {
            if !name.is_empty() {
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                let pay_ptr = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 8", pay_ptr, val));
                let pay = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", pay, pay_ptr));
                self.emit(&format!("    storel {}, {}", pay, slot));
                self.variable_registers.insert(name.clone(), slot);
            }
        }
        result
    }

    fn gen_is_err_expr(&mut self, value: &Expr, binding: Option<&String>) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 1", cmp_w, tag)); // tag==1 means Err
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        if let Some(name) = binding {
            if !name.is_empty() {
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                let pay_ptr = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 8", pay_ptr, val));
                let pay = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", pay, pay_ptr));
                self.emit(&format!("    storel {}, {}", pay, slot));
                self.variable_registers.insert(name.clone(), slot);
                self.variable_types.insert(name.clone(), Type::String);
            }
        }
        result
    }

    fn gen_await_expr(&mut self, argument: &Expr) -> String {
        if !self.in_async_function {
            // Not in async context: just evaluate the argument
            return self.gen_expr(argument);
        }

        // Check if this is a sync capability call — need to wrap in completed future
        let is_sync_cap =
            self.is_capability_call(argument) && !self.is_async_capability_call(argument);
        let future_val = if is_sync_cap {
            // Sync capability: call returns raw value, wrap in immediately-completed future
            let cap_result = self.gen_expr(argument);
            let new_future = self.fresh_reg();
            self.emit(&format!("    {} =l call $mog_future_new()", new_future));
            self.emit(&format!(
                "    call $mog_future_complete(l {}, l {})",
                new_future, cap_result
            ));
            new_future
        } else {
            // Async capability or regular async call: result IS a future pointer
            self.gen_expr(argument)
        };

        let await_idx = self.await_counter;
        self.await_counter += 1;
        let cont_lbl = format!("@await{}.cont", await_idx);
        self.coro_resume_blocks.push(cont_lbl.clone());
        // Spill all locals to frame
        if let Some(frame) = self.coro_frame.clone() {
            let mut offset = CORO_HEADER_SIZE;
            let spilled_vars = self.coro_spilled_vars.clone();
            for var_name in &spilled_vars {
                if let Some(reg) = self.variable_registers.get(var_name).cloned() {
                    let var_is_float =
                        matches!(self.variable_types.get(var_name), Some(t) if t.is_float());
                    let val = self.fresh_reg();
                    if var_is_float {
                        self.emit(&format!("    {} =d loadd {}", val, reg));
                    } else {
                        self.emit(&format!("    {} =l loadl {}", val, reg));
                    }
                    let off = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", off, frame, offset));
                    let op = if var_is_float { "stored" } else { "storel" };
                    self.emit(&format!("    {} {}, {}", op, val, off));
                }
                offset += 8;
            }
            // Store awaited future
            let foff = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, {}", foff, frame, offset));
            self.emit(&format!("    storel {}, {}", future_val, foff));
            // Set state index
            let state_off = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, 8", state_off, frame));
            self.emit(&format!("    storel {}, {}", await_idx, state_off));
            // Call mog_await
            self.emit(&format!(
                "    call $mog_await(l {}, l {})",
                future_val, frame
            ));
            // Return (suspend)
            self.emit("    call $gc_pop_frame()");
            self.emit("    ret");
            // Continuation label
            self.emit(&format!("{}", cont_lbl));
            // Reload spilled vars
            let mut reload_offset = CORO_HEADER_SIZE;
            for var_name in &spilled_vars {
                if let Some(reg) = self.variable_registers.get(var_name).cloned() {
                    let var_is_float =
                        matches!(self.variable_types.get(var_name), Some(t) if t.is_float());
                    let roff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", roff, frame, reload_offset));
                    let rval = self.fresh_reg();
                    if var_is_float {
                        self.emit(&format!("    {} =d loadd {}", rval, roff));
                    } else {
                        self.emit(&format!("    {} =l loadl {}", rval, roff));
                    }
                    let op = if var_is_float { "stored" } else { "storel" };
                    self.emit(&format!("    {} {}, {}", op, rval, reg));
                }
                reload_offset += 8;
            }
            // Get result from future
            let foff2 = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, {}", foff2, frame, offset));
            let awaited = self.fresh_reg();
            self.emit(&format!("    {} =l loadl {}", awaited, foff2));
            let result = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $mog_future_get_result(l {})",
                result, awaited
            ));
            result
        } else {
            future_val
        }
    }

    fn gen_spawn_expr(&mut self, argument: &Expr) -> String {
        self.gen_expr(argument)
    }

    fn gen_tensor_construction(&mut self, args: &[Expr]) -> String {
        if args.is_empty() {
            // tensor<f32>() - empty 0-dim tensor
            let shape_slot = self.fresh_reg();
            self.emit_alloc(&format!("    {} =l alloc8 8", shape_slot));
            self.emit(&format!("    storel 0, {}", shape_slot));
            let r = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $tensor_create(l 0, l {}, l 0)",
                r, shape_slot
            ));
            return r;
        }
        // First arg is shape (should be ArrayLiteral)
        let shape_arg = &args[0];
        if let ExprKind::ArrayLiteral { elements } = &shape_arg.kind {
            let ndim = elements.len();
            // Allocate shape array on heap
            let shape_buf = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $gc_alloc(l {})",
                shape_buf,
                ndim * 8
            ));
            for (i, elem) in elements.iter().enumerate() {
                let dim_val = self.gen_expr(elem);
                if i == 0 {
                    self.emit(&format!("    storel {}, {}", dim_val, shape_buf));
                } else {
                    let off = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", off, shape_buf, i * 8));
                    self.emit(&format!("    storel {}, {}", dim_val, off));
                }
            }
            if args.len() >= 2 {
                if let ExprKind::ArrayLiteral {
                    elements: data_elems,
                } = &args[1].kind
                {
                    // tensor<f32>([shape], [data]) — with float data
                    let data_count = data_elems.len();
                    let data_buf = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l call $gc_alloc(l {})",
                        data_buf,
                        data_count * 4
                    )); // float = 4 bytes
                    for (i, elem) in data_elems.iter().enumerate() {
                        let elem_val = self.gen_expr(elem);
                        // Convert to float (may need d->s conversion)
                        let float_val = if self.float_regs.contains(elem_val.as_str()) {
                            let sv = self.fresh_reg();
                            self.emit(&format!("    {} =s truncd {}", sv, elem_val));
                            sv
                        } else {
                            // Integer literal like 1.0 parsed as float
                            let dv = self.fresh_reg();
                            self.emit(&format!("    {} =d sltof {}", dv, elem_val));
                            self.mark_float_reg(&dv);
                            let sv = self.fresh_reg();
                            self.emit(&format!("    {} =s truncd {}", sv, dv));
                            sv
                        };
                        if i == 0 {
                            self.emit(&format!("    stores {}, {}", float_val, data_buf));
                        } else {
                            let off = self.fresh_reg();
                            self.emit(&format!("    {} =l add {}, {}", off, data_buf, i * 4));
                            self.emit(&format!("    stores {}, {}", float_val, off));
                        }
                    }
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l call $tensor_create_with_data(l {}, l {}, l {}, l 0)",
                        r, ndim, shape_buf, data_buf
                    ));
                    return r;
                }
            }
            // tensor<f32>([shape]) — zeros
            let r = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $tensor_create(l {}, l {}, l 0)",
                r, ndim, shape_buf
            ));
            return r;
        }
        // Fallback: pass shape expr directly
        let shape = self.gen_expr(shape_arg);
        let r = self.fresh_reg();
        self.emit(&format!(
            "    {} =l call $tensor_create(l 1, l {}, l 0)",
            r, shape
        ));
        r
    }

    fn gen_capability_call(
        &mut self,
        cap_name: &str,
        method: &str,
        arg_regs: &[String],
        arguments: &[Expr],
    ) -> String {
        let nargs = arg_regs.len();
        let cap_str = self.register_string(cap_name);
        let method_str = self.register_string(method);
        // Get VM pointer
        let vm = self.fresh_reg();
        self.emit(&format!("    {} =l call $mog_vm_get_global()", vm));
        // Allocate output MogValue (24 bytes)
        let out = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 24)", out));
        // Allocate args buffer
        if nargs > 0 {
            let args_buf = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $gc_alloc(l {})",
                args_buf,
                nargs * 24
            ));
            for (i, arg) in arg_regs.iter().enumerate() {
                let base = i * 24;
                // Determine MogValue tag: 0=INT, 1=FLOAT, 3=STRING
                let tag = if i < arguments.len() && self.is_string_producing_expr(&arguments[i]) {
                    3 // MOG_STRING
                } else if i < arguments.len() && self.is_float_operand(&arguments[i]) {
                    1 // MOG_FLOAT
                } else {
                    0 // MOG_INT
                };
                if base == 0 {
                    self.emit(&format!("    storel {}, {}", tag, args_buf));
                    let doff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, 8", doff, args_buf));
                    if tag == 1 {
                        // Float: store as double bits
                        let bits = self.fresh_reg();
                        self.emit(&format!("    {} =l cast {}", bits, arg));
                        self.emit(&format!("    storel {}, {}", bits, doff));
                    } else {
                        self.emit(&format!("    storel {}, {}", arg, doff));
                    }
                } else {
                    let toff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", toff, args_buf, base));
                    self.emit(&format!("    storel {}, {}", tag, toff));
                    let doff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", doff, args_buf, base + 8));
                    if tag == 1 {
                        let bits = self.fresh_reg();
                        self.emit(&format!("    {} =l cast {}", bits, arg));
                        self.emit(&format!("    storel {}, {}", bits, doff));
                    } else {
                        self.emit(&format!("    storel {}, {}", arg, doff));
                    }
                }
            }
            self.emit(&format!(
                "    call $mog_cap_call_out(l {}, l {}, l {}, l {}, l {}, w {})",
                out, vm, cap_str, method_str, args_buf, nargs
            ));
        } else {
            self.emit(&format!(
                "    call $mog_cap_call_out(l {}, l {}, l {}, l {}, l 0, w 0)",
                out, vm, cap_str, method_str
            ));
        }
        // Read result from out+8
        let roff = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", roff, out));
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", result, roff));
        result
    }

    // -----------------------------------------------------------------------
    // Statement generation
    // -----------------------------------------------------------------------

    fn gen_block_stmts(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.gen_statement(stmt);
        }
    }

    fn gen_statement(&mut self, stmt: &Statement) {
        if self.block_terminated {
            return;
        }
        match &stmt.kind {
            StatementKind::ExpressionStatement { expression } => {
                self.gen_expr(expression);
            }
            StatementKind::VariableDeclaration {
                name,
                var_type,
                value,
            } => {
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                if let Some(val) = value {
                    let v = self.gen_expr(val);
                    let op = if var_type.as_ref().map_or(false, |t| t.is_float())
                        || self.is_float_operand(val)
                    {
                        "stored"
                    } else {
                        "storel"
                    };
                    self.emit(&format!("    {} {}, {}", op, v, slot));
                }
                self.variable_registers.insert(name.clone(), slot);
                if let Some(t) = var_type {
                    self.variable_types.insert(name.clone(), t.clone());
                }
            }
            StatementKind::Assignment { name, value } => {
                let val = self.gen_expr(value);
                if let Some(reg) = self.variable_registers.get(name).cloned() {
                    self.emit_store_var(&val, &reg, name);
                }
            }
            StatementKind::Return { value } => {
                if self.in_async_function {
                    if let Some(future) = self.coro_future.clone() {
                        if let Some(v) = value {
                            let val = self.gen_expr(v);
                            self.emit(&format!(
                                "    call $mog_future_complete(l {}, l {})",
                                future, val
                            ));
                        } else {
                            self.emit(&format!("    call $mog_future_complete(l {}, l 0)", future));
                        }
                    }
                    self.emit("    call $gc_pop_frame()");
                    self.emit("    ret");
                } else {
                    self.emit("    call $gc_pop_frame()");
                    if let Some(v) = value {
                        let val = self.gen_expr(v);
                        self.emit(&format!("    ret {}", val));
                    } else {
                        self.emit("    ret 0");
                    }
                }
                self.block_terminated = true;
            }
            StatementKind::Conditional {
                condition,
                true_branch,
                false_branch,
            } => {
                let cond = self.gen_expr(condition);
                let cond_w = self.fresh_reg();
                self.emit(&format!("    {} =w copy {}", cond_w, cond));
                let true_lbl = self.fresh_label();
                let false_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.emit(&format!(
                    "    jnz {}, {}, {}",
                    cond_w,
                    true_lbl,
                    if false_branch.is_some() {
                        &false_lbl
                    } else {
                        &end_lbl
                    }
                ));
                self.emit(&format!("{}", true_lbl));
                let saved = self.block_terminated;
                self.block_terminated = false;
                self.gen_block_stmts(&true_branch.statements);
                if !self.block_terminated {
                    self.emit(&format!("    jmp {}", end_lbl));
                }
                self.block_terminated = saved;
                if let Some(fb) = false_branch {
                    self.emit(&format!("{}", false_lbl));
                    let saved2 = self.block_terminated;
                    self.block_terminated = false;
                    self.gen_block_stmts(&fb.statements);
                    if !self.block_terminated {
                        self.emit(&format!("    jmp {}", end_lbl));
                    }
                    self.block_terminated = saved2;
                }
                self.emit(&format!("{}", end_lbl));
            }
            StatementKind::WhileLoop { test, body } => {
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), test_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let cond = self.gen_expr(test);
                let cond_w = self.fresh_reg();
                self.emit(&format!("    {} =w copy {}", cond_w, cond));
                self.emit(&format!("    jnz {}, {}, {}", cond_w, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                self.gen_block_stmts(&body.statements);
                self.emit_interrupt_check();
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", end_lbl));
                self.loop_stack.pop();
            }
            StatementKind::ForLoop {
                variable,
                start,
                end,
                body,
            } => {
                let s = self.gen_expr(start);
                let e = self.gen_expr(end);
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                self.emit(&format!("    storel {}, {}", s, slot));
                self.variable_registers
                    .insert(variable.clone(), slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let step_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), step_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let i = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", i, slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, i, e));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                self.gen_block_stmts(&body.statements);
                if !self.block_terminated {
                    self.emit(&format!("    jmp {}", step_lbl));
                }
                self.emit(&format!("{}", step_lbl));
                let cur = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", cur, slot));
                let inc = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 1", inc, cur));
                self.emit(&format!("    storel {}, {}", inc, slot));
                self.emit_interrupt_check();
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", end_lbl));
                self.loop_stack.pop();
            }
            StatementKind::ForInRange {
                variable,
                start,
                end,
                body,
            } => {
                // Same as ForLoop
                let s = self.gen_expr(start);
                let e = self.gen_expr(end);
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                self.emit(&format!("    storel {}, {}", s, slot));
                self.variable_registers
                    .insert(variable.clone(), slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let step_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), step_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let i = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", i, slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, i, e));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                self.gen_block_stmts(&body.statements);
                if !self.block_terminated {
                    self.emit(&format!("    jmp {}", step_lbl));
                }
                self.emit(&format!("{}", step_lbl));
                let cur = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", cur, slot));
                let inc = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 1", inc, cur));
                self.emit(&format!("    storel {}, {}", inc, slot));
                self.emit_interrupt_check();
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", end_lbl));
                self.loop_stack.pop();
            }
            StatementKind::ForEachLoop {
                variable,
                var_type: _,
                array,
                body,
            } => {
                let arr = self.gen_expr(array);
                let len = self.fresh_reg();
                self.emit(&format!("    {} =l call $array_length(l {})", len, arr));
                let idx_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", idx_slot));
                self.emit(&format!("    storel 0, {}", idx_slot));
                let val_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", val_slot));
                self.variable_registers
                    .insert(variable.clone(), val_slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let step_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), step_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let idx = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", idx, idx_slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, idx, len));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                let elem = self.fresh_reg();
                self.emit(&format!(
                    "    {} =l call $array_get(l {}, l {})",
                    elem, arr, idx
                ));
                self.emit(&format!("    storel {}, {}", elem, val_slot));
                self.gen_block_stmts(&body.statements);
                if !self.block_terminated {
                    self.emit(&format!("    jmp {}", step_lbl));
                }
                self.emit(&format!("{}", step_lbl));
                let cur_idx = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", cur_idx, idx_slot));
                let inc = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 1", inc, cur_idx));
                self.emit(&format!("    storel {}, {}", inc, idx_slot));
                self.emit_interrupt_check();
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", end_lbl));
                self.loop_stack.pop();
            }
            StatementKind::ForInIndex {
                index_variable,
                value_variable,
                iterable,
                body,
            } => {
                let is_map = self.is_map_expr(iterable);
                let arr = self.gen_expr(iterable);
                let len = self.fresh_reg();
                if is_map {
                    self.emit(&format!("    {} =l call $map_size(l {})", len, arr));
                } else {
                    self.emit(&format!("    {} =l call $array_length(l {})", len, arr));
                }
                // For maps: use a hidden counter separate from the key variable
                let counter_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", counter_slot));
                self.emit(&format!("    storel 0, {}", counter_slot));
                let key_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", key_slot));
                self.variable_registers
                    .insert(index_variable.clone(), key_slot.clone());
                if is_map {
                    self.variable_types
                        .insert(index_variable.clone(), Type::String);
                }
                let val_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", val_slot));
                self.variable_registers
                    .insert(value_variable.clone(), val_slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let step_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), step_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let idx = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", idx, counter_slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, idx, len));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                if is_map {
                    // For maps: key_slot = key (string), val_slot = value
                    let key = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l call $map_key_at(l {}, l {})",
                        key, arr, idx
                    ));
                    self.emit(&format!("    storel {}, {}", key, key_slot));
                    let val = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l call $map_value_at(l {}, l {})",
                        val, arr, idx
                    ));
                    self.emit(&format!("    storel {}, {}", val, val_slot));
                } else {
                    // For arrays: key_slot = index (int), val_slot = element
                    self.emit(&format!("    storel {}, {}", idx, key_slot));
                    let elem = self.fresh_reg();
                    self.emit(&format!(
                        "    {} =l call $array_get(l {}, l {})",
                        elem, arr, idx
                    ));
                    self.emit(&format!("    storel {}, {}", elem, val_slot));
                }
                self.gen_block_stmts(&body.statements);
                if !self.block_terminated {
                    self.emit(&format!("    jmp {}", step_lbl));
                }
                self.emit(&format!("{}", step_lbl));
                let cur_idx = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", cur_idx, counter_slot));
                let inc = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 1", inc, cur_idx));
                self.emit(&format!("    storel {}, {}", inc, counter_slot));
                self.emit_interrupt_check();
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", end_lbl));
                self.loop_stack.pop();
            }
            StatementKind::ForInMap {
                key_variable,
                value_variable,
                map,
                body,
            } => {
                let m = self.gen_expr(map);
                let len = self.fresh_reg();
                self.emit(&format!("    {} =l call $map_size(l {})", len, m));
                let idx_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", idx_slot));
                self.emit(&format!("    storel 0, {}", idx_slot));
                let key_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", key_slot));
                self.variable_registers
                    .insert(key_variable.clone(), key_slot.clone());
                self.variable_types
                    .insert(key_variable.clone(), Type::String);
                let val_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", val_slot));
                self.variable_registers
                    .insert(value_variable.clone(), val_slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let step_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), step_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let idx = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", idx, idx_slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, idx, len));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                // Get key and value by index
                let key = self.fresh_reg();
                self.emit(&format!(
                    "    {} =l call $map_key_at(l {}, l {})",
                    key, m, idx
                ));
                self.emit(&format!("    storel {}, {}", key, key_slot));
                let val = self.fresh_reg();
                self.emit(&format!(
                    "    {} =l call $map_value_at(l {}, l {})",
                    val, m, idx
                ));
                self.emit(&format!("    storel {}, {}", val, val_slot));
                self.gen_block_stmts(&body.statements);
                if !self.block_terminated {
                    self.emit(&format!("    jmp {}", step_lbl));
                }
                self.emit(&format!("{}", step_lbl));
                let cur_idx = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", cur_idx, idx_slot));
                let inc = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, 1", inc, cur_idx));
                self.emit(&format!("    storel {}, {}", inc, idx_slot));
                self.emit_interrupt_check();
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", end_lbl));
                self.loop_stack.pop();
            }
            StatementKind::Block { statements, .. } => {
                self.gen_block_stmts(statements);
            }
            StatementKind::Break => {
                if let Some((break_lbl, _)) = self.loop_stack.last().cloned() {
                    self.emit(&format!("    jmp {}", break_lbl));
                    self.block_terminated = true;
                }
            }
            StatementKind::Continue => {
                if let Some((_, cont_lbl)) = self.loop_stack.last().cloned() {
                    self.emit(&format!("    jmp {}", cont_lbl));
                    self.block_terminated = true;
                }
            }
            StatementKind::FunctionDeclaration {
                name,
                params,
                return_type,
                body,
                is_public,
            } => {
                self.gen_function_decl(name, params, return_type, body, *is_public);
            }
            StatementKind::AsyncFunctionDeclaration {
                name,
                params,
                return_type,
                body,
                is_public,
            } => {
                self.gen_async_function_decl(name, params, return_type, body, *is_public);
            }
            StatementKind::StructDefinition { name, fields, .. } => {
                let field_vec: Vec<(String, Type)> = fields
                    .iter()
                    .map(|f| (f.name.clone(), f.field_type.clone()))
                    .collect();
                self.struct_fields.insert(name.clone(), field_vec);
            }
            StatementKind::RequiresDeclaration { capabilities } => {
                for cap in capabilities {
                    self.capabilities.insert(cap.clone());
                }
            }
            StatementKind::OptionalDeclaration { capabilities } => {
                for cap in capabilities {
                    self.capabilities.insert(cap.clone());
                }
            }
            StatementKind::TryCatch {
                try_body,
                error_var,
                catch_body,
            } => {
                let catch_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.try_stack.push(catch_lbl.clone());
                self.gen_block_stmts(&try_body.statements);
                self.try_stack.pop();
                self.emit(&format!("    jmp {}", end_lbl));
                self.emit(&format!("{}", catch_lbl));
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                self.variable_registers.insert(error_var.clone(), slot);
                self.gen_block_stmts(&catch_body.statements);
                self.emit(&format!("{}", end_lbl));
            }
            StatementKind::WithBlock { context, body } => {
                let is_no_grad = match &context.kind {
                    ExprKind::Identifier { name } => name == "no_grad",
                    ExprKind::CallExpression { callee, .. } => {
                        matches!(&callee.kind, ExprKind::Identifier { name } if name == "no_grad")
                    }
                    _ => false,
                };
                if is_no_grad {
                    self.emit("    call $mog_no_grad_begin()");
                    self.gen_block_stmts(&body.statements);
                    self.emit("    call $mog_no_grad_end()");
                } else {
                    self.gen_block_stmts(&body.statements);
                }
            }
            StatementKind::TypeAliasDeclaration { .. } => { /* no codegen needed */ }
            StatementKind::PackageDeclaration { name } => {
                self.package_prefix = format!("{}__", name);
            }
            StatementKind::ImportDeclaration { paths } => {
                for path in paths {
                    let parts: Vec<&str> = path.split('/').collect();
                    if let Some(last) = parts.last() {
                        self.imported_packages
                            .insert(last.to_string(), path.replace('/', "__"));
                    }
                }
            }
            StatementKind::Program { statements, .. } => {
                self.gen_block_stmts(statements);
            }
        }
    }

    fn emit_interrupt_check(&mut self) {
        let flag = self.fresh_reg();
        self.emit(&format!("    {} =w loadw $mog_interrupt_flag", flag));
        let check_lbl = self.fresh_label();
        let cont_lbl = self.fresh_label();
        self.emit(&format!("    jnz {}, {}, {}", flag, check_lbl, cont_lbl));
        self.emit(&format!("{}", check_lbl));
        self.emit("    call $exit(w 99)");
        self.emit(&format!("{}", cont_lbl));
    }

    // -----------------------------------------------------------------------
    // Function generation
    // -----------------------------------------------------------------------

    fn gen_function_decl(
        &mut self,
        name: &str,
        params: &[FunctionParam],
        return_type: &Type,
        body: &Block,
        is_public: bool,
    ) {
        let saved_regs = std::mem::take(&mut self.variable_registers);
        let saved_types = std::mem::take(&mut self.variable_types);
        let saved_entry = std::mem::take(&mut self.entry_allocs);
        let saved_func = std::mem::take(&mut self.current_func_lines);
        let saved_ret = self.current_function_return_type.take();
        let saved_term = self.block_terminated;
        let saved_reg_counter = self.reg_counter;
        let saved_label_counter = self.label_counter;
        let saved_float_regs = self.float_regs.clone();
        let saved_in_async = self.in_async_function;
        let saved_coro_frame = self.coro_frame.take();
        let saved_coro_future = self.coro_future.take();
        self.block_terminated = false;
        self.in_async_function = false;
        self.reset_counters();
        self.current_function_return_type = Some(return_type.clone());

        let func_name = if name == "main" {
            "program_user".to_string()
        } else {
            format!("{}{}", self.package_prefix, name)
        };
        let ret = self.ret_type_str(return_type);
        let mut param_strs = Vec::new();
        for p in params {
            let pty = self.to_qbe_type(&p.param_type);
            param_strs.push(format!("{} %p.{}", pty, p.name));
        }
        let params_joined = param_strs.join(", ");

        // Emit param allocs and stores
        for p in params {
            let slot = self.fresh_reg();
            self.emit_alloc(&format!("    {} =l alloc8 8", slot));
            let op = if p.param_type.is_float() {
                "stored"
            } else {
                "storel"
            };
            self.emit(&format!("    {} %p.{}, {}", op, p.name, slot));
            self.variable_registers.insert(p.name.clone(), slot);
            self.variable_types
                .insert(p.name.clone(), p.param_type.clone());
        }

        // Register function param info
        let param_info: Vec<(String, Type)> = params
            .iter()
            .map(|p| (p.name.clone(), p.param_type.clone()))
            .collect();
        self.function_param_info
            .insert(func_name.clone(), param_info);
        self.function_types
            .insert(func_name.clone(), return_type.clone());

        self.emit("    call $gc_push_frame()");
        self.gen_block_stmts(&body.statements);
        if !self.block_terminated {
            self.emit("    call $gc_pop_frame()");
            if *return_type == Type::Void {
                self.emit("    ret");
            } else {
                self.emit("    ret 0");
            }
        }

        // Assemble function
        let export = if is_public && self.plugin_mode {
            "export "
        } else {
            ""
        };
        let visibility = if !is_public && self.plugin_mode {
            ""
        } else {
            ""
        };
        let _ = visibility;
        let mut func_ir = format!(
            "{}function {} ${}({}) {{\n",
            export, ret, func_name, params_joined
        );
        func_ir.push_str("@start\n");
        for alloc in &self.entry_allocs {
            func_ir.push_str(alloc);
            func_ir.push('\n');
        }
        for line in &self.current_func_lines {
            func_ir.push_str(line);
            func_ir.push('\n');
        }
        func_ir.push_str("}\n\n");
        self.output.push_str(&func_ir);

        if is_public {
            self.pub_functions
                .push((name.to_string(), params.len(), false));
        }

        // Restore state
        self.variable_registers = saved_regs;
        self.variable_types = saved_types;
        self.entry_allocs = saved_entry;
        self.current_func_lines = saved_func;
        self.current_function_return_type = saved_ret;
        self.block_terminated = saved_term;
        self.reg_counter = saved_reg_counter;
        self.label_counter = saved_label_counter;
        self.float_regs = saved_float_regs;
        self.in_async_function = saved_in_async;
        self.coro_frame = saved_coro_frame;
        self.coro_future = saved_coro_future;
    }

    /// Scan a block's statements recursively for all variable names that need
    /// spilling across await points in a coroutine frame.
    fn collect_declared_vars(stmts: &[Statement], out: &mut Vec<String>) {
        for stmt in stmts {
            match &stmt.kind {
                StatementKind::VariableDeclaration { name, .. } => {
                    if !out.contains(name) {
                        out.push(name.clone());
                    }
                }
                StatementKind::ExpressionStatement { expression } => {
                    if let ExprKind::AssignmentExpression {
                        name: Some(name), ..
                    } = &expression.kind
                    {
                        if !out.contains(name) {
                            out.push(name.clone());
                        }
                    }
                }
                StatementKind::Conditional {
                    true_branch,
                    false_branch,
                    ..
                } => {
                    Self::collect_declared_vars(&true_branch.statements, out);
                    if let Some(fb) = false_branch {
                        Self::collect_declared_vars(&fb.statements, out);
                    }
                }
                StatementKind::WhileLoop { body, .. } => {
                    Self::collect_declared_vars(&body.statements, out);
                }
                StatementKind::ForLoop { variable, body, .. } => {
                    if !out.contains(variable) {
                        out.push(variable.clone());
                    }
                    Self::collect_declared_vars(&body.statements, out);
                }
                StatementKind::ForEachLoop { variable, body, .. } => {
                    if !out.contains(variable) {
                        out.push(variable.clone());
                    }
                    Self::collect_declared_vars(&body.statements, out);
                }
                StatementKind::ForInRange { variable, body, .. } => {
                    if !out.contains(variable) {
                        out.push(variable.clone());
                    }
                    Self::collect_declared_vars(&body.statements, out);
                }
                StatementKind::ForInIndex {
                    index_variable,
                    value_variable,
                    body,
                    ..
                } => {
                    if !out.contains(index_variable) {
                        out.push(index_variable.clone());
                    }
                    if !out.contains(value_variable) {
                        out.push(value_variable.clone());
                    }
                    Self::collect_declared_vars(&body.statements, out);
                }
                StatementKind::ForInMap {
                    key_variable,
                    value_variable,
                    body,
                    ..
                } => {
                    if !out.contains(key_variable) {
                        out.push(key_variable.clone());
                    }
                    if !out.contains(value_variable) {
                        out.push(value_variable.clone());
                    }
                    Self::collect_declared_vars(&body.statements, out);
                }
                StatementKind::TryCatch {
                    try_body,
                    error_var,
                    catch_body,
                } => {
                    Self::collect_declared_vars(&try_body.statements, out);
                    if !out.contains(error_var) {
                        out.push(error_var.clone());
                    }
                    Self::collect_declared_vars(&catch_body.statements, out);
                }
                StatementKind::Block { statements, .. } => {
                    Self::collect_declared_vars(statements, out);
                }
                StatementKind::WithBlock { body, .. } => {
                    Self::collect_declared_vars(&body.statements, out);
                }
                _ => {}
            }
        }
    }

    fn gen_async_function_decl(
        &mut self,
        name: &str,
        params: &[FunctionParam],
        return_type: &Type,
        body: &Block,
        is_public: bool,
    ) {
        self.has_async_functions = true;
        self.async_functions.insert(name.to_string());

        // --- Save all codegen state (same pattern as gen_function_decl) ---
        let saved_regs = std::mem::take(&mut self.variable_registers);
        let saved_types = std::mem::take(&mut self.variable_types);
        let saved_entry = std::mem::take(&mut self.entry_allocs);
        let saved_func = std::mem::take(&mut self.current_func_lines);
        let saved_ret = self.current_function_return_type.take();
        let saved_term = self.block_terminated;
        let saved_reg_counter = self.reg_counter;
        let saved_label_counter = self.label_counter;
        let saved_float_regs = self.float_regs.clone();
        let saved_in_async = self.in_async_function;
        let saved_coro_frame = self.coro_frame.take();
        let saved_coro_future = self.coro_future.take();
        let saved_await_counter = self.await_counter;
        let saved_coro_spilled = std::mem::take(&mut self.coro_spilled_vars);
        let saved_coro_resume = std::mem::take(&mut self.coro_resume_blocks);
        self.block_terminated = false;
        self.reset_counters();

        let func_name = if name == "main" {
            "program_user".to_string()
        } else {
            format!("{}{}", self.package_prefix, name)
        };

        // --- Set up async/coroutine state ---
        self.in_async_function = true;
        self.coro_frame = Some("%frame".to_string());
        self.await_counter = 0;
        self.coro_resume_blocks.clear();
        self.current_function_return_type = Some(return_type.clone());

        // Collect all variables that need spilling
        let mut spilled: Vec<String> = Vec::new();
        for p in params {
            if !spilled.contains(&p.name) {
                spilled.push(p.name.clone());
            }
        }
        Self::collect_declared_vars(&body.statements, &mut spilled);
        self.coro_spilled_vars = spilled.clone();

        // Frame layout: [resume_fn(8)] [state(8)] [future(8)] [var0(8)] ... [varN(8)] [awaited_future(8)]
        let num_spilled = spilled.len();
        let frame_size = CORO_HEADER_SIZE as usize + (num_spilled + 1) * 8;
        self._coro_frame_size = frame_size as u32;

        // Set coro_future — will be loaded in the coro entry
        self.coro_future = Some("%c.future".to_string());

        // Register function param info and types
        let param_info: Vec<(String, Type)> = params
            .iter()
            .map(|p| (p.name.clone(), p.param_type.clone()))
            .collect();
        self.function_param_info
            .insert(func_name.clone(), param_info);
        self.function_types
            .insert(func_name.clone(), return_type.clone());

        // --- Generate coroutine body ---
        // Allocate stack slots for all params (same as gen_function_decl)
        for p in params {
            let slot = self.fresh_reg();
            self.emit_alloc(&format!("    {} =l alloc8 8", slot));
            self.variable_registers.insert(p.name.clone(), slot);
            self.variable_types
                .insert(p.name.clone(), p.param_type.clone());
        }

        self.emit("    call $gc_push_frame()");
        self.gen_block_stmts(&body.statements);
        if !self.block_terminated {
            // Implicit return: complete future with 0
            self.emit(&format!("    call $mog_future_complete(l %c.future, l 0)"));
            self.emit("    call $gc_pop_frame()");
            self.emit("    ret");
        }

        // Capture generated lines and entry allocs
        let body_entry_allocs = std::mem::take(&mut self.entry_allocs);
        let body_lines = std::mem::take(&mut self.current_func_lines);
        let resume_blocks = self.coro_resume_blocks.clone();

        // ===================================================================
        // Assemble the CORO function: $funcName.coro(l %frame)
        // ===================================================================
        let mut coro_ir = format!("function ${}.coro(l %frame) {{\n", func_name);
        coro_ir.push_str("@c.entry\n");

        // Entry allocs (must be in entry block per QBE rules)
        for alloc in &body_entry_allocs {
            coro_ir.push_str(alloc);
            coro_ir.push('\n');
        }

        // Load future from frame
        coro_ir.push_str("    %c.future.ptr =l add %frame, 16\n");
        coro_ir.push_str("    %c.future =l loadl %c.future.ptr\n");

        // Load state from frame
        coro_ir.push_str("    %c.state.ptr =l add %frame, 8\n");
        coro_ir.push_str("    %c.state =l loadl %c.state.ptr\n");

        // Branch: initial (-1) vs dispatch (resume)
        coro_ir.push_str("    %c.is_initial =w ceql %c.state, -1\n");
        coro_ir.push_str("    jnz %c.is_initial, @c.initial, @c.dispatch\n");

        // --- @c.dispatch: restore vars from frame and jump to the right resume point ---
        coro_ir.push_str("@c.dispatch\n");

        // Restore ALL spilled vars from frame
        {
            let mut offset = CORO_HEADER_SIZE;
            for (i, var_name) in spilled.iter().enumerate() {
                // Find the alloc slot for this var in body_entry_allocs
                // The slot register is the i-th param/var allocation
                // We need to find the register from the variable_registers map
                // but that's been used during body generation.
                // Instead, parse the entry_allocs to find the register name.
                // The entry_allocs are pairs: "    %v.N =l alloc8 8" + "  storel 0, %v.N"
                // The first param alloc is at index 0*2, second at 1*2, etc.
                let alloc_idx = i * 2; // Each alloc produces 2 lines (alloc + zero-init)
                if alloc_idx < body_entry_allocs.len() {
                    if let Some(reg) = body_entry_allocs[alloc_idx]
                        .split('=')
                        .next()
                        .map(|s| s.trim().to_string())
                    {
                        let roff = format!("%c.r.{}", i);
                        coro_ir.push_str(&format!("    {} =l add %frame, {}\n", roff, offset));
                        let rval = format!("%c.rv.{}", i);
                        let var_is_float = params
                            .iter()
                            .find(|p| p.name == *var_name)
                            .map(|p| p.param_type.is_float())
                            .unwrap_or(false);
                        if var_is_float {
                            coro_ir.push_str(&format!("    {} =d loadd {}\n", rval, roff));
                            coro_ir.push_str(&format!("    stored {}, {}\n", rval, reg));
                        } else {
                            coro_ir.push_str(&format!("    {} =l loadl {}\n", rval, roff));
                            coro_ir.push_str(&format!("    storel {}, {}\n", rval, reg));
                        }
                    }
                }
                offset += 8;
            }
        }

        // Linear dispatch chain: compare state to 0, 1, 2, ... and jump
        for (i, lbl) in resume_blocks.iter().enumerate() {
            let cmp_reg = format!("%c.cmp.{}", i);
            coro_ir.push_str(&format!("    {} =w ceql %c.state, {}\n", cmp_reg, i));
            let next_lbl = if i + 1 < resume_blocks.len() {
                format!("@c.check.{}", i + 1)
            } else {
                "@c.dispatch.end".to_string()
            };
            coro_ir.push_str(&format!("    jnz {}, {}, {}\n", cmp_reg, lbl, next_lbl));
            if i + 1 < resume_blocks.len() {
                coro_ir.push_str(&format!("@c.check.{}\n", i + 1));
            }
        }

        coro_ir.push_str("@c.dispatch.end\n");
        coro_ir.push_str("    call $gc_pop_frame()\n");
        coro_ir.push_str("    ret\n");

        // --- @c.initial: load params from frame, then fall through to body ---
        coro_ir.push_str("@c.initial\n");

        // Load params from frame into their stack slots
        {
            let mut offset = CORO_HEADER_SIZE;
            for (i, p) in params.iter().enumerate() {
                let alloc_idx = i * 2;
                if alloc_idx < body_entry_allocs.len() {
                    if let Some(reg) = body_entry_allocs[alloc_idx]
                        .split('=')
                        .next()
                        .map(|s| s.trim().to_string())
                    {
                        let poff = format!("%c.p.{}", i);
                        coro_ir.push_str(&format!("    {} =l add %frame, {}\n", poff, offset));
                        let pval = format!("%c.pv.{}", i);
                        let op = if p.param_type.is_float() {
                            coro_ir.push_str(&format!("    {} =d loadd {}\n", pval, poff));
                            "stored"
                        } else {
                            coro_ir.push_str(&format!("    {} =l loadl {}\n", pval, poff));
                            "storel"
                        };
                        coro_ir.push_str(&format!("    {} {}, {}\n", op, pval, reg));
                    }
                }
                offset += 8;
            }
        }

        // Body code
        for line in &body_lines {
            coro_ir.push_str(line);
            coro_ir.push('\n');
        }
        coro_ir.push_str("}\n\n");
        self.output.push_str(&coro_ir);

        // ===================================================================
        // Assemble the WRAPPER function: $funcName(params...) -> l (future)
        // ===================================================================
        let mut param_strs = Vec::new();
        for p in params {
            let pty = self.to_qbe_type(&p.param_type);
            param_strs.push(format!("{} %p.{}", pty, p.name));
        }
        let params_joined = param_strs.join(", ");

        let export = if is_public && self.plugin_mode {
            "export "
        } else {
            ""
        };

        let mut wrapper_ir = format!(
            "{}function l ${}({}) {{\n",
            export, func_name, params_joined
        );
        wrapper_ir.push_str("@start\n");

        // Allocate frame
        wrapper_ir.push_str(&format!(
            "    %init.frame =l call $malloc(l {})\n",
            frame_size
        ));
        // frame[0] = resume function pointer
        wrapper_ir.push_str(&format!("    storel ${}.coro, %init.frame\n", func_name));
        // frame[8] = state = -1 (initial)
        wrapper_ir.push_str("    %s0 =l add %init.frame, 8\n");
        wrapper_ir.push_str("    storel -1, %s0\n");
        // frame[16] = future
        wrapper_ir.push_str("    %init.future =l call $mog_future_new()\n");
        wrapper_ir.push_str("    %f0 =l add %init.frame, 16\n");
        wrapper_ir.push_str("    storel %init.future, %f0\n");
        // Set coro frame on the future
        wrapper_ir.push_str("    call $mog_future_set_coro_frame(l %init.future, l %init.frame)\n");
        // Store params into frame at offset 24+
        {
            let mut offset = CORO_HEADER_SIZE;
            for (i, p) in params.iter().enumerate() {
                let poff = format!("%wp.{}", i);
                wrapper_ir.push_str(&format!("    {} =l add %init.frame, {}\n", poff, offset));
                let op = if p.param_type.is_float() {
                    "stored"
                } else {
                    "storel"
                };
                wrapper_ir.push_str(&format!("    {} %p.{}, {}\n", op, p.name, poff));
                offset += 8;
            }
        }
        // Eager execution: call the coro function
        wrapper_ir.push_str(&format!("    call ${}.coro(l %init.frame)\n", func_name));
        // Return the future
        wrapper_ir.push_str("    ret %init.future\n");
        wrapper_ir.push_str("}\n\n");
        self.output.push_str(&wrapper_ir);

        if is_public {
            self.pub_functions
                .push((name.to_string(), params.len(), true));
        }

        // --- Restore all state ---
        self.variable_registers = saved_regs;
        self.variable_types = saved_types;
        self.entry_allocs = saved_entry;
        self.current_func_lines = saved_func;
        self.current_function_return_type = saved_ret;
        self.block_terminated = saved_term;
        self.reg_counter = saved_reg_counter;
        self.label_counter = saved_label_counter;
        self.float_regs = saved_float_regs;
        self.in_async_function = saved_in_async;
        self.coro_frame = saved_coro_frame;
        self.coro_future = saved_coro_future;
        self.await_counter = saved_await_counter;
        self.coro_spilled_vars = saved_coro_spilled;
        self.coro_resume_blocks = saved_coro_resume;
    }

    // -----------------------------------------------------------------------
    // Top-level orchestration
    // -----------------------------------------------------------------------

    fn generate(&mut self, ast: &Statement) {
        // Pre-register structs and functions
        if let StatementKind::Program { statements, .. } = &ast.kind {
            for stmt in statements {
                if let StatementKind::StructDefinition { name, fields, .. } = &stmt.kind {
                    let field_vec: Vec<(String, Type)> = fields
                        .iter()
                        .map(|f| (f.name.clone(), f.field_type.clone()))
                        .collect();
                    self.struct_fields.insert(name.clone(), field_vec);
                }
                if let StatementKind::FunctionDeclaration {
                    name,
                    params,
                    return_type,
                    ..
                } = &stmt.kind
                {
                    self.function_types
                        .insert(name.clone(), return_type.clone());
                    let param_info: Vec<(String, Type)> = params
                        .iter()
                        .map(|p| (p.name.clone(), p.param_type.clone()))
                        .collect();
                    self.function_param_info.insert(name.clone(), param_info);
                }
                if let StatementKind::AsyncFunctionDeclaration {
                    name,
                    params,
                    return_type,
                    ..
                } = &stmt.kind
                {
                    self.function_types
                        .insert(name.clone(), return_type.clone());
                    self.async_functions.insert(name.to_string());
                    self.has_async_functions = true;
                    let param_info: Vec<(String, Type)> = params
                        .iter()
                        .map(|p| (p.name.clone(), p.param_type.clone()))
                        .collect();
                    self.function_param_info.insert(name.clone(), param_info);
                }
            }

            // Collect string constants
            self.collect_strings(ast);

            // Separate top-level statements from function/struct declarations
            let mut top_level = Vec::new();
            for stmt in statements {
                match &stmt.kind {
                    StatementKind::FunctionDeclaration { .. }
                    | StatementKind::AsyncFunctionDeclaration { .. }
                    | StatementKind::StructDefinition { .. }
                    | StatementKind::TypeAliasDeclaration { .. }
                    | StatementKind::PackageDeclaration { .. }
                    | StatementKind::ImportDeclaration { .. } => {
                        self.gen_statement(stmt);
                    }
                    StatementKind::RequiresDeclaration { capabilities } => {
                        for cap in capabilities {
                            self.capabilities.insert(cap.clone());
                        }
                    }
                    StatementKind::OptionalDeclaration { capabilities } => {
                        for cap in capabilities {
                            self.capabilities.insert(cap.clone());
                        }
                    }
                    _ => {
                        top_level.push(stmt.clone());
                    }
                }
            }

            // Generate program function for top-level statements
            if !top_level.is_empty() {
                self.gen_program_function(&top_level);
            }

            // Generate main if there's a program_user function
            if self.function_types.contains_key("main")
                || self.function_types.contains_key("program_user")
                || !top_level.is_empty()
            {
                self.gen_main_entry(!top_level.is_empty());
            }

            // Plugin support
            if self.plugin_mode {
                self.gen_plugin_info();
                self.gen_plugin_init();
                self.gen_plugin_exports();
            }
        }
    }

    fn collect_strings(&mut self, ast: &Statement) {
        match &ast.kind {
            StatementKind::Program { statements, .. } | StatementKind::Block { statements, .. } => {
                for s in statements {
                    self.collect_strings(s);
                }
            }
            StatementKind::FunctionDeclaration { body, .. }
            | StatementKind::AsyncFunctionDeclaration { body, .. } => {
                for s in &body.statements {
                    self.collect_strings(s);
                }
            }
            StatementKind::ExpressionStatement { expression } => {
                self.collect_strings_expr(expression);
            }
            StatementKind::VariableDeclaration { value, .. } => {
                if let Some(v) = value {
                    self.collect_strings_expr(v);
                }
            }
            StatementKind::Assignment { value, .. } => {
                self.collect_strings_expr(value);
            }
            StatementKind::Return { value } => {
                if let Some(v) = value {
                    self.collect_strings_expr(v);
                }
            }
            _ => {}
        }
    }

    fn collect_strings_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::StringLiteral { value } => {
                self.register_string(value);
            }
            ExprKind::TemplateLiteral { parts } => {
                for p in parts {
                    if let TemplatePart::String(s) = p {
                        self.register_string(s);
                    }
                    if let TemplatePart::Expr(e) = p {
                        self.collect_strings_expr(e);
                    }
                }
            }
            ExprKind::CallExpression {
                callee, arguments, ..
            } => {
                self.collect_strings_expr(callee);
                for a in arguments {
                    self.collect_strings_expr(a);
                }
            }
            ExprKind::BinaryExpression { left, right, .. } => {
                self.collect_strings_expr(left);
                self.collect_strings_expr(right);
            }
            _ => {}
        }
    }

    fn gen_program_function(&mut self, stmts: &[Statement]) {
        let saved_regs = std::mem::take(&mut self.variable_registers);
        let saved_types = std::mem::take(&mut self.variable_types);
        let saved_entry = std::mem::take(&mut self.entry_allocs);
        let saved_func = std::mem::take(&mut self.current_func_lines);
        let saved_ret = self.current_function_return_type.take();
        self.reset_counters();

        self.emit("    call $gc_push_frame()");
        for stmt in stmts {
            self.gen_statement(stmt);
        }
        self.emit("    call $gc_pop_frame()");
        self.emit("    ret");

        let mut func_ir = String::from("function $mog_program() {\n@start\n");
        for alloc in &self.entry_allocs {
            func_ir.push_str(alloc);
            func_ir.push('\n');
        }
        for line in &self.current_func_lines {
            func_ir.push_str(line);
            func_ir.push('\n');
        }
        func_ir.push_str("}\n\n");
        self.output.push_str(&func_ir);

        self.variable_registers = saved_regs;
        self.variable_types = saved_types;
        self.entry_allocs = saved_entry;
        self.current_func_lines = saved_func;
        self.current_function_return_type = saved_ret;
    }

    fn gen_main_entry(&mut self, has_program_fn: bool) {
        let is_async_main =
            self.async_functions.contains("main") || self.async_functions.contains("program_user");

        let mut main_ir = String::from("export function w $main() {\n@start\n");
        main_ir.push_str("    call $gc_init()\n");
        main_ir.push_str("    call $gc_push_frame()\n");
        // Check if VM exists, create if not
        main_ir.push_str("    %vm.0 =l call $mog_vm_get_global()\n");
        main_ir.push_str("    %has_vm =w ceql %vm.0, 0\n");
        main_ir.push_str("    jnz %has_vm, @create_vm, @have_vm\n");
        main_ir.push_str("@create_vm\n");
        main_ir.push_str("    %new_vm =l call $mog_vm_new()\n");
        main_ir.push_str("    call $mog_vm_set_global(l %new_vm)\n");
        main_ir.push_str("    call $mog_register_posix_host(l %new_vm)\n");
        main_ir.push_str("    jmp @have_vm\n");
        main_ir.push_str("@have_vm\n");

        if is_async_main {
            // Async main: create event loop, call program_user (returns future), run loop
            main_ir.push_str("    %loop =l call $mog_loop_new()\n");
            main_ir.push_str("    call $mog_loop_set_global(l %loop)\n");
            if has_program_fn {
                main_ir.push_str("    call $mog_program()\n");
            }
            main_ir.push_str("    %future =l call $program_user()\n");
            main_ir.push_str("    call $mog_loop_run(l %loop)\n");
            main_ir.push_str("    %ret =l call $mog_future_get_result(l %future)\n");
            main_ir.push_str("    call $mog_loop_free(l %loop)\n");
            main_ir.push_str("    call $gc_pop_frame()\n");
            main_ir.push_str("    %retw =w copy %ret\n");
            main_ir.push_str("    ret %retw\n");
        } else {
            if has_program_fn {
                main_ir.push_str("    call $mog_program()\n");
            }
            if self.function_types.contains_key("main")
                || self.function_types.contains_key("program_user")
            {
                main_ir.push_str("    %ret =l call $program_user()\n");
                main_ir.push_str("    call $gc_pop_frame()\n");
                main_ir.push_str("    %retw =w copy %ret\n");
                main_ir.push_str("    ret %retw\n");
            } else {
                main_ir.push_str("    call $gc_pop_frame()\n");
                main_ir.push_str("    ret 0\n");
            }
        }
        main_ir.push_str("}\n\n");
        self.output.push_str(&main_ir);
    }

    fn gen_plugin_info(&mut self) {
        let name = self.plugin_name.clone().unwrap_or_default();
        let version = self
            .plugin_version
            .clone()
            .unwrap_or_else(|| "0.1.0".to_string());
        let name_label = self.register_string(&name);
        let ver_label = self.register_string(&version);
        let num_exports = self.pub_functions.len();
        let mut ir = String::from("export function l $mog_plugin_info() {\n@start\n");
        ir.push_str(&format!("    %info =l call $gc_alloc(l 24)\n"));
        ir.push_str(&format!("    storel {}, %info\n", name_label));
        ir.push_str(&format!("    %ver_off =l add %info, 8\n"));
        ir.push_str(&format!("    storel {}, %ver_off\n", ver_label));
        ir.push_str(&format!("    %num_off =l add %info, 16\n"));
        ir.push_str(&format!("    storel {}, %num_off\n", num_exports));
        ir.push_str("    ret %info\n");
        ir.push_str("}\n\n");
        self.output.push_str(&ir);
    }

    fn gen_plugin_init(&mut self) {
        let mut ir = String::from("export function $mog_plugin_init(l %vm) {\n@start\n");
        ir.push_str("    call $mog_vm_set_global(l %vm)\n");
        ir.push_str("    call $gc_init()\n");
        ir.push_str("    ret\n");
        ir.push_str("}\n\n");
        self.output.push_str(&ir);
    }

    fn gen_plugin_exports(&mut self) {
        let fns = self.pub_functions.clone();
        let num = fns.len();
        let mut ir = format!("export function l $mog_plugin_exports(l %count_out) {{\n@start\n");
        ir.push_str(&format!("    storel {}, %count_out\n", num));
        ir.push_str(&format!("    %arr =l call $gc_alloc(l {})\n", num * 16));
        for (i, (name, _nparams, _is_async)) in fns.iter().enumerate() {
            let name_label = self.register_string(name);
            let base = i * 16;
            if base == 0 {
                ir.push_str(&format!("    storel {}, %arr\n", name_label));
                ir.push_str(&format!("    %fp_{} =l add %arr, 8\n", i));
                ir.push_str(&format!("    storel $mogp_{}, %fp_{}\n", name, i));
            } else {
                ir.push_str(&format!("    %no_{} =l add %arr, {}\n", i, base));
                ir.push_str(&format!("    storel {}, %no_{}\n", name_label, i));
                ir.push_str(&format!("    %fp_{} =l add %arr, {}\n", i, base + 8));
                ir.push_str(&format!("    storel $mogp_{}, %fp_{}\n", name, i));
            }
        }
        ir.push_str("    ret %arr\n");
        ir.push_str("}\n\n");
        self.output.push_str(&ir);

        // Generate mogp_ wrappers
        for (name, nparams, _) in &fns {
            let func_name = format!("{}{}", self.package_prefix, name);
            let mut params = Vec::new();
            for i in 0..*nparams {
                params.push(format!("l %a{}", i));
            }
            let params_str = params.join(", ");
            let call_args = (0..*nparams)
                .map(|i| format!("l %a{}", i))
                .collect::<Vec<_>>()
                .join(", ");
            let mut wrapper = format!(
                "export function l $mogp_{}({}) {{\n@start\n",
                name, params_str
            );
            wrapper.push_str(&format!("    %r =l call ${}({})\n", func_name, call_args));
            wrapper.push_str("    ret %r\n");
            wrapper.push_str("}\n\n");
            self.output.push_str(&wrapper);
        }
    }

    fn set_capability_decls(&mut self, decls: HashMap<String, CapabilityDecl>) {
        self.capability_decls = decls;
    }

    fn set_plugin_mode(&mut self, name: &str, version: &str) {
        self.plugin_mode = true;
        self.plugin_name = Some(name.to_string());
        self.plugin_version = Some(version.to_string());
    }

    fn gen_number_literal(&mut self, value: &str, literal_type: Option<&Type>) -> String {
        if let Some(ty) = literal_type {
            match ty {
                Type::Float(FloatKind::F32) => {
                    if let Ok(v) = value.parse::<f32>() {
                        let lit = Self::float_to_single_qbe(v);
                        let reg = self.fresh_reg();
                        self.emit(&format!("  {} =s copy {}", reg, lit));
                        return reg;
                    }
                }
                Type::Float(_) => {
                    if let Ok(v) = value.parse::<f64>() {
                        // Load float literal via copy to avoid QBE parser issues
                        // with hex floats containing 'e' (treated as sci notation)
                        let lit = Self::float_to_qbe(v);
                        let reg = self.fresh_reg();
                        self.emit(&format!("  {} =d copy {}", reg, lit));
                        self.mark_float_reg(&reg);
                        return reg;
                    }
                }
                _ => {}
            }
        }
        if value.contains('.') {
            if let Ok(v) = value.parse::<f64>() {
                // Load float literal via copy to avoid QBE parser issues
                let lit = Self::float_to_qbe(v);
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =d copy {}", reg, lit));
                self.mark_float_reg(&reg);
                return reg;
            }
        }
        value.to_string()
    }

    fn gen_string_literal(&mut self, value: &str) -> String {
        self.register_string(value)
    }

    fn gen_identifier(&mut self, name: &str) -> String {
        // Math constants
        if name == "PI" {
            let r = self.fresh_reg();
            self.emit(&format!("    {} =d copy d_3.141592653589793e0", r));
            self.mark_float_reg(&r);
            return r;
        }
        if name == "E" {
            let r = self.fresh_reg();
            self.emit(&format!("    {} =d copy d_2.718281828459045e0", r));
            self.mark_float_reg(&r);
            return r;
        }
        if let Some(slot) = self.variable_registers.get(name).cloned() {
            let reg = self.fresh_reg();
            let is_float = matches!(self.variable_types.get(name), Some(t) if t.is_float());
            if is_float {
                self.emit(&format!("  {} =d loadd {}", reg, slot));
                self.mark_float_reg(&reg);
            } else {
                self.emit(&format!("  {} =l loadl {}", reg, slot));
            }
            reg
        } else if self.function_types.contains_key(name) {
            format!("${}", name)
        } else {
            "0".to_string()
        }
    }

    fn gen_binary_expr(
        &mut self,
        op: &str,
        left: &Expr,
        right: &Expr,
        is_float: bool,
        is_str_left: bool,
        is_str_right: bool,
    ) -> String {
        // String equality
        if (op == "==" || op == "!=") && (is_str_left || is_str_right) {
            let l = self.gen_expr(left);
            let r = self.gen_expr(right);
            let reg = self.fresh_reg();
            self.emit(&format!("  {} =l call $string_eq(l {}, l {})", reg, l, r));
            if op == "!=" {
                let r2 = self.fresh_reg();
                self.emit(&format!("  {} =l xor {}, 1", r2, reg));
                return r2;
            }
            return reg;
        }

        // String concatenation
        if op == "+" && (is_str_left || is_str_right) {
            let l = self.gen_expr(left);
            let r = self.gen_expr(right);
            let reg = self.fresh_reg();
            self.emit(&format!(
                "  {} =l call $string_concat(l {}, l {})",
                reg, l, r
            ));
            return reg;
        }

        // Element-wise array operations
        let is_array_left = self.is_array_expr(left);
        let is_array_right = self.is_array_expr(right);
        if is_array_left && is_array_right {
            let l = self.gen_expr(left);
            let r = self.gen_expr(right);
            let result = self.fresh_reg();
            let runtime_fn = match op {
                "+" => "vector_add",
                "-" => "vector_sub",
                "*" => "vector_mul",
                "/" => "vector_div",
                _ => "vector_add",
            };
            self.emit(&format!(
                "  {} =l call ${}(l {}, l {}, l 0)",
                result, runtime_fn, l, r
            ));
            return result;
        }
        if is_array_left || is_array_right {
            // Scalar-array operation
            let l = self.gen_expr(left);
            let r = self.gen_expr(right);
            let (scalar, array) = if is_array_left { (r, l) } else { (l, r) };
            let result = self.fresh_reg();
            let runtime_fn = match op {
                "+" => "vector_scalar_add",
                "-" => "vector_scalar_sub",
                "*" => "vector_scalar_mul",
                "/" => "vector_scalar_div",
                _ => "vector_scalar_add",
            };
            self.emit(&format!(
                "  {} =l call ${}(l {}, l {}, l 0)",
                result, runtime_fn, scalar, array
            ));
            return result;
        }

        // Tensor element-wise operations
        let is_tensor_left = self.is_tensor_producing_expr(left);
        let is_tensor_right = self.is_tensor_producing_expr(right);
        if is_tensor_left && is_tensor_right {
            let l = self.gen_expr(left);
            let r = self.gen_expr(right);
            let result = self.fresh_reg();
            let runtime_fn = match op {
                "+" => "tensor_add",
                "-" => "tensor_sub",
                "*" => "tensor_mul",
                "/" => "tensor_div",
                _ => "tensor_add",
            };
            self.emit(&format!(
                "  {} =l call ${}(l {}, l {})",
                result, runtime_fn, l, r
            ));
            return result;
        }

        let mut l = self.gen_expr(left);
        let mut r = self.gen_expr(right);

        // Re-check float status after generation — struct field loads may have
        // placed registers in float_regs that weren't detectable from AST alone
        let is_float = is_float || self.float_regs.contains(&l) || self.float_regs.contains(&r);

        if is_float {
            if !self.is_float_operand(left) && !self.float_regs.contains(&l) {
                let tmp = self.fresh_reg();
                self.emit(&format!("  {} =d swtof {}", tmp, l));
                self.mark_float_reg(&tmp);
                l = tmp;
            }
            if !self.is_float_operand(right) && !self.float_regs.contains(&r) {
                let tmp = self.fresh_reg();
                self.emit(&format!("  {} =d swtof {}", tmp, r));
                self.mark_float_reg(&tmp);
                r = tmp;
            }

            match op {
                "+" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d add {}, {}", reg, l, r));
                    self.mark_float_reg(&reg);
                    return reg;
                }
                "-" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d sub {}, {}", reg, l, r));
                    self.mark_float_reg(&reg);
                    return reg;
                }
                "*" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d mul {}, {}", reg, l, r));
                    self.mark_float_reg(&reg);
                    return reg;
                }
                "/" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d div {}, {}", reg, l, r));
                    self.mark_float_reg(&reg);
                    return reg;
                }
                _ => {}
            }

            let cmp_op = match op {
                "==" => Some("ceqd"),
                "!=" => Some("cned"),
                "<" => Some("cltd"),
                ">" => Some("cgtd"),
                "<=" => Some("cled"),
                ">=" => Some("cged"),
                _ => None,
            };
            if let Some(cop) = cmp_op {
                let rw = self.fresh_reg();
                self.emit(&format!("  {} =w {} {}, {}", rw, cop, l, r));
                let rl = self.fresh_reg();
                self.emit(&format!("  {} =l extsw {}", rl, rw));
                return rl;
            }
        }

        // Integer arithmetic
        match op {
            "+" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l add {}, {}", reg, l, r));
                return reg;
            }
            "-" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l sub {}, {}", reg, l, r));
                return reg;
            }
            "*" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l mul {}, {}", reg, l, r));
                return reg;
            }
            "/" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l div {}, {}", reg, l, r));
                return reg;
            }
            "%" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l rem {}, {}", reg, l, r));
                return reg;
            }
            _ => {}
        }

        // Integer comparisons
        let int_cmp = match op {
            "==" => Some("ceql"),
            "!=" => Some("cnel"),
            "<" => Some("csltl"),
            ">" => Some("csgtl"),
            "<=" => Some("cslel"),
            ">=" => Some("csgel"),
            _ => None,
        };
        if let Some(cop) = int_cmp {
            let rw = self.fresh_reg();
            self.emit(&format!("  {} =w {} {}, {}", rw, cop, l, r));
            let rl = self.fresh_reg();
            self.emit(&format!("  {} =l extsw {}", rl, rw));
            return rl;
        }

        // Logical operators
        if op == "&&" {
            let lb = self.fresh_reg();
            self.emit(&format!("  {} =w cnel {}, 0", lb, l));
            let rb = self.fresh_reg();
            self.emit(&format!("  {} =w cnel {}, 0", rb, r));
            let rw = self.fresh_reg();
            self.emit(&format!("  {} =w and {}, {}", rw, lb, rb));
            let rl = self.fresh_reg();
            self.emit(&format!("  {} =l extsw {}", rl, rw));
            return rl;
        }
        if op == "||" {
            let lb = self.fresh_reg();
            self.emit(&format!("  {} =w cnel {}, 0", lb, l));
            let rb = self.fresh_reg();
            self.emit(&format!("  {} =w cnel {}, 0", rb, r));
            let rw = self.fresh_reg();
            self.emit(&format!("  {} =w or {}, {}", rw, lb, rb));
            let rl = self.fresh_reg();
            self.emit(&format!("  {} =l extsw {}", rl, rw));
            return rl;
        }

        // Bitwise operators
        match op {
            "&" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l and {}, {}", reg, l, r));
                return reg;
            }
            "|" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l or {}, {}", reg, l, r));
                return reg;
            }
            "^" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l xor {}, {}", reg, l, r));
                return reg;
            }
            "<<" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l shl {}, {}", reg, l, r));
                return reg;
            }
            ">>" => {
                let reg = self.fresh_reg();
                self.emit(&format!("  {} =l sar {}, {}", reg, l, r));
                return reg;
            }
            _ => {}
        }

        "0".to_string()
    }

    fn gen_unary_expr(&mut self, op: &str, operand: &Expr, is_float: bool) -> String {
        let val = self.gen_expr(operand);
        match op {
            "-" => {
                if is_float {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =d neg {}", r, val));
                    self.mark_float_reg(&r);
                    r
                } else {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l sub 0, {}", r, val));
                    r
                }
            }
            "!" => {
                let r = self.fresh_reg();
                self.emit(&format!("  {} =l xor {}, 1", r, val));
                r
            }
            "~" => {
                let r = self.fresh_reg();
                self.emit(&format!("  {} =l xor {}, -1", r, val));
                r
            }
            _ => val,
        }
    }

    // --- Call Expression ---
    fn gen_call_expr(
        &mut self,
        callee: &Expr,
        arguments: &[Expr],
        _named_args: &Option<Vec<NamedArg>>,
    ) -> String {
        // Direct identifier calls
        if let ExprKind::Identifier { name } = &callee.kind {
            let name = name.clone();

            // Generate all argument registers
            let arg_regs: Vec<String> = arguments.iter().map(|a| self.gen_expr(a)).collect();

            // Print functions
            let print_funcs = [
                "print",
                "println",
                "print_string",
                "println_string",
                "print_i64",
                "println_i64",
                "print_f64",
                "println_f64",
                "print_u64",
                "println_u64",
                "print_buffer",
            ];
            if print_funcs.contains(&name.as_str()) {
                return self.gen_print_call(&name, &arg_regs, arguments);
            }

            // Math builtins (single-arg)
            let math_single = [
                "sqrt", "sin", "cos", "tan", "asin", "acos", "exp", "log", "log2", "floor", "ceil",
                "round", "abs",
            ];
            if math_single.contains(&name.as_str()) {
                let r = self.fresh_reg();
                let qbe_name = if name == "abs" { "fabs" } else { &name };
                self.emit(&format!("  {} =d call ${}(d {})", r, qbe_name, arg_regs[0]));
                self.mark_float_reg(&r);
                return r;
            }

            // Math builtins (two-arg)
            let math_double: HashMap<&str, &str> = [
                ("pow", "pow"),
                ("atan2", "atan2"),
                ("min", "fmin"),
                ("max", "fmax"),
                ("fmin", "fmin"),
                ("fmax", "fmax"),
            ]
            .iter()
            .copied()
            .collect();
            if let Some(qbe_name) = math_double.get(name.as_str()) {
                let r = self.fresh_reg();
                self.emit(&format!(
                    "  {} =d call ${}(d {}, d {})",
                    r, qbe_name, arg_regs[0], arg_regs[1]
                ));
                self.mark_float_reg(&r);
                return r;
            }

            // Conversion functions
            match name.as_str() {
                "str" | "i64_to_string" => {
                    let r = self.fresh_reg();
                    // Route to f64_to_string if argument is float
                    if self.is_float_operand(&arguments[0])
                        || self.float_regs.contains(&arg_regs[0])
                    {
                        self.emit(&format!(
                            "  {} =l call $f64_to_string(d {})",
                            r, arg_regs[0]
                        ));
                    } else {
                        self.emit(&format!(
                            "  {} =l call $i64_to_string(l {})",
                            r, arg_regs[0]
                        ));
                    }
                    return r;
                }
                "u64_to_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $u64_to_string(l {})",
                        r, arg_regs[0]
                    ));
                    return r;
                }
                "f64_to_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $f64_to_string(d {})",
                        r, arg_regs[0]
                    ));
                    return r;
                }
                "int_from_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $int_from_string(l {})",
                        r, arg_regs[0]
                    ));
                    return r;
                }
                "float_from_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =d call $float_from_string(l {})",
                        r, arg_regs[0]
                    ));
                    self.mark_float_reg(&r);
                    return r;
                }
                "input_i64" | "input_u64" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call ${}()", r, name));
                    return r;
                }
                "input_f64" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =d call $input_f64()", r));
                    self.mark_float_reg(&r);
                    return r;
                }
                "input_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $input_string()", r));
                    return r;
                }
                "tensor_print" => {
                    self.emit(&format!("  call $tensor_print(l {})", arg_regs[0]));
                    return "0".to_string();
                }
                "gc_collect" | "gc_benchmark_stats" | "gc_reset_stats" => {
                    self.emit(&format!("  call ${}()", name));
                    return "0".to_string();
                }
                _ => {}
            }

            // Tensor activation free functions
            match name.as_str() {
                "relu" if arg_regs.len() == 1 => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $tensor_relu(l {})", r, arg_regs[0]));
                    return r;
                }
                "sigmoid" if arg_regs.len() == 1 => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $tensor_sigmoid(l {})",
                        r, arg_regs[0]
                    ));
                    return r;
                }
                "softmax" => {
                    let dim_arg = if arg_regs.len() > 1 {
                        &arg_regs[1]
                    } else {
                        &"-1".to_string()
                    };
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $tensor_softmax(l {}, l {})",
                        r, arg_regs[0], dim_arg
                    ));
                    return r;
                }
                "matmul" if arg_regs.len() == 2 => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $tensor_matmul(l {}, l {})",
                        r, arg_regs[0], arg_regs[1]
                    ));
                    return r;
                }
                _ => {}
            }

            // Async combinators: all([...]) and race([...])
            // mog_all / mog_race take (MogFuture** buf, int count)
            match name.as_str() {
                "all" | "race" => {
                    let runtime_fn = if name == "all" {
                        "$mog_all"
                    } else {
                        "$mog_race"
                    };
                    // Check if the single argument is an ArrayLiteral
                    if arguments.len() == 1 {
                        if let ExprKind::ArrayLiteral { elements } = &arguments[0].kind {
                            // Generate each element (future) individually
                            let mut future_regs = Vec::new();
                            for elem in elements {
                                let fr = self.gen_expr(elem);
                                future_regs.push(fr);
                            }
                            let count = future_regs.len();
                            let buf_size = count * 8;
                            let buf = self.fresh_reg();
                            self.emit(&format!("  {} =l call $gc_alloc(l {})", buf, buf_size));
                            for (i, fr) in future_regs.iter().enumerate() {
                                if i == 0 {
                                    self.emit(&format!("  storel {}, {}", fr, buf));
                                } else {
                                    let off = self.fresh_reg();
                                    self.emit(&format!("  {} =l add {}, {}", off, buf, i * 8));
                                    self.emit(&format!("  storel {}, {}", fr, off));
                                }
                            }
                            let r = self.fresh_reg();
                            self.emit(&format!(
                                "  {} =l call {}(l {}, l {})",
                                r, runtime_fn, buf, count
                            ));
                            return r;
                        }
                    }
                    // Fallback: argument is not an ArrayLiteral (e.g. a variable)
                    // Pass the already-evaluated arg as-is (pointer to array + count unknown,
                    // use the single arg_reg as the buffer pointer with count 1)
                    if !arg_regs.is_empty() {
                        let r = self.fresh_reg();
                        self.emit(&format!(
                            "  {} =l call {}(l {}, l 1)",
                            r, runtime_fn, arg_regs[0]
                        ));
                        return r;
                    }
                }
                _ => {}
            }

            // If the name is a local variable (e.g. a closure binding), do an indirect call
            if let Some(slot) = self.variable_registers.get(&name).cloned() {
                let closure_reg = self.fresh_reg();
                self.emit(&format!("  {} =l loadl {}", closure_reg, slot));
                let fn_ptr = self.fresh_reg();
                self.emit(&format!("  {} =l loadl {}", fn_ptr, closure_reg));
                let env_off = self.fresh_reg();
                self.emit(&format!("  {} =l add {}, 8", env_off, closure_reg));
                let env_ptr = self.fresh_reg();
                self.emit(&format!("  {} =l loadl {}", env_ptr, env_off));
                let call_args_vec: Vec<String> = std::iter::once(format!("l {}", env_ptr))
                    .chain(arg_regs.iter().map(|r| format!("l {}", r)))
                    .collect();
                let call_args = call_args_vec.join(", ");
                let r = self.fresh_reg();
                self.emit(&format!("  {} =l call {}({})", r, fn_ptr, call_args));
                return r;
            }

            // General function calls
            let ret_type = self.function_types.get(&name).cloned();
            let qbe_ret = ret_type
                .as_ref()
                .map(|t| self.to_qbe_type(t))
                .unwrap_or("l");

            // Determine argument types from function parameter info or float_regs
            let param_info = self.function_param_info.get(&name).cloned();
            let arg_list: String = arg_regs
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let is_float_param = param_info
                        .as_ref()
                        .and_then(|pi| pi.get(i))
                        .map(|(_, t)| t.is_float())
                        .unwrap_or(false);
                    let ty = if is_float_param || self.float_regs.contains(r) {
                        "d"
                    } else {
                        "l"
                    };
                    format!("{} {}", ty, r)
                })
                .collect::<Vec<_>>()
                .join(", ");

            if qbe_ret == "void" {
                self.emit(&format!("  call ${}({})", name, arg_list));
                return "0".to_string();
            }

            let r = self.fresh_reg();
            let ret_str = if qbe_ret == "d" || qbe_ret == "s" {
                qbe_ret
            } else {
                "l"
            };
            self.emit(&format!(
                "  {} ={} call ${}({})",
                r, ret_str, name, arg_list
            ));
            if ret_str == "d" || ret_str == "s" {
                self.mark_float_reg(&r);
            }
            return r;
        }

        // Member expression calls
        if let ExprKind::MemberExpression { object, property } = &callee.kind {
            let method = property.clone();
            let obj_name = Self::get_identifier_name(object).map(|s| s.to_string());
            let obj_reg = self.gen_expr(object);
            let arg_regs: Vec<String> = arguments.iter().map(|a| self.gen_expr(a)).collect();

            // Package-qualified calls
            if let Some(ref oname) = obj_name {
                if let Some(pkg) = self.imported_packages.get(oname).cloned() {
                    let qualified = format!("{}_{}", pkg, method);
                    let pkg_param_info = self.function_param_info.get(&qualified).cloned();
                    let all_args: String = arg_regs
                        .iter()
                        .enumerate()
                        .map(|(i, r)| {
                            let is_float_param = pkg_param_info
                                .as_ref()
                                .and_then(|pi| pi.get(i))
                                .map(|(_, t)| t.is_float())
                                .unwrap_or(false);
                            let ty = if is_float_param || self.float_regs.contains(r) {
                                "d"
                            } else {
                                "l"
                            };
                            format!("{} {}", ty, r)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let ret_type = self.function_types.get(&qualified).cloned();
                    let qbe_ret = ret_type
                        .as_ref()
                        .map(|t| self.to_qbe_type(t))
                        .unwrap_or("l");
                    if qbe_ret == "void" {
                        self.emit(&format!("  call ${}({})", qualified, all_args));
                        return "0".to_string();
                    }
                    let r = self.fresh_reg();
                    let ret_str = if qbe_ret == "d" || qbe_ret == "s" {
                        qbe_ret
                    } else {
                        "l"
                    };
                    self.emit(&format!(
                        "  {} ={} call ${}({})",
                        r, ret_str, qualified, all_args
                    ));
                    if ret_str == "d" || ret_str == "s" {
                        self.mark_float_reg(&r);
                    }
                    return r;
                }

                // Capability calls
                if self.capabilities.contains(oname.as_str()) {
                    return self.gen_capability_call(oname, &method, &arg_regs, arguments);
                }
            }

            // String methods — check for String type or [u8] (byte array = string)
            if obj_name
                .as_ref()
                .map_or(false, |n| match self.variable_types.get(n.as_str()) {
                    Some(Type::String) => true,
                    Some(Type::Array { element_type, .. }) => {
                        matches!(
                            element_type.as_ref(),
                            Type::Unsigned(crate::types::UnsignedKind::U8)
                        )
                    }
                    _ => false,
                })
                || self.is_string_producing_expr(object)
            {
                let string_methods: HashMap<&str, (&str, bool)> = [
                    ("upper", ("string_upper", false)),
                    ("lower", ("string_lower", false)),
                    ("trim", ("string_trim", false)),
                    ("split", ("string_split", true)),
                    ("contains", ("string_contains", true)),
                    ("starts_with", ("string_starts_with", true)),
                    ("ends_with", ("string_ends_with", true)),
                    ("replace", ("string_replace", true)),
                    ("len", ("string_len", false)),
                    ("index_of", ("string_index_of", true)),
                ]
                .iter()
                .copied()
                .collect();
                if let Some((runtime, extra_args)) = string_methods.get(method.as_str()) {
                    let r = self.fresh_reg();
                    let args = if *extra_args {
                        let extra: String = arg_regs.iter().map(|a| format!(", l {}", a)).collect();
                        format!("l {}{}", obj_reg, extra)
                    } else {
                        format!("l {}", obj_reg)
                    };
                    self.emit(&format!("  {} =l call ${}({})", r, runtime, args));
                    return r;
                }
            }

            // Array methods
            let obj_type = obj_name
                .as_ref()
                .and_then(|n| self.variable_types.get(n.as_str()))
                .cloned();
            if matches!(&obj_type, Some(Type::Array { .. })) || self.is_array_expr(object) {
                match method.as_str() {
                    "push" => {
                        let val = self.ensure_long(&arg_regs[0]);
                        self.emit(&format!("  call $array_push(l {}, l {})", obj_reg, val));
                        return "0".to_string();
                    }
                    "pop" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $array_pop(l {})", r, obj_reg));
                        return r;
                    }
                    "contains" => {
                        let r = self.fresh_reg();
                        self.emit(&format!(
                            "  {} =l call $array_contains(l {}, l {})",
                            r, obj_reg, arg_regs[0]
                        ));
                        return r;
                    }
                    "sort" => {
                        if !arg_regs.is_empty() {
                            let fn_ptr = self.fresh_reg();
                            self.emit(&format!("  {} =l loadl {}", fn_ptr, arg_regs[0]));
                            let env_off = self.fresh_reg();
                            self.emit(&format!("  {} =l add {}, 8", env_off, arg_regs[0]));
                            let env_ptr = self.fresh_reg();
                            self.emit(&format!("  {} =l loadl {}", env_ptr, env_off));
                            self.emit(&format!(
                                "  call $array_sort_with_comparator(l {}, l {}, l {})",
                                obj_reg, fn_ptr, env_ptr
                            ));
                        } else {
                            self.emit(&format!("  call $array_sort(l {})", obj_reg));
                        }
                        return "0".to_string();
                    }
                    "reverse" => {
                        self.emit(&format!("  call $array_reverse(l {})", obj_reg));
                        return "0".to_string();
                    }
                    "join" => {
                        let r = self.fresh_reg();
                        self.emit(&format!(
                            "  {} =l call $array_join(l {}, l {})",
                            r, obj_reg, arg_regs[0]
                        ));
                        return r;
                    }
                    "len" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $array_len(l {})", r, obj_reg));
                        return r;
                    }
                    "filter" | "map" => {
                        let fn_ptr = self.fresh_reg();
                        self.emit(&format!("  {} =l loadl {}", fn_ptr, arg_regs[0]));
                        let env_off = self.fresh_reg();
                        self.emit(&format!("  {} =l add {}, 8", env_off, arg_regs[0]));
                        let env_ptr = self.fresh_reg();
                        self.emit(&format!("  {} =l loadl {}", env_ptr, env_off));
                        let r = self.fresh_reg();
                        let rt_name = if method == "filter" {
                            "array_filter"
                        } else {
                            "array_map"
                        };
                        self.emit(&format!(
                            "  {} =l call ${}(l {}, l {}, l {})",
                            r, rt_name, obj_reg, fn_ptr, env_ptr
                        ));
                        return r;
                    }
                    _ => {}
                }
            }

            // Map methods
            if matches!(&obj_type, Some(Type::Map { .. })) || self.is_map_expr(object) {
                if method == "has" {
                    let len_reg = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $string_length(l {})",
                        len_reg, arg_regs[0]
                    ));
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $map_has(l {}, l {}, l {})",
                        r, obj_reg, arg_regs[0], len_reg
                    ));
                    return r;
                }
            }

            // Tensor static constructors
            if obj_name.as_deref() == Some("tensor") {
                if matches!(method.as_str(), "zeros" | "ones" | "randn") {
                    let ndim = self.fresh_reg();
                    self.emit(&format!("  {} =l loadl {}", ndim, arg_regs[0]));
                    let shape_dp_off = self.fresh_reg();
                    self.emit(&format!("  {} =l add {}, 16", shape_dp_off, arg_regs[0]));
                    let shape_ptr = self.fresh_reg();
                    self.emit(&format!("  {} =l loadl {}", shape_ptr, shape_dp_off));
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $tensor_{}(l {}, l {})",
                        r, method, shape_ptr, ndim
                    ));
                    return r;
                }
            }

            // Tensor methods
            if matches!(&obj_type, Some(Type::Tensor { .. })) {
                match method.as_str() {
                    "sum" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =d call $tensor_sum(l {})", r, obj_reg));
                        self.mark_float_reg(&r);
                        return r;
                    }
                    "mean" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =d call $tensor_mean(l {})", r, obj_reg));
                        self.mark_float_reg(&r);
                        return r;
                    }
                    "matmul" => {
                        let r = self.fresh_reg();
                        self.emit(&format!(
                            "  {} =l call $tensor_matmul(l {}, l {})",
                            r, obj_reg, arg_regs[0]
                        ));
                        return r;
                    }
                    "reshape" => {
                        let r = self.fresh_reg();
                        self.emit(&format!(
                            "  {} =l call $tensor_reshape(l {}, l {})",
                            r, obj_reg, arg_regs[0]
                        ));
                        return r;
                    }
                    "print" => {
                        self.emit(&format!("  call $tensor_print(l {})", obj_reg));
                        return "0".to_string();
                    }
                    "shape" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_shape(l {})", r, obj_reg));
                        return r;
                    }
                    "ndim" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_ndim(l {})", r, obj_reg));
                        return r;
                    }
                    "size" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_size(l {})", r, obj_reg));
                        return r;
                    }
                    "relu" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_relu(l {})", r, obj_reg));
                        return r;
                    }
                    "sigmoid" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_sigmoid(l {})", r, obj_reg));
                        return r;
                    }
                    "tanh" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_tanh_act(l {})", r, obj_reg));
                        return r;
                    }
                    "transpose" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_transpose(l {})", r, obj_reg));
                        return r;
                    }
                    "softmax" => {
                        let dim = if !arg_regs.is_empty() {
                            arg_regs[0].clone()
                        } else {
                            "-1".to_string()
                        };
                        let r = self.fresh_reg();
                        self.emit(&format!(
                            "  {} =l call $tensor_softmax(l {}, l {})",
                            r, obj_reg, dim
                        ));
                        return r;
                    }
                    _ => {}
                }
            }

            // Tensor method fallback (when type info is not available)
            match method.as_str() {
                "sum" | "mean" => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =d call $tensor_{}(l {})",
                        r, method, obj_reg
                    ));
                    self.mark_float_reg(&r);
                    return r;
                }
                "matmul" | "reshape" => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $tensor_{}(l {}, l {})",
                        r, method, obj_reg, arg_regs[0]
                    ));
                    return r;
                }
                "transpose" | "relu" | "sigmoid" | "tanh" => {
                    let r = self.fresh_reg();
                    let fn_name = if method == "tanh" {
                        "tensor_tanh_act"
                    } else {
                        &format!("tensor_{}", method)
                    };
                    self.emit(&format!("  {} =l call ${}(l {})", r, fn_name, obj_reg));
                    return r;
                }
                "softmax" => {
                    let dim = if !arg_regs.is_empty() {
                        arg_regs[0].clone()
                    } else {
                        "-1".to_string()
                    };
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $tensor_softmax(l {}, l {})",
                        r, obj_reg, dim
                    ));
                    return r;
                }
                _ => {}
            }

            // Fallback: generic method call
            let all_args_vec: Vec<String> = std::iter::once(format!("l {}", obj_reg))
                .chain(arg_regs.iter().map(|r| format!("l {}", r)))
                .collect();
            let all_args = all_args_vec.join(", ");
            let r = self.fresh_reg();
            self.emit(&format!("  {} =l call ${}({})", r, method, all_args));
            return r;
        }

        // Indirect calls (closures)
        let closure_reg = self.gen_expr(callee);
        let fn_ptr = self.fresh_reg();
        self.emit(&format!("  {} =l loadl {}", fn_ptr, closure_reg));
        let env_off = self.fresh_reg();
        self.emit(&format!("  {} =l add {}, 8", env_off, closure_reg));
        let env_ptr = self.fresh_reg();
        self.emit(&format!("  {} =l loadl {}", env_ptr, env_off));

        let arg_regs: Vec<String> = arguments.iter().map(|a| self.gen_expr(a)).collect();
        let call_args_vec: Vec<String> = std::iter::once(format!("l {}", env_ptr))
            .chain(arg_regs.iter().map(|r| format!("l {}", r)))
            .collect();
        let call_args = call_args_vec.join(", ");
        let r = self.fresh_reg();
        self.emit(&format!("  {} =l call {}({})", r, fn_ptr, call_args));
        r
    }

    fn gen_print_call(&mut self, func_name: &str, args: &[String], arg_exprs: &[Expr]) -> String {
        let arg_reg = if !args.is_empty() {
            &args[0]
        } else {
            &"0".to_string()
        };
        let arg_expr = arg_exprs.first();

        match func_name {
            "print" | "println" => {
                let suffix = if func_name == "println" { "ln" } else { "" };
                if let Some(ae) = arg_expr {
                    if self.is_string_producing_expr(ae) {
                        self.emit(&format!("  call $print{}_string(l {})", suffix, arg_reg));
                    } else if self.is_float_operand(ae)
                        || self.float_regs.contains(arg_reg.as_str())
                    {
                        self.emit(&format!("  call $print{}_f64(d {})", suffix, arg_reg));
                    } else if let ExprKind::Identifier { name } = &ae.kind {
                        if matches!(
                            self.variable_types.get(name.as_str()),
                            Some(Type::Unsigned(_))
                        ) {
                            self.emit(&format!("  call $print{}_u64(l {})", suffix, arg_reg));
                        } else {
                            self.emit(&format!("  call $print{}_i64(l {})", suffix, arg_reg));
                        }
                    } else {
                        self.emit(&format!("  call $print{}_i64(l {})", suffix, arg_reg));
                    }
                } else {
                    self.emit(&format!("  call $print{}_i64(l {})", suffix, arg_reg));
                }
            }
            "print_f64" | "println_f64" => {
                self.emit(&format!("  call ${}(d {})", func_name, arg_reg));
            }
            "print_buffer" => {
                self.emit(&format!(
                    "  call $print_buffer(l {}, l {})",
                    args[0], args[1]
                ));
            }
            _ => {
                self.emit(&format!("  call ${}(l {})", func_name, arg_reg));
            }
        }
        "0".to_string()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn generate_qbe_ir(ast: &Statement) -> String {
    let mut codegen = QBECodeGen::new();
    codegen.generate(ast);
    let mut result = String::new();
    result.push_str(&codegen.data_section);
    result.push_str(&codegen.lambda_funcs);
    result.push_str(&codegen.output);
    result
}

pub fn generate_qbe_ir_with_caps(ast: &Statement, caps: HashMap<String, CapabilityDecl>) -> String {
    let mut codegen = QBECodeGen::new();
    codegen.set_capability_decls(caps);
    codegen.generate(ast);
    let mut result = String::new();
    result.push_str(&codegen.data_section);
    result.push_str(&codegen.lambda_funcs);
    result.push_str(&codegen.output);
    result
}

pub fn generate_plugin_qbe_ir(ast: &Statement, name: &str, version: &str) -> String {
    let mut codegen = QBECodeGen::new();
    codegen.set_plugin_mode(name, version);
    codegen.generate(ast);
    let mut result = String::new();
    result.push_str(&codegen.data_section);
    result.push_str(&codegen.lambda_funcs);
    result.push_str(&codegen.output);
    result
}

pub fn generate_module_qbe_ir(
    ast: &Statement,
    caps: HashMap<String, CapabilityDecl>,
    pkg: &str,
) -> String {
    let mut codegen = QBECodeGen::new();
    codegen.set_capability_decls(caps);
    codegen.package_prefix = format!("{}__", pkg);
    codegen.generate(ast);
    let mut result = String::new();
    result.push_str(&codegen.data_section);
    result.push_str(&codegen.lambda_funcs);
    result.push_str(&codegen.output);
    result
}
