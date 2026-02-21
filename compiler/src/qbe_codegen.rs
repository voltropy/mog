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
        let bits = value.to_bits();
        format!("d_{:016x}", bits)
    }

    fn float_to_single_qbe(value: f32) -> String {
        let bits = value.to_bits();
        format!("s_{:08x}", bits)
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
                matches!(self.variable_types.get(name.as_str()), Some(t) if t.is_float())
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
            ExprKind::Identifier { name } => {
                matches!(self.variable_types.get(name.as_str()), Some(Type::String))
            }
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
            ExprKind::NoneExpression => "0".to_string(),
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
            ExprKind::IsSomeExpression { value, .. } => self.gen_is_some_expr(value),
            ExprKind::IsNoneExpression { value } => self.gen_is_none_expr(value),
            ExprKind::IsOkExpression { value, .. } => self.gen_is_ok_expr(value),
            ExprKind::IsErrExpression { value, .. } => self.gen_is_err_expr(value),
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
                    } else if self.is_float_operand(e) {
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
        if let Some(n) = name {
            // Simple variable assignment or walrus declaration
            if let Some(reg) = self.variable_registers.get(n).cloned() {
                self.emit(&format!("    storel {}, {}", val, reg));
            } else {
                // New variable declaration via :=
                let slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", slot));
                self.emit(&format!("    storel {}, {}", val, slot));
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
                    let obj = self.gen_expr(object);
                    if let Some(sname) = self.get_struct_type_for_expr(object) {
                        if let Some(fields) = self.struct_fields.get(&sname).cloned() {
                            if let Some(idx) = fields.iter().position(|(n, _)| n == property) {
                                let off = self.fresh_reg();
                                self.emit(&format!("    {} =l add {}, {}", off, obj, idx * 8));
                                self.emit(&format!("    storel {}, {}", val, off));
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

    fn gen_array_literal(&mut self, elements: &[Expr]) -> String {
        let arr = self.fresh_reg();
        self.emit(&format!(
            "    {} =l call $array_new(l 8, l {})",
            arr,
            elements.len()
        ));
        for elem in elements {
            let v = self.gen_expr(elem);
            self.emit(&format!("    call $array_push(l {}, l {})", arr, v));
        }
        arr
    }

    fn gen_array_fill(&mut self, value: &Expr, count: &Expr) -> String {
        let cnt = self.gen_expr(count);
        let arr = self.fresh_reg();
        self.emit(&format!("    {} =l call $array_new(l 8, l {})", arr, cnt));
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
        self.emit(&format!("    {} =l call $map_new()", m));
        for entry in entries {
            let key_label = self.register_string(&entry.key);
            let val = self.gen_expr(&entry.value);
            self.emit(&format!(
                "    call $map_set(l {}, l {}, l {})",
                m, key_label, val
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
            if i == 0 {
                self.emit(&format!("    storel {}, {}", val, ptr));
            } else {
                let off = self.fresh_reg();
                self.emit(&format!("    {} =l add {}, {}", off, ptr, i * 8));
                self.emit(&format!("    storel {}, {}", val, off));
            }
        }
        if let Some(sn) = struct_name {
            // Record struct type for this allocation (done at call site via variable_types)
            let _ = sn;
        }
        ptr
    }

    fn gen_soa_constructor(&mut self, _struct_name: &str, capacity: Option<usize>) -> String {
        let cap = capacity.unwrap_or(0) as i64;
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l {})", r, cap * 8 + 16));
        r
    }

    fn gen_member_expr(&mut self, object: &Expr, property: &str) -> String {
        // Check for .len/.cap on arrays/strings
        if property == "len" {
            let obj = self.gen_expr(object);
            if self.is_string_producing_expr(object) {
                let r = self.fresh_reg();
                self.emit(&format!("    {} =l call $string_length(l {})", r, obj));
                return r;
            }
            // Array .len
            let r = self.fresh_reg();
            self.emit(&format!("    {} =l loadl {}", r, obj));
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
                    if idx == 0 {
                        let r = self.fresh_reg();
                        self.emit(&format!("    {} =l loadl {}", r, obj));
                        return r;
                    }
                    let off = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", off, obj, idx * 8));
                    let r = self.fresh_reg();
                    self.emit(&format!("    {} =l loadl {}", r, off));
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
            self.emit(&format!("    {} =l call $map_get(l {}, l {})", r, obj, idx));
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
        self.emit(&format!("    storel {}, {}", last_val, result_slot));
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
        self.emit(&format!("    storel {}, {}", false_val, result_slot));
        self.emit(&format!("    jmp {}", end_lbl));
        // end
        self.emit(&format!("{}", end_lbl));
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", r, result_slot));
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
                        let body_val = self.gen_expr(&arm.body);
                        self.emit(&format!("    storel {}, {}", body_val, result_slot));
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
            self.emit(&format!("    storel {}, {}", body_val, result_slot));
            self.emit(&format!("    jmp {}", end_lbl));
            if i + 1 < arms.len() && !matches!(arm.pattern, MatchPattern::WildcardPattern) {
                self.emit(&format!("{}", next_lbl));
            }
        }
        self.emit(&format!("{}", end_lbl));
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", r, result_slot));
        r
    }

    fn gen_lambda(&mut self, params: &[FunctionParam], return_type: &Type, body: &Block) -> String {
        let lambda_name = format!("lambda_{}", self.lambda_counter);
        self.lambda_counter += 1;
        // Find captured variables
        let captured = self.find_captured_vars(body, params);
        // Generate lambda function
        let mut param_str = String::from("l %env");
        for p in params {
            param_str.push_str(&format!(", l %p_{}", p.name));
        }
        let ret = self.ret_type_str(return_type);
        let mut func_ir = format!("function {} ${}({}) {{\n", ret, lambda_name, param_str);
        func_ir.push_str("@start\n");
        // Alloc slots for params
        for p in params {
            func_ir.push_str(&format!("    %s_{} =l alloc8 8\n", p.name));
            func_ir.push_str(&format!("    storel 0, %s_{}\n", p.name));
            func_ir.push_str(&format!("    storel %p_{}, %s_{}\n", p.name, p.name));
        }
        // Reload captures from env
        for (i, cap) in captured.iter().enumerate() {
            if i == 0 {
                func_ir.push_str(&format!("    %cap_{} =l loadl %env\n", cap));
            } else {
                func_ir.push_str(&format!("    %off_cap_{} =l add %env, {}\n", cap, i * 8));
                func_ir.push_str(&format!("    %cap_{} =l loadl %off_cap_{}\n", cap, cap));
            }
            func_ir.push_str(&format!("    %s_{} =l alloc8 8\n", cap));
            func_ir.push_str(&format!("    storel 0, %s_{}\n", cap));
            func_ir.push_str(&format!("    storel %cap_{}, %s_{}\n", cap, cap));
        }
        // We need a sub-codegen for the body
        func_ir.push_str("    ret 0\n");
        func_ir.push_str("}\n\n");
        self.lambda_funcs.push_str(&func_ir);
        // Build closure {fn_ptr, env_ptr}
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
                if let Some(reg) = self.variable_registers.get(cap).cloned() {
                    let val = self.fresh_reg();
                    self.emit(&format!("    {} =l loadl {}", val, reg));
                    if i == 0 {
                        self.emit(&format!("    storel {}, {}", val, env));
                    } else {
                        let off = self.fresh_reg();
                        self.emit(&format!("    {} =l add {}, {}", off, env, i * 8));
                        self.emit(&format!("    storel {}, {}", val, off));
                    }
                }
            }
            let ep = self.fresh_reg();
            self.emit(&format!("    {} =l add {}, 8", ep, cls));
            self.emit(&format!("    storel {}, {}", env, ep));
        }
        cls
    }

    fn find_captured_vars(&self, block: &Block, params: &[FunctionParam]) -> Vec<String> {
        let param_names: HashSet<_> = params.iter().map(|p| p.name.clone()).collect();
        let mut captured = Vec::new();
        for name in self.variable_registers.keys() {
            if !param_names.contains(name) {
                captured.push(name.clone());
            }
        }
        let _ = block; // Would walk AST to find used vars; simplified
        captured
    }

    fn gen_ok_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
        self.emit(&format!("    storel 0, {}", r)); // tag=0 for Ok
        let pay = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", pay, r));
        self.emit(&format!("    storel {}, {}", val, pay));
        r
    }

    fn gen_err_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
        self.emit(&format!("    storel 1, {}", r)); // tag=1 for Err
        let pay = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", pay, r));
        self.emit(&format!("    storel {}, {}", val, pay));
        r
    }

    fn gen_some_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let r = self.fresh_reg();
        self.emit(&format!("    {} =l call $gc_alloc(l 16)", r));
        self.emit(&format!("    storel 0, {}", r)); // tag=0 for Some
        let pay = self.fresh_reg();
        self.emit(&format!("    {} =l add {}, 8", pay, r));
        self.emit(&format!("    storel {}, {}", val, pay));
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

    fn gen_is_some_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 0", cmp_w, tag));
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        result
    }

    fn gen_is_none_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 1", cmp_w, tag));
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        result
    }

    fn gen_is_ok_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 0", cmp_w, tag));
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        result
    }

    fn gen_is_err_expr(&mut self, value: &Expr) -> String {
        let val = self.gen_expr(value);
        let tag = self.fresh_reg();
        self.emit(&format!("    {} =l loadl {}", tag, val));
        let cmp_w = self.fresh_reg();
        self.emit(&format!("    {} =w ceql {}, 1", cmp_w, tag));
        let result = self.fresh_reg();
        self.emit(&format!("    {} =l extsw {}", result, cmp_w));
        result
    }

    fn gen_await_expr(&mut self, argument: &Expr) -> String {
        if !self.in_async_function {
            // Not in async context: just evaluate the argument
            return self.gen_expr(argument);
        }
        let future_val = self.gen_expr(argument);
        let await_idx = self.await_counter;
        self.await_counter += 1;
        let cont_lbl = format!("@await{}.cont", await_idx);
        self.coro_resume_blocks.push(cont_lbl.clone());
        // Spill all locals to frame
        if let Some(frame) = self.coro_frame.clone() {
            let mut offset = CORO_HEADER_SIZE;
            for var_name in self.coro_spilled_vars.clone() {
                if let Some(reg) = self.variable_registers.get(&var_name).cloned() {
                    let val = self.fresh_reg();
                    self.emit(&format!("    {} =l loadl {}", val, reg));
                    let off = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", off, frame, offset));
                    self.emit(&format!("    storel {}, {}", val, off));
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
            for var_name in self.coro_spilled_vars.clone() {
                if let Some(reg) = self.variable_registers.get(&var_name).cloned() {
                    let roff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", roff, frame, reload_offset));
                    let rval = self.fresh_reg();
                    self.emit(&format!("    {} =l loadl {}", rval, roff));
                    self.emit(&format!("    storel {}, {}", rval, reg));
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
        // Simplified: call tensor_new or tensor_ones/zeros based on args
        if args.is_empty() {
            let r = self.fresh_reg();
            self.emit(&format!("    {} =l call $tensor_new(l 0, l 0)", r));
            return r;
        }
        let shape = self.gen_expr(&args[0]);
        if args.len() > 1 {
            let data = self.gen_expr(&args[1]);
            let r = self.fresh_reg();
            self.emit(&format!(
                "    {} =l call $tensor_from_data(l {}, l {})",
                r, shape, data
            ));
            r
        } else {
            let r = self.fresh_reg();
            self.emit(&format!("    {} =l call $tensor_zeros(l {})", r, shape));
            r
        }
    }

    fn gen_capability_call(
        &mut self,
        cap_name: &str,
        method: &str,
        arg_regs: &[String],
        _arguments: &[Expr],
    ) -> String {
        let nargs = arg_regs.len();
        let cap_str = self.register_string(cap_name);
        let method_str = self.register_string(method);
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
                if base == 0 {
                    self.emit(&format!("    storel 0, {}", args_buf)); // tag=MOG_INT
                    let doff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, 8", doff, args_buf));
                    self.emit(&format!("    storel {}, {}", arg, doff));
                } else {
                    let toff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", toff, args_buf, base));
                    self.emit(&format!("    storel 0, {}", toff)); // tag
                    let doff = self.fresh_reg();
                    self.emit(&format!("    {} =l add {}, {}", doff, args_buf, base + 8));
                    self.emit(&format!("    storel {}, {}", arg, doff));
                }
            }
            self.emit(&format!(
                "    call $mog_cap_call_out(l {}, l 0, l {}, l {}, l {}, w {})",
                out, cap_str, method_str, args_buf, nargs
            ));
        } else {
            self.emit(&format!(
                "    call $mog_cap_call_out(l {}, l 0, l {}, l {}, l 0, w 0)",
                out, cap_str, method_str
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
                    self.emit(&format!("    storel {}, {}", v, slot));
                }
                self.variable_registers.insert(name.clone(), slot);
                if let Some(t) = var_type {
                    self.variable_types.insert(name.clone(), t.clone());
                }
            }
            StatementKind::Assignment { name, value } => {
                let val = self.gen_expr(value);
                if let Some(reg) = self.variable_registers.get(name).cloned() {
                    self.emit(&format!("    storel {}, {}", val, reg));
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
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), test_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let i = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", i, slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, i, e));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                self.gen_block_stmts(&body.statements);
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
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), test_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let i = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", i, slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, i, e));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                self.gen_block_stmts(&body.statements);
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
                self.emit(&format!("    {} =l loadl {}", len, arr));
                let idx_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", idx_slot));
                self.emit(&format!("    storel 0, {}", idx_slot));
                let val_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", val_slot));
                self.variable_registers
                    .insert(variable.clone(), val_slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), test_lbl.clone()));
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
                let arr = self.gen_expr(iterable);
                let len = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", len, arr));
                let idx_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", idx_slot));
                self.emit(&format!("    storel 0, {}", idx_slot));
                self.variable_registers
                    .insert(index_variable.clone(), idx_slot.clone());
                let val_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", val_slot));
                self.variable_registers
                    .insert(value_variable.clone(), val_slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), test_lbl.clone()));
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
            StatementKind::ForInMap {
                key_variable,
                value_variable,
                map,
                body,
            } => {
                let m = self.gen_expr(map);
                let keys = self.fresh_reg();
                self.emit(&format!("    {} =l call $map_keys(l {})", keys, m));
                let len = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", len, keys));
                let idx_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", idx_slot));
                self.emit(&format!("    storel 0, {}", idx_slot));
                let key_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", key_slot));
                self.variable_registers
                    .insert(key_variable.clone(), key_slot.clone());
                let val_slot = self.fresh_reg();
                self.emit_alloc(&format!("    {} =l alloc8 8", val_slot));
                self.variable_registers
                    .insert(value_variable.clone(), val_slot.clone());
                let test_lbl = self.fresh_label();
                let body_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.loop_stack.push((end_lbl.clone(), test_lbl.clone()));
                self.emit(&format!("    jmp {}", test_lbl));
                self.emit(&format!("{}", test_lbl));
                let idx = self.fresh_reg();
                self.emit(&format!("    {} =l loadl {}", idx, idx_slot));
                let cmp = self.fresh_reg();
                self.emit(&format!("    {} =w csltl {}, {}", cmp, idx, len));
                self.emit(&format!("    jnz {}, {}, {}", cmp, body_lbl, end_lbl));
                self.emit(&format!("{}", body_lbl));
                let key = self.fresh_reg();
                self.emit(&format!(
                    "    {} =l call $array_get(l {}, l {})",
                    key, keys, idx
                ));
                self.emit(&format!("    storel {}, {}", key, key_slot));
                let val = self.fresh_reg();
                self.emit(&format!("    {} =l call $map_get(l {}, l {})", val, m, key));
                self.emit(&format!("    storel {}, {}", val, val_slot));
                self.gen_block_stmts(&body.statements);
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
        self.block_terminated = false;
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
            param_strs.push(format!("l %p.{}", p.name));
        }
        let params_joined = param_strs.join(", ");

        // Emit param allocs and stores
        for p in params {
            let slot = self.fresh_reg();
            self.emit_alloc(&format!("    {} =l alloc8 8", slot));
            self.emit(&format!("    storel %p.{}, {}", p.name, slot));
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
        // For now, generate as sync function that returns a future
        self.gen_function_decl(name, params, return_type, body, is_public);
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

    fn gen_number_literal(&self, value: &str, literal_type: Option<&Type>) -> String {
        if let Some(ty) = literal_type {
            match ty {
                Type::Float(FloatKind::F32) => {
                    if let Ok(v) = value.parse::<f32>() {
                        return Self::float_to_single_qbe(v);
                    }
                }
                Type::Float(_) => {
                    if let Ok(v) = value.parse::<f64>() {
                        return Self::float_to_qbe(v);
                    }
                }
                _ => {}
            }
        }
        if value.contains('.') {
            if let Ok(v) = value.parse::<f64>() {
                return Self::float_to_qbe(v);
            }
        }
        value.to_string()
    }

    fn gen_string_literal(&mut self, value: &str) -> String {
        self.register_string(value)
    }

    fn gen_identifier(&mut self, name: &str) -> String {
        if let Some(slot) = self.variable_registers.get(name).cloned() {
            let reg = self.fresh_reg();
            let is_float = matches!(self.variable_types.get(name), Some(t) if t.is_float());
            if is_float {
                self.emit(&format!("  {} =d loadd {}", reg, slot));
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

        let mut l = self.gen_expr(left);
        let mut r = self.gen_expr(right);

        if is_float {
            if !self.is_float_operand(left) {
                let tmp = self.fresh_reg();
                self.emit(&format!("  {} =d swtof {}", tmp, l));
                l = tmp;
            }
            if !self.is_float_operand(right) {
                let tmp = self.fresh_reg();
                self.emit(&format!("  {} =d swtof {}", tmp, r));
                r = tmp;
            }

            match op {
                "+" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d add {}, {}", reg, l, r));
                    return reg;
                }
                "-" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d sub {}, {}", reg, l, r));
                    return reg;
                }
                "*" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d mul {}, {}", reg, l, r));
                    return reg;
                }
                "/" => {
                    let reg = self.fresh_reg();
                    self.emit(&format!("  {} =d div {}, {}", reg, l, r));
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
                return r;
            }

            // Conversion functions
            match name.as_str() {
                "str" | "i64_to_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!(
                        "  {} =l call $i64_to_string(l {})",
                        r, arg_regs[0]
                    ));
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
                _ => {}
            }

            // General function calls
            let ret_type = self.function_types.get(&name).cloned();
            let qbe_ret = ret_type
                .as_ref()
                .map(|t| self.to_qbe_type(t))
                .unwrap_or("l");

            let arg_list: String = arg_regs
                .iter()
                .map(|r| format!("l {}", r))
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
                    let all_args: String = arg_regs
                        .iter()
                        .map(|r| format!("l {}", r))
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
                    return r;
                }

                // Capability calls
                if self.capabilities.contains(oname.as_str()) {
                    return self.gen_capability_call(oname, &method, &arg_regs, arguments);
                }
            }

            // String methods
            if obj_name.as_ref().map_or(false, |n| {
                matches!(self.variable_types.get(n.as_str()), Some(Type::String))
            }) || self.is_string_producing_expr(object)
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
                        self.emit(&format!(
                            "  call $array_push(l {}, l {})",
                            obj_reg, arg_regs[0]
                        ));
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
                        return r;
                    }
                    "mean" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =d call $tensor_mean(l {})", r, obj_reg));
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
                    } else if self.is_float_operand(ae) {
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
