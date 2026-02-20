// QBE Backend Code Generator for Mog
// Generates QBE IL from analyzed AST, targeting the QBE compiler backend
// QBE base types: w (i32), l (i64), s (f32), d (f64). All pointers are l.

use std::collections::{HashMap, HashSet};
use crate::ast::*;
use crate::types::{Type, FloatKind};
use crate::capability::CapabilityDecl;

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
    coro_frame_size: u32,
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
            coro_frame_size: 0,
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
        if t == "void" { "" } else { t }
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
        self.string_constants.insert(value.to_string(), name.clone());
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
            ExprKind::NumberLiteral { literal_type: Some(ty), .. } => ty.is_float(),
            ExprKind::NumberLiteral { value, literal_type: None } => value.contains('.'),
            ExprKind::Identifier { name } => {
                matches!(self.variable_types.get(name.as_str()), Some(t) if t.is_float())
            }
            ExprKind::MemberExpression { object, property } => {
                if let ExprKind::Identifier { name } = &object.kind {
                    if let Some(Type::Struct { name: sname, .. }) = self.variable_types.get(name.as_str()) {
                        if let Some(fields) = self.struct_fields.get(sname.as_str()) {
                            return fields.iter().any(|(n, t)| n == property && t.is_float());
                        }
                    }
                }
                false
            }
            ExprKind::IndexExpression { object, .. } => {
                if let ExprKind::Identifier { name } = &object.kind {
                    if let Some(Type::Array { element_type, .. }) = self.variable_types.get(name.as_str()) {
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
                    let math_funcs: HashSet<&str> = ["sqrt","sin","cos","tan","asin","acos","atan2","exp","log","log2","floor","ceil","round","fabs","pow","fmin","fmax"].iter().copied().collect();
                    if math_funcs.contains(name.as_str()) { return true; }
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
                    matches!(name.as_str(), "str" | "i64_to_string" | "u64_to_string" | "f64_to_string" | "input_string")
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
                matches!(self.variable_types.get(name.as_str()), Some(Type::Array { .. }))
            }
            _ => false,
        }
    }

    fn is_map_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::MapLiteral { .. } => true,
            ExprKind::Identifier { name } => {
                matches!(self.variable_types.get(name.as_str()), Some(Type::Map { .. }))
            }
            _ => false,
        }
    }

    fn infer_expression_type(&self, expr: &Expr) -> &'static str {
        if self.is_string_producing_expr(expr) { "string" }
        else if self.is_float_operand(expr) { "float" }
        else { "int" }
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
            ExprKind::NumberLiteral { value, literal_type } => self.gen_number_literal(value, literal_type.as_ref()),
            ExprKind::BooleanLiteral { value } => if *value { "1".to_string() } else { "0".to_string() },
            ExprKind::NoneExpression => "0".to_string(),
            ExprKind::StringLiteral { value } => self.gen_string_literal(value),
            ExprKind::TemplateLiteral { parts } => {
                let parts = parts.clone();
                self.gen_template_literal(&parts)
            }
            ExprKind::Identifier { name } => self.gen_identifier(name),
            ExprKind::BinaryExpression { operator, left, right } => {
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
            ExprKind::CallExpression { callee, arguments, named_args } => {
                let args = arguments.clone();
                let na = named_args.clone();
                self.gen_call_expr(callee, &args, &na)
            }
            ExprKind::AssignmentExpression { name, target, value } => {
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
            ExprKind::StructLiteral { struct_name, fields } => {
                let sn = struct_name.clone();
                let fs = fields.clone();
                self.gen_struct_literal(&sn, &fs)
            }
            ExprKind::SoAConstructor { struct_name, capacity } => {
                let sn = struct_name.clone();
                self.gen_soa_constructor(&sn, *capacity)
            }
            ExprKind::MemberExpression { object, property } => {
                let prop = property.clone();
                self.gen_member_expr(object, &prop)
            }
            ExprKind::IndexExpression { object, index } => self.gen_index_expr(object, index),
            ExprKind::SliceExpression { object, start, end, .. } => self.gen_slice_expr(object, start, end),
            ExprKind::CastExpression { target_type, value, source_type } => {
                let tt = target_type.clone();
                let st = source_type.clone();
                self.gen_cast_expr(&tt, value, &st)
            }
            ExprKind::IfExpression { condition, true_branch, false_branch } => {
                let tb = true_branch.clone();
                let fb = false_branch.clone();
                self.gen_if_expr(condition, &tb, &fb)
            }
            ExprKind::MatchExpression { subject, arms } => {
                let a = arms.clone();
                self.gen_match_expr(subject, &a)
            }
            ExprKind::Lambda { params, return_type, body } => {
                let p = params.clone();
                let rt = return_type.clone();
                let b = body.clone();
                self.gen_lambda(&p, &rt, &b)
            }
            ExprKind::OkExpression { value } => self.gen_ok_expr(value),
            ExprKind::ErrExpression { value } => self.gen_err_expr(value),
            ExprKind::SomeExpression { value } => self.gen_some_expr(value),
            ExprKind::NoneExpression => self.gen_none_expr(),
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

    fn gen_binary_expr(&mut self, op: &str, left: &Expr, right: &Expr, is_float: bool, is_str_left: bool, is_str_right: bool) -> String {
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
            self.emit(&format!("  {} =l call $string_concat(l {}, l {})", reg, l, r));
            return reg;
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
                "+" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =d add {}, {}", reg, l, r)); return reg; }
                "-" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =d sub {}, {}", reg, l, r)); return reg; }
                "*" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =d mul {}, {}", reg, l, r)); return reg; }
                "/" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =d div {}, {}", reg, l, r)); return reg; }
                _ => {}
            }

            let cmp_op = match op {
                "==" => Some("ceqd"), "!=" => Some("cned"),
                "<" => Some("cltd"), ">" => Some("cgtd"),
                "<=" => Some("cled"), ">=" => Some("cged"),
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
            "+" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l add {}, {}", reg, l, r)); return reg; }
            "-" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l sub {}, {}", reg, l, r)); return reg; }
            "*" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l mul {}, {}", reg, l, r)); return reg; }
            "/" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l div {}, {}", reg, l, r)); return reg; }
            "%" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l rem {}, {}", reg, l, r)); return reg; }
            _ => {}
        }

        // Integer comparisons
        let int_cmp = match op {
            "==" => Some("ceql"), "!=" => Some("cnel"),
            "<" => Some("csltl"), ">" => Some("csgtl"),
            "<=" => Some("cslel"), ">=" => Some("csgel"),
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
            "&" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l and {}, {}", reg, l, r)); return reg; }
            "|" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l or {}, {}", reg, l, r)); return reg; }
            "^" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l xor {}, {}", reg, l, r)); return reg; }
            "<<" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l shl {}, {}", reg, l, r)); return reg; }
            ">>" => { let reg = self.fresh_reg(); self.emit(&format!("  {} =l sar {}, {}", reg, l, r)); return reg; }
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
    fn gen_call_expr(&mut self, callee: &Expr, arguments: &[Expr], named_args: &Option<Vec<NamedArg>>) -> String {
        // Direct identifier calls
        if let ExprKind::Identifier { name } = &callee.kind {
            let name = name.clone();

            // Generate all argument registers
            let arg_regs: Vec<String> = arguments.iter().map(|a| self.gen_expr(a)).collect();

            // Print functions
            let print_funcs = ["print", "println", "print_string", "println_string",
                "print_i64", "println_i64", "print_f64", "println_f64",
                "print_u64", "println_u64", "print_buffer"];
            if print_funcs.contains(&name.as_str()) {
                return self.gen_print_call(&name, &arg_regs, arguments);
            }

            // Math builtins (single-arg)
            let math_single = ["sqrt", "sin", "cos", "tan", "asin", "acos",
                "exp", "log", "log2", "floor", "ceil", "round", "abs"];
            if math_single.contains(&name.as_str()) {
                let r = self.fresh_reg();
                let qbe_name = if name == "abs" { "fabs" } else { &name };
                self.emit(&format!("  {} =d call ${}(d {})", r, qbe_name, arg_regs[0]));
                return r;
            }

            // Math builtins (two-arg)
            let math_double: HashMap<&str, &str> = [("pow","pow"),("atan2","atan2"),("min","fmin"),("max","fmax"),("fmin","fmin"),("fmax","fmax")].iter().copied().collect();
            if let Some(qbe_name) = math_double.get(name.as_str()) {
                let r = self.fresh_reg();
                self.emit(&format!("  {} =d call ${}(d {}, d {})", r, qbe_name, arg_regs[0], arg_regs[1]));
                return r;
            }

            // Conversion functions
            match name.as_str() {
                "str" | "i64_to_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $i64_to_string(l {})", r, arg_regs[0]));
                    return r;
                }
                "u64_to_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $u64_to_string(l {})", r, arg_regs[0]));
                    return r;
                }
                "f64_to_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $f64_to_string(d {})", r, arg_regs[0]));
                    return r;
                }
                "int_from_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $int_from_string(l {})", r, arg_regs[0]));
                    return r;
                }
                "float_from_string" => {
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =d call $float_from_string(l {})", r, arg_regs[0]));
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
                    self.emit(&format!("  {} =l call $tensor_sigmoid(l {})", r, arg_regs[0]));
                    return r;
                }
                "softmax" => {
                    let dim_arg = if arg_regs.len() > 1 { &arg_regs[1] } else { &"-1".to_string() };
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $tensor_softmax(l {}, l {})", r, arg_regs[0], dim_arg));
                    return r;
                }
                _ => {}
            }

            // General function calls
            let ret_type = self.function_types.get(&name).cloned();
            let qbe_ret = ret_type.as_ref().map(|t| self.to_qbe_type(t)).unwrap_or("l");

            let arg_list: String = arg_regs.iter().map(|r| format!("l {}", r)).collect::<Vec<_>>().join(", ");

            if qbe_ret == "void" {
                self.emit(&format!("  call ${}({})", name, arg_list));
                return "0".to_string();
            }

            let r = self.fresh_reg();
            let ret_str = if qbe_ret == "d" || qbe_ret == "s" { qbe_ret } else { "l" };
            self.emit(&format!("  {} ={} call ${}({})", r, ret_str, name, arg_list));
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
                    let all_args: String = arg_regs.iter().map(|r| format!("l {}", r)).collect::<Vec<_>>().join(", ");
                    let ret_type = self.function_types.get(&qualified).cloned();
                    let qbe_ret = ret_type.as_ref().map(|t| self.to_qbe_type(t)).unwrap_or("l");
                    if qbe_ret == "void" {
                        self.emit(&format!("  call ${}({})", qualified, all_args));
                        return "0".to_string();
                    }
                    let r = self.fresh_reg();
                    let ret_str = if qbe_ret == "d" || qbe_ret == "s" { qbe_ret } else { "l" };
                    self.emit(&format!("  {} ={} call ${}({})", r, ret_str, qualified, all_args));
                    return r;
                }

                // Capability calls
                if self.capabilities.contains(oname.as_str()) {
                    return self.gen_capability_call(oname, &method, &arg_regs, arguments);
                }
            }

            // String methods
            if obj_name.as_ref().map_or(false, |n| matches!(self.variable_types.get(n.as_str()), Some(Type::String))) || self.is_string_producing_expr(object) {
                let string_methods: HashMap<&str, (&str, bool)> = [
                    ("upper", ("string_upper", false)), ("lower", ("string_lower", false)),
                    ("trim", ("string_trim", false)), ("split", ("string_split", true)),
                    ("contains", ("string_contains", true)), ("starts_with", ("string_starts_with", true)),
                    ("ends_with", ("string_ends_with", true)), ("replace", ("string_replace", true)),
                    ("len", ("string_len", false)), ("index_of", ("string_index_of", true)),
                ].iter().copied().collect();
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
            let obj_type = obj_name.as_ref().and_then(|n| self.variable_types.get(n.as_str())).cloned();
            if matches!(&obj_type, Some(Type::Array { .. })) || self.is_array_expr(object) {
                match method.as_str() {
                    "push" => {
                        self.emit(&format!("  call $array_push(l {}, l {})", obj_reg, arg_regs[0]));
                        return "0".to_string();
                    }
                    "pop" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $array_pop(l {})", r, obj_reg));
                        return r;
                    }
                    "contains" => {
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $array_contains(l {}, l {})", r, obj_reg, arg_regs[0]));
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
                            self.emit(&format!("  call $array_sort_with_comparator(l {}, l {}, l {})", obj_reg, fn_ptr, env_ptr));
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
                        self.emit(&format!("  {} =l call $array_join(l {}, l {})", r, obj_reg, arg_regs[0]));
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
                        let rt_name = if method == "filter" { "array_filter" } else { "array_map" };
                        self.emit(&format!("  {} =l call ${}(l {}, l {}, l {})", r, rt_name, obj_reg, fn_ptr, env_ptr));
                        return r;
                    }
                    _ => {}
                }
            }

            // Map methods
            if matches!(&obj_type, Some(Type::Map { .. })) || self.is_map_expr(object) {
                if method == "has" {
                    let len_reg = self.fresh_reg();
                    self.emit(&format!("  {} =l call $string_length(l {})", len_reg, arg_regs[0]));
                    let r = self.fresh_reg();
                    self.emit(&format!("  {} =l call $map_has(l {}, l {}, l {})", r, obj_reg, arg_regs[0], len_reg));
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
                    self.emit(&format!("  {} =l call $tensor_{}(l {}, l {})", r, method, shape_ptr, ndim));
                    return r;
                }
            }

            // Tensor methods
            if matches!(&obj_type, Some(Type::Tensor { .. })) {
                match method.as_str() {
                    "sum" => { let r = self.fresh_reg(); self.emit(&format!("  {} =d call $tensor_sum(l {})", r, obj_reg)); return r; }
                    "mean" => { let r = self.fresh_reg(); self.emit(&format!("  {} =d call $tensor_mean(l {})", r, obj_reg)); return r; }
                    "matmul" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_matmul(l {}, l {})", r, obj_reg, arg_regs[0])); return r; }
                    "reshape" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_reshape(l {}, l {})", r, obj_reg, arg_regs[0])); return r; }
                    "print" => { self.emit(&format!("  call $tensor_print(l {})", obj_reg)); return "0".to_string(); }
                    "shape" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_shape(l {})", r, obj_reg)); return r; }
                    "ndim" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_ndim(l {})", r, obj_reg)); return r; }
                    "size" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_size(l {})", r, obj_reg)); return r; }
                    "relu" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_relu(l {})", r, obj_reg)); return r; }
                    "sigmoid" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_sigmoid(l {})", r, obj_reg)); return r; }
                    "tanh" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_tanh_act(l {})", r, obj_reg)); return r; }
                    "transpose" => { let r = self.fresh_reg(); self.emit(&format!("  {} =l call $tensor_transpose(l {})", r, obj_reg)); return r; }
                    "softmax" => {
                        let dim = if !arg_regs.is_empty() { arg_regs[0].clone() } else { "-1".to_string() };
                        let r = self.fresh_reg();
                        self.emit(&format!("  {} =l call $tensor_softmax(l {}, l {})", r, obj_reg, dim));
                        return r;
                    }
                    _ => {}
                }
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
        let arg_reg = if !args.is_empty() { &args[0] } else { &"0".to_string() };
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
                        if matches!(self.variable_types.get(name.as_str()), Some(Type::Unsigned(_))) {
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
                self.emit(&format!("  call $print_buffer(l {}, l {})", args[0], args[1]));
            }
            _ => {
                self.emit(&format!("  call ${}(l {})", func_name, arg_reg));
            }
        }
        "0".to_string()
    }

