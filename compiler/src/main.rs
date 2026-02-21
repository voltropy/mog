// mogc - The Mog Language Compiler
//
// Usage:
//   mogc <input.mog>                    Compile to ./a.out
//   mogc <input.mog> -o <output>        Compile to specified path
//   mogc <input.mog> --emit-ir          Print QBE IR to stdout
//   mogc <input.mog> -O0|-O1|-O2       Set optimization level
//   mogc <input.mog> --plugin <name>    Compile as plugin (.dylib)

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use mog::compiler::{
    compile, compile_plugin, compile_to_binary, Backend, CompileOptions, OptLevel,
};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        print_usage(&args[0]);
        process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut emit_ir = false;
    let mut opt_level = OptLevel::O0;
    let mut plugin_name: Option<String> = None;
    let mut plugin_version = "0.1.0".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: -o requires an argument");
                    process::exit(1);
                }
                output_path = Some(args[i].clone());
            }
            "--emit-ir" | "--ir" => emit_ir = true,
            "-O0" => opt_level = OptLevel::O0,
            "-O1" => opt_level = OptLevel::O1,
            "-O2" => opt_level = OptLevel::O2,
            "--plugin" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --plugin requires a name");
                    process::exit(1);
                }
                plugin_name = Some(args[i].clone());
            }
            "--plugin-version" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --plugin-version requires a version string");
                    process::exit(1);
                }
                plugin_version = args[i].clone();
            }
            s if s.starts_with('-') => {
                eprintln!("error: unknown option: {s}");
                process::exit(1);
            }
            _ => {
                if input_path.is_some() {
                    eprintln!("error: multiple input files not supported");
                    process::exit(1);
                }
                input_path = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let input = match input_path {
        Some(p) => p,
        None => {
            eprintln!("error: no input file");
            process::exit(1);
        }
    };

    let source = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {input}: {e}");
            process::exit(1);
        }
    };

    // Plugin mode
    if let Some(name) = plugin_name {
        match compile_plugin(&source, &name, &plugin_version) {
            Ok(path) => {
                eprintln!("compiled plugin: {}", path.display());
            }
            Err(errors) => {
                for e in &errors {
                    eprintln!("error: {e}");
                }
                process::exit(1);
            }
        }
        return;
    }

    let source_file = PathBuf::from(&input);

    // IR-only mode
    if emit_ir {
        let options = CompileOptions {
            backend: Backend::Qbe,
            optimization: opt_level,
            source_path: Some(source_file.clone()),
            ..Default::default()
        };
        let result = compile(&source, &options);
        for w in &result.warnings {
            eprintln!("warning: {w}");
        }
        if !result.errors.is_empty() {
            for e in &result.errors {
                eprintln!("error: {e}");
            }
            process::exit(1);
        }
        print!("{}", result.ir);
        return;
    }

    // Full compilation to binary
    let out = output_path.map(PathBuf::from).unwrap_or_else(|| {
        // Default: input stem or "a.out"
        let p = PathBuf::from(&input);
        p.file_stem()
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| PathBuf::from("a.out"))
    });

    let options = CompileOptions {
        backend: Backend::Qbe,
        optimization: opt_level,
        output_path: Some(out.clone()),
        source_path: Some(source_file.clone()),
        ..Default::default()
    };

    match compile_to_binary(&source, &options) {
        Ok(path) => {
            eprintln!("compiled: {}", path.display());
        }
        Err(errors) => {
            for e in &errors {
                eprintln!("error: {e}");
            }
            process::exit(1);
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("mogc - The Mog Language Compiler");
    eprintln!();
    eprintln!("Usage: {program} <input.mog> [options]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o <path>              Output path (default: input stem)");
    eprintln!("  --emit-ir              Print QBE IR to stdout instead of compiling");
    eprintln!("  -O0, -O1, -O2         Optimization level (default: -O0)");
    eprintln!("  --plugin <name>        Compile as shared library plugin");
    eprintln!("  --plugin-version <v>   Plugin version (default: 0.1.0)");
    eprintln!("  -h, --help             Show this help");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {program} hello.mog                   Compile to ./hello");
    eprintln!("  {program} hello.mog -o build/hello     Compile to build/hello");
    eprintln!("  {program} hello.mog --emit-ir          Print QBE IR");
    eprintln!("  {program} hello.mog -O1                Compile with optimization");
}
