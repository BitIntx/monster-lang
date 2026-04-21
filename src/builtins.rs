use crate::ast::Type;

#[derive(Clone)]
pub struct RuntimeBuiltin {
    pub name: &'static str,
    pub params: Vec<Type>,
    pub ret_type: Type,
    pub llvm_name: &'static str,
}

pub fn runtime_builtins() -> Vec<RuntimeBuiltin> {
    vec![
        RuntimeBuiltin {
            name: "print_i32",
            params: vec![Type::I32],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_print_i32",
        },
        RuntimeBuiltin {
            name: "print_bool",
            params: vec![Type::Bool],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_print_bool",
        },
        RuntimeBuiltin {
            name: "print_str",
            params: vec![Type::Str],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_print_str",
        },
        RuntimeBuiltin {
            name: "print_ln_i32",
            params: vec![Type::I32],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_print_ln_i32",
        },
        RuntimeBuiltin {
            name: "print_ln_bool",
            params: vec![Type::Bool],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_print_ln_bool",
        },
        RuntimeBuiltin {
            name: "print_ln_str",
            params: vec![Type::Str],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_print_ln_str",
        },
        RuntimeBuiltin {
            name: "read_i32",
            params: vec![],
            ret_type: Type::I32,
            llvm_name: "@__monster_builtin_read_i32",
        },
        RuntimeBuiltin {
            name: "read_file",
            params: vec![Type::Str, Type::Ptr(Box::new(Type::USize))],
            ret_type: Type::Ptr(Box::new(Type::U8)),
            llvm_name: "@__monster_builtin_read_file",
        },
        RuntimeBuiltin {
            name: "write_file",
            params: vec![Type::Str, Type::Ptr(Box::new(Type::U8)), Type::USize],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_write_file",
        },
        RuntimeBuiltin {
            name: "strlen",
            params: vec![Type::Str],
            ret_type: Type::USize,
            llvm_name: "@__monster_builtin_strlen",
        },
        RuntimeBuiltin {
            name: "memcmp",
            params: vec![
                Type::Ptr(Box::new(Type::U8)),
                Type::Ptr(Box::new(Type::U8)),
                Type::USize,
            ],
            ret_type: Type::I32,
            llvm_name: "@__monster_builtin_memcmp",
        },
        RuntimeBuiltin {
            name: "memcpy",
            params: vec![
                Type::Ptr(Box::new(Type::U8)),
                Type::Ptr(Box::new(Type::U8)),
                Type::USize,
            ],
            ret_type: Type::Void,
            llvm_name: "@__monster_builtin_memcpy",
        },
        RuntimeBuiltin {
            name: "str_eq",
            params: vec![Type::Str, Type::Str],
            ret_type: Type::Bool,
            llvm_name: "@__monster_builtin_str_eq",
        },
    ]
}

pub fn runtime_builtin_llvm_name(name: &str) -> Option<&'static str> {
    runtime_builtins()
        .into_iter()
        .find(|builtin| builtin.name == name)
        .map(|builtin| builtin.llvm_name)
}

pub fn is_runtime_builtin(name: &str) -> bool {
    runtime_builtin_llvm_name(name).is_some()
}

pub fn is_compiler_builtin(name: &str) -> bool {
    matches!(name, "len" | "slice" | "is" | "payload") || is_runtime_builtin(name)
}

pub fn runtime_declared_function(name: &str) -> bool {
    matches!(
        name,
        "printf"
            | "puts"
            | "scanf"
            | "fopen"
            | "fclose"
            | "fseek"
            | "ftell"
            | "fread"
            | "fwrite"
            | "calloc"
            | "strlen"
            | "memcmp"
            | "memcpy"
            | "exit"
    )
}
