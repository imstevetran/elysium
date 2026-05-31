use crate::ast::*;
use crate::error::{CompileError, Result};
use std::collections::HashMap;

pub struct TypeChecker {
    types: HashMap<String, Type>,
    scopes: Vec<HashMap<String, Type>>,
    functions: HashMap<String, FunctionSignature>,
    errors: Vec<CompileError>,
    in_async: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    Nil,
    Array(Box<Type>),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Function(Vec<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Named(String, Vec<Type>),
    Future(Box<Type>),
    Infer,
    Error,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub param_types: Vec<Type>,
    pub return_type: Box<Type>,
    pub is_async: bool,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = Self {
            types: HashMap::new(),
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            errors: Vec::new(),
            in_async: false,
        };
        tc.register_builtins();
        tc
    }

    fn register_builtins(&mut self) {
        self.types.insert("Int".into(), Type::Int);
        self.types.insert("Float".into(), Type::Float);
        self.types.insert("Bool".into(), Type::Bool);
        self.types.insert("String".into(), Type::String);
        self.types.insert("Char".into(), Type::Char);
        self.types.insert("Nil".into(), Type::Nil);

        self.functions.insert(
            "print".into(),
            FunctionSignature {
                param_types: vec![Type::Infer],
                return_type: Box::new(Type::Nil),
                is_async: false,
            },
        );
        // console.* builtins (desugared from console.debug/warn/info/log/error)
        // and bare print (desugared to __console_print)
        for name in &[
            "__console_debug",
            "__console_info",
            "__console_warn",
            "__console_error",
            "__console_log",
            "__console_print",
        ] {
            self.functions.insert(
                name.to_string(),
                FunctionSignature {
                    param_types: vec![Type::Infer],
                    return_type: Box::new(Type::Nil),
                    is_async: false,
                },
            );
        }
        // fs.* builtins (desugared from fs.readFile/writeFile/exists/...)
        {
            let void_sigs: &[(&str, Vec<Type>)] = &[
                ("__fs_writeFile", vec![Type::String, Type::String]),
                ("__fs_appendFile", vec![Type::String, Type::String]),
                ("__fs_removeFile", vec![Type::String]),
                ("__fs_createDir", vec![Type::String]),
                ("__fs_removeDir", vec![Type::String]),
                ("__fs_copyFile", vec![Type::String, Type::String]),
                ("__fs_rename", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in void_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::Nil),
                        is_async: false,
                    },
                );
            }
            let string_sigs: &[(&str, Vec<Type>)] = &[
                ("__fs_readFile", vec![Type::String]),
                ("__fs_readFileSync", vec![Type::String]),
            ];
            for (name, param_types) in string_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
            let bool_sigs: &[(&str, Vec<Type>)] = &[
                ("__fs_exists", vec![Type::String]),
                ("__fs_isFile", vec![Type::String]),
                ("__fs_isDir", vec![Type::String]),
            ];
            for (name, param_types) in bool_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::Bool),
                        is_async: false,
                    },
                );
            }
        }
        // transport.* builtins (HTTP, WebSocket, MQTT)
        {
            let http_sigs: &[(&str, Vec<Type>, Type)] = &[
                ("__transport_get", vec![Type::String], Type::String),
                ("__transport_post", vec![Type::String, Type::String], Type::String),
                ("__transport_put", vec![Type::String, Type::String], Type::String),
                ("__transport_delete", vec![Type::String], Type::String),
            ];
            for (name, param_types, ret_ty) in http_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(ret_ty.clone()),
                        is_async: false,
                    },
                );
            }
            let ws_sigs: &[(&str, Vec<Type>, Type)] = &[
                ("__transport_wsConnect", vec![Type::String], Type::String),
                ("__transport_wsSend", vec![Type::String, Type::String], Type::Nil),
                ("__transport_wsClose", vec![Type::String], Type::Nil),
            ];
            for (name, param_types, ret_ty) in ws_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(ret_ty.clone()),
                        is_async: false,
                    },
                );
            }
            let mqtt_sigs: &[(&str, Vec<Type>, Type)] = &[
                ("__transport_mqttConnect", vec![Type::String, Type::String], Type::String),
                ("__transport_mqttPublish", vec![Type::String, Type::String], Type::Nil),
                ("__transport_mqttSubscribe", vec![Type::String], Type::Nil),
                ("__transport_mqttDisconnect", vec![], Type::Nil),
            ];
            for (name, param_types, ret_ty) in mqtt_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(ret_ty.clone()),
                        is_async: false,
                    },
                );
            }
        }
        // string.* builtins (desugared from string.length(x) or x.length())
        // First arg is always the string receiver.
        {
            // String → Int
            let int_sigs: &[(&str, Vec<Type>)] = &[
                ("__string_length", vec![Type::String]),
                ("__string_charCodeAt", vec![Type::String, Type::Int]),
                ("__string_indexOf", vec![Type::String, Type::String]),
                ("__string_lastIndexOf", vec![Type::String, Type::String]),
                ("__string_search", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in int_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::Int),
                        is_async: false,
                    },
                );
            }
            // String → Bool
            let bool_sigs: &[(&str, Vec<Type>)] = &[
                ("__string_isEmpty", vec![Type::String]),
                ("__string_startsWith", vec![Type::String, Type::String]),
                ("__string_endsWith", vec![Type::String, Type::String]),
                ("__string_contains", vec![Type::String, Type::String]),
                ("__string_includes", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in bool_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::Bool),
                        is_async: false,
                    },
                );
            }
            // String → String (including crypto: sha256, md5, base64, hex)
            let string_sigs: &[(&str, Vec<Type>)] = &[
                ("__string_toUpper", vec![Type::String]),
                ("__string_toLower", vec![Type::String]),
                ("__string_trim", vec![Type::String]),
                ("__string_trimStart", vec![Type::String]),
                ("__string_trimEnd", vec![Type::String]),
                ("__string_toString", vec![Type::String]),
                ("__string_charAt", vec![Type::String, Type::Int]),
                ("__string_slice", vec![Type::String, Type::Int, Type::Int]),
                ("__string_substring", vec![Type::String, Type::Int, Type::Int]),
                ("__string_replace", vec![Type::String, Type::String, Type::String]),
                ("__string_concat", vec![Type::String, Type::String]),
                ("__string_padStart", vec![Type::String, Type::Int, Type::String]),
                ("__string_padEnd", vec![Type::String, Type::Int, Type::String]),
                ("__string_repeat", vec![Type::String, Type::Int]),
                ("__string_split", vec![Type::String, Type::String]),
                ("__string_match", vec![Type::String, Type::String]),
                // crypto: (String) → String (input → hash/encoded)
                ("__string_sha256", vec![Type::String]),
                ("__string_md5", vec![Type::String]),
                ("__string_base64Encode", vec![Type::String]),
                ("__string_base64Decode", vec![Type::String]),
                ("__string_hexEncode", vec![Type::String]),
                ("__string_hexDecode", vec![Type::String]),
            ];
            for (name, param_types) in string_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
            // (String, String) → String (crypto: hmac with key)
            self.functions.insert(
                "__string_hmac".to_string(),
                FunctionSignature {
                    param_types: vec![Type::String, Type::String],
                    return_type: Box::new(Type::String),
                    is_async: false,
                },
            );
            // () → String
            self.functions.insert(
                "__string_uuid".to_string(),
                FunctionSignature {
                    param_types: vec![],
                    return_type: Box::new(Type::String),
                    is_async: false,
                },
            );
        }
        // regex.* builtins (desugared from regex.test/match/search/replace/split)
        {
            // (String, String) → Bool
            let bool_sigs: &[(&str, Vec<Type>)] = &[
                ("__regex_test", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in bool_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::Bool),
                        is_async: false,
                    },
                );
            }
            // (String, String) → Int
            let int_sigs: &[(&str, Vec<Type>)] = &[
                ("__regex_search", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in int_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::Int),
                        is_async: false,
                    },
                );
            }
            // (String, String) → String or (String, String, String) → String
            let string_sigs: &[(&str, Vec<Type>)] = &[
                ("__regex_match", vec![Type::String, Type::String]),
                ("__regex_replace", vec![Type::String, Type::String, Type::String]),
                ("__regex_split", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in string_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
        }
        // datetime.* builtins (unix timestamp as universal bridge)
        {
            // () → Int
            self.functions.insert(
                "__datetime_now".to_string(),
                FunctionSignature {
                    param_types: vec![],
                    return_type: Box::new(Type::Int),
                    is_async: false,
                },
            );
            // (Int, String) → String
            let fmt_sigs: &[(&str, Vec<Type>)] = &[
                ("__datetime_fromTimestamp", vec![Type::Int]),
                ("__datetime_format", vec![Type::Int, Type::String]),
            ];
            for (name, param_types) in fmt_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
            // (String) → String
            self.functions.insert(
                "__datetime_parse".to_string(),
                FunctionSignature {
                    param_types: vec![Type::String],
                    return_type: Box::new(Type::String),
                    is_async: false,
                },
            );
            // (Int) → Int — component extraction
            let comp_sigs: &[&str] = &[
                "__datetime_year",
                "__datetime_month",
                "__datetime_day",
                "__datetime_hour",
                "__datetime_minute",
                "__datetime_second",
                "__datetime_weekday",
            ];
            for name in comp_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: vec![Type::Int],
                        return_type: Box::new(Type::Int),
                        is_async: false,
                    },
                );
            }
            // (Int, Int) → Int — arithmetic
            let arith_sigs: &[&str] = &[
                "__datetime_addDays",
                "__datetime_addHours",
                "__datetime_diffSeconds",
            ];
            for name in arith_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: vec![Type::Int, Type::Int],
                        return_type: Box::new(Type::Int),
                        is_async: false,
                    },
                );
            }
        }
        // auth.* builtins (Session, JWT, Passkey, OAuth2, Authorization, Multi-tenant)
        {
            // Most auth functions take String params and return String (JSON-encoded)
            let string_sigs: &[(&str, Vec<Type>)] = &[
                ("__auth_jwtSign", vec![Type::String, Type::String]),
                ("__auth_jwtVerify", vec![Type::String]),
                ("__auth_jwtDecode", vec![Type::String]),
                ("__auth_createSession", vec![Type::String, Type::String]),
                ("__auth_getSession", vec![Type::String]),
                ("__auth_hashPassword", vec![Type::String]),
                ("__auth_verifyPassword", vec![Type::String, Type::String]),
                ("__auth_checkPermission", vec![Type::String, Type::String]),
                ("__auth_hasRole", vec![Type::String, Type::String]),
                ("__auth_hasScope", vec![Type::String, Type::String]),
                ("__auth_oauth2Authorize", vec![Type::String, Type::String, Type::String]),
                ("__auth_oauth2Token", vec![Type::String, Type::String, Type::String]),
                ("__auth_oauth2Refresh", vec![Type::String, Type::String]),
                ("__auth_passkeyRegister", vec![Type::String, Type::String]),
                ("__auth_passkeyAuthenticate", vec![Type::String]),
                ("__auth_tenantContext", vec![Type::String]),
                ("__auth_getTenant", vec![]),
                ("__auth_listTenants", vec![]),
                ("__auth_createTenant", vec![Type::String, Type::String]),
                ("__auth_grantRole", vec![Type::String, Type::String]),
                ("__auth_grantPermission", vec![Type::String, Type::String]),
                ("__auth_revokeRole", vec![Type::String, Type::String]),
                ("__auth_revokePermission", vec![Type::String, Type::String]),
                ("__auth_generateApiKey", vec![Type::String]),
                ("__auth_validateApiKey", vec![Type::String]),
                ("__auth_checkAccess", vec![Type::String, Type::String, Type::String]),
                ("__auth_setRoles", vec![Type::String, Type::String]),
                ("__auth_setPermissions", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in string_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
            // destroySession: (String) → Nil
            self.functions.insert(
                "__auth_destroySession".to_string(),
                FunctionSignature {
                    param_types: vec![Type::String],
                    return_type: Box::new(Type::Nil),
                    is_async: false,
                },
            );
        }
        // worker.* builtins
        {
            let string_sigs: &[(&str, Vec<Type>)] = &[
                ("__worker_create", vec![Type::String]),
                ("__worker_send", vec![Type::String, Type::String]),
                ("__worker_post", vec![Type::String, Type::String]),
                ("__worker_receive", vec![Type::String]),
                ("__worker_wait", vec![Type::String]),
                ("__worker_terminate", vec![Type::String]),
                ("__worker_isRunning", vec![Type::String]),
                ("__worker_activeCount", vec![]),
                ("__worker_terminateAll", vec![]),
            ];
            for (name, param_types) in string_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
        }
        // dict.* builtins (mutable key-value dictionary)
        {
            let string_sigs: &[(&str, Vec<Type>)] = &[
                ("__dict_create", vec![]),
                ("__dict_set", vec![Type::String, Type::String, Type::String]),
                ("__dict_get", vec![Type::String, Type::String]),
                ("__dict_has", vec![Type::String, Type::String]),
                ("__dict_delete", vec![Type::String, Type::String]),
                ("__dict_keys", vec![Type::String]),
                ("__dict_length", vec![Type::String]),
                ("__dict_clear", vec![Type::String]),
            ];
            for (name, param_types) in string_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
        }
        // json.* builtins (JSON parsing and serialization)
        {
            let string_sigs: &[(&str, Vec<Type>)] = &[
                ("__json_parse", vec![Type::String]),
                ("__json_parseInline", vec![Type::String]),
                ("__json_get", vec![Type::String, Type::String]),
                ("__json_stringify", vec![Type::String]),
                ("__json_free", vec![Type::String]),
                ("__json_buildMessage", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in string_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
            // __json_buildObject is varargs: pairs of (String, String)*
            self.functions.insert(
                "__json_buildObject".to_string(),
                FunctionSignature {
                    param_types: vec![
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                        Type::String, Type::String,
                    ],
                    return_type: Box::new(Type::String),
                    is_async: false,
                },
            );
            // __json_buildArray is varargs too
            self.functions.insert(
                "__json_buildArray".to_string(),
                FunctionSignature {
                    param_types: vec![Type::String, Type::String, Type::String, Type::String,
                                      Type::String, Type::String, Type::String, Type::String],
                    return_type: Box::new(Type::String),
                    is_async: false,
                },
            );
        }
        // math.* builtins (extended math operations)
        {
            let unary_sigs: &[&str] = &[
                "sqrt", "abs", "floor", "ceil", "round",
                "sin", "cos", "tan", "log", "log2", "log10", "exp",
            ];
            for name in unary_sigs {
                let fn_name = format!("__math_{}", name);
                self.functions.insert(
                    fn_name,
                    FunctionSignature {
                        param_types: vec![Type::Float],
                        return_type: Box::new(Type::Float),
                        is_async: false,
                    },
                );
            }
            let binary_sigs: &[&str] = &[
                "pow", "max", "min",
            ];
            for name in binary_sigs {
                let fn_name = format!("__math_{}", name);
                self.functions.insert(
                    fn_name,
                    FunctionSignature {
                        param_types: vec![Type::Float, Type::Float],
                        return_type: Box::new(Type::Float),
                        is_async: false,
                    },
                );
            }
            // Array/vector math: all take String (JSON array strings) and return String
            let vec_sigs: &[(&str, Vec<Type>)] = &[
                ("__math_sum", vec![Type::String]),
                ("__math_mean", vec![Type::String]),
                ("__math_dot", vec![Type::String, Type::String]),
                ("__math_cosineSimilarity", vec![Type::String, Type::String]),
                ("__math_euclidean", vec![Type::String, Type::String]),
            ];
            for (name, param_types) in vec_sigs {
                self.functions.insert(
                    name.to_string(),
                    FunctionSignature {
                        param_types: param_types.clone(),
                        return_type: Box::new(Type::String),
                        is_async: false,
                    },
                );
            }
        }
        // env.* builtins (environment variables)
        {
            self.functions.insert(
                "__env_get".to_string(),
                FunctionSignature {
                    param_types: vec![Type::String],
                    return_type: Box::new(Type::String),
                    is_async: false,
                },
            );
            self.functions.insert(
                "__env_set".to_string(),
                FunctionSignature {
                    param_types: vec![Type::String, Type::String],
                    return_type: Box::new(Type::String),
                    is_async: false,
                },
            );
        }
        // http.* builtins (HTTP client with custom headers)
        self.functions.insert(
            "__http_request".to_string(),
            FunctionSignature {
                param_types: vec![Type::String, Type::String, Type::String, Type::String],
                return_type: Box::new(Type::String),
                is_async: false,
            },
        );
        self.functions.insert(
            "__http_requestSync".to_string(),
            FunctionSignature {
                param_types: vec![Type::String, Type::String, Type::String, Type::String],
                return_type: Box::new(Type::String),
                is_async: false,
            },
        );
        // __is_instanceof for runtime type checking (desugared from `is` operator)
        self.functions.insert(
            "__is_instanceof".to_string(),
            FunctionSignature {
                param_types: vec![Type::Infer, Type::String],
                return_type: Box::new(Type::Bool),
                is_async: false,
            },
        );
        self.functions.insert(
            "sum".into(),
            FunctionSignature {
                param_types: vec![Type::Array(Box::new(Type::Float))],
                return_type: Box::new(Type::Float),
                is_async: false,
            },
        );
        self.functions.insert(
            "min".into(),
            FunctionSignature {
                param_types: vec![Type::Float, Type::Float],
                return_type: Box::new(Type::Float),
                is_async: false,
            },
        );
        self.functions.insert(
            "max".into(),
            FunctionSignature {
                param_types: vec![Type::Float, Type::Float],
                return_type: Box::new(Type::Float),
                is_async: false,
            },
        );
    }

    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        // First pass: collect all function signatures (including class methods)
        for item in &program.items {
            match &item.value {
                Item::Function(f) => {
                    let sig = self.infer_function_signature(f);
                    self.functions.insert(f.name.clone(), sig);
                }
                Item::Class(c) => {
                    for method in &c.methods {
                        let sig = self.infer_function_signature(method);
                        self.functions.insert(method.name.clone(), sig);
                    }
                }
                _ => {}
            }
        }

        // Second pass: check bodies
        for item in &program.items {
            match &item.value {
                Item::Function(f) => {
                    self.check_func_body(f)?;
                }
                Item::Class(c) => {
                    for method in &c.methods {
                        self.check_func_body(method)?;
                    }
                }
                _ => {}
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.remove(0))
        }
    }

    fn check_func_body(&mut self, f: &Function) -> Result<()> {
        // Skip stub functions — no body to type-check
        if f.stub_envs.is_some() {
            return Ok(());
        }
        let prev_async = self.in_async;
        self.in_async = f.is_async;
        self.scopes.push(HashMap::new());
        for param in &f.params {
            let ty = param
                .type_ann
                .as_ref()
                .map(|t| self.resolve_type_expr(t))
                .unwrap_or(Type::Infer);
            self.scopes.last_mut().unwrap().insert(param.name.clone(), ty);
        }

        if let Some(_ret_type) = &f.return_type {
            // checked by resolution
        }

        let _ = self.check_block(&f.body);
        self.scopes.pop();
        self.in_async = prev_async;
        Ok(())
    }

    fn infer_function_signature(&mut self, f: &Function) -> FunctionSignature {
        let param_types = f
            .params
            .iter()
            .map(|p| {
                p.type_ann
                    .as_ref()
                    .map(|t| self.resolve_type_expr(t))
                    .unwrap_or(Type::Infer)
            })
            .collect();
        let return_type = f
            .return_type
            .as_ref()
            .map(|t| Box::new(self.resolve_type_expr(t)))
            .unwrap_or(Box::new(Type::Infer));
        FunctionSignature {
            param_types,
            return_type,
            is_async: f.is_async,
        }
    }

    fn resolve_type_expr(&self, texpr: &TypeExpr) -> Type {
        match texpr {
            TypeExpr::Named(name) => {
                self.types.get(name).cloned().unwrap_or(Type::Named(name.clone(), vec![]))
            }
            TypeExpr::Generic(name, params) => {
                let resolved: Vec<Type> = params.iter().map(|p| self.resolve_type_expr(p)).collect();
                Type::Named(name.clone(), resolved)
            }
            TypeExpr::Array(inner) => Type::Array(Box::new(self.resolve_type_expr(inner))),
            TypeExpr::Option(inner) => Type::Option(Box::new(self.resolve_type_expr(inner))),
            TypeExpr::Result(ok, err) => Type::Result(
                Box::new(self.resolve_type_expr(ok)),
                Box::new(self.resolve_type_expr(err)),
            ),
            TypeExpr::Union(ts) => {
                ts.first()
                    .map(|t| self.resolve_type_expr(t))
                    .unwrap_or(Type::Error)
            }
            TypeExpr::Function(params, ret) => Type::Function(
                params.iter().map(|p| self.resolve_type_expr(p)).collect(),
                Box::new(self.resolve_type_expr(ret)),
            ),
            TypeExpr::Tuple(ts) => {
                Type::Tuple(ts.iter().map(|t| self.resolve_type_expr(t)).collect())
            }
            TypeExpr::Record(fields) => Type::Named(
                "Record".into(),
                fields.iter().map(|(_, t)| self.resolve_type_expr(t)).collect(),
            ),
            TypeExpr::Infer => Type::Infer,
        }
    }

    fn check_block(&mut self, block: &Block) -> Option<Type> {
        let mut last_type = None;
        for stmt in &block.statements {
            last_type = Some(self.check_stmt(&stmt.value));
        }
        last_type
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Type {
        match stmt {
            Stmt::Let(boxed_node) => {
                let let_stmt = &boxed_node.value;
                let ty = if let Some(val) = &let_stmt.value {
                    self.check_expr(val)
                } else {
                    Type::Infer
                };
                self.scopes.last_mut().unwrap().insert(let_stmt.name.clone(), ty.clone());
                ty
            }
            Stmt::Expr(boxed) => self.check_expr(&boxed.value),
            Stmt::Return(ret) => {
                ret.as_ref()
                    .map(|e| self.check_expr(&e.value))
                    .unwrap_or(Type::Nil)
            }
            Stmt::Assign(assign) => {
                let a = &assign.value;
                let val_ty = self.check_expr(&a.value.value);
                let _target_ty = self.check_expr(&a.target.value);
                val_ty
            }
            Stmt::BcAssert(assert) => {
                let _cond = self.check_expr(&assert.value.condition.value);
                Type::Nil
            }
            Stmt::If(boxed) => {
                let ifs = &boxed.value;
                let _ = self.check_expr(&ifs.condition.value);
                let then_ty = self.check_block(&ifs.then_block);
                let _else_ty = ifs
                    .else_block
                    .as_ref()
                    .map(|b| self.check_block(b))
                    .unwrap_or(None);
                then_ty.unwrap_or(Type::Nil)
            }
            Stmt::For(boxed) => {
                let fs = &boxed.value;
                let _ = self.check_expr(&fs.iterable.value);
                self.scopes.last_mut().unwrap().insert(fs.variable.clone(), Type::Infer);
                self.check_block(&fs.body);
                Type::Nil
            }
            Stmt::While(boxed) => {
                let ws = &boxed.value;
                let _ = self.check_expr(&ws.condition.value);
                self.check_block(&ws.body);
                Type::Nil
            }
            Stmt::Match(boxed) => {
                let ms = &boxed.value;
                let _ = self.check_expr(&ms.value.value);
                for arm in &ms.arms {
                    self.check_block(&arm.body);
                }
                Type::Infer
            }
            Stmt::TryCatch(boxed) => {
                let tc = &boxed.value;
                let try_ty = self.check_block(&tc.try_block);
                let catch_ty = self.check_block(&tc.catch_block);
                catch_ty.unwrap_or(try_ty.unwrap_or(Type::Nil))
            }
            Stmt::OnlyGuard(boxed) => {
                let og = &boxed.value;
                let _ = self.check_expr(&og.condition.value);
                self.check_block(&og.body);
                Type::Nil
            }
            Stmt::UnsafeBlock(boxed) => {
                self.check_block(&boxed.value.body);
                Type::Infer
            }
            Stmt::Expect(boxed) => {
                let _ = self.check_expr(&boxed.value.expr.value);
                Type::Nil
            }
            Stmt::Todo(_) | Stmt::Question(_) => Type::Nil,
            Stmt::Bench(boxed) => {
                self.check_block(&boxed.value.body);
                Type::Nil
            }
            Stmt::Parallel(boxed) => {
                for item in &boxed.value.items {
                    self.check_stmt(&item.value);
                }
                Type::Nil
            }
            Stmt::Wait(_) => Type::Nil,
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(lit) => match &lit.value {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::String(_) => Type::String,
                Literal::Char(_) => Type::Char,
                Literal::Nil => Type::Nil,
            },
            Expr::Identifier(name) => {
                for scope in self.scopes.iter().rev() {
                    if let Some(ty) = scope.get(name) {
                        return ty.clone();
                    }
                }
                if let Some(sig) = self.functions.get(name) {
                    return Type::Function(sig.param_types.clone(), sig.return_type.clone());
                }
                Type::Infer
            }
            Expr::BinaryOp { op: _, left, right } => {
                let l = self.check_expr(&left.value);
                let r = self.check_expr(&right.value);
                match (l, r) {
                    (Type::Int, Type::Int) => Type::Int,
                    (Type::Float, _) | (_, Type::Float) => Type::Float,
                    (Type::Bool, Type::Bool) => Type::Bool,
                    (Type::String, _) | (_, Type::String) => Type::String,
                    _ => Type::Infer,
                }
            }
            Expr::UnaryOp { op: _, operand } => self.check_expr(&operand.value),
            Expr::Call { callee, args } => {
                let _callee_ty = self.check_expr(&callee.value);
                for arg in args {
                    self.check_expr(&arg.value);
                }
                if let Expr::Identifier(name) = &callee.value {
                    if let Some(sig) = self.functions.get(name) {
                        return *sig.return_type.clone();
                    }
                }
                Type::Infer
            }
            Expr::MethodCall { object, method: _, args } => {
                let _obj_ty = self.check_expr(&object.value);
                for arg in args {
                    self.check_expr(&arg.value);
                }
                Type::Infer
            }
            Expr::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                let _ = self.check_expr(&condition.value);
                let then_ty = self.check_expr(&then_expr.value);
                let else_ty = else_expr.as_ref().map(|e| self.check_expr(&e.value)).unwrap_or(Type::Nil);
                if then_ty == else_ty { then_ty } else { Type::Infer }
            }
            Expr::Lambda { params, body } => {
                let param_types: Vec<Type> = params
                    .iter()
                    .map(|p| {
                        p.type_ann
                            .as_ref()
                            .map(|t| self.resolve_type_expr(t))
                            .unwrap_or(Type::Infer)
                    })
                    .collect();
                let ret_ty = self.check_expr(&body.value);
                Type::Function(param_types, Box::new(ret_ty))
            }
            Expr::Block(block) => self.check_block(block).unwrap_or(Type::Nil),
            Expr::Array(items) => {
                if items.is_empty() {
                    Type::Array(Box::new(Type::Infer))
                } else {
                    Type::Array(Box::new(self.check_expr(&items[0].value)))
                }
            }
            Expr::Tuple(items) => Type::Tuple(items.iter().map(|i| self.check_expr(&i.value)).collect()),
            Expr::Record(fields) => {
                for (_, expr) in fields { self.check_expr(&expr.value); }
                Type::Infer
            }
            Expr::Index { target, index } => {
                let _ = self.check_expr(&target.value);
                let _ = self.check_expr(&index.value);
                Type::Infer
            }
            Expr::MemberAccess { target, field: _ } => self.check_expr(&target.value),
            Expr::Range { start, end, .. } => {
                self.check_expr(&start.value);
                self.check_expr(&end.value);
                Type::Array(Box::new(Type::Int))
            }
            Expr::Spread(inner) => self.check_expr(&inner.value),
            Expr::BcAnnotation { expr, .. } => self.check_expr(&expr.value),
            Expr::ErrorPropagate(inner) => {
                let ty = self.check_expr(&inner.value);
                if let Type::Result(ok, _) = ty { *ok } else { Type::Infer }
            }
            Expr::Await(inner) => {
                if !self.in_async {
                    self.errors.push(crate::error::CompileError::new(
                        "`await` used outside of async function".to_string(),
                    ));
                }
                let ty = self.check_expr(&inner.value);
                match ty {
                    Type::Future(inner_ty) => *inner_ty,
                    Type::Infer => Type::Infer,
                    _ => {
                        self.errors.push(crate::error::CompileError::new(
                            format!("cannot `await` non-Future type {:?}", ty),
                        ));
                        Type::Error
                    }
                }
            }
            Expr::MatchExpression { value, arms } => {
                let _ = self.check_expr(&value.value);
                for arm in arms { self.check_block(&arm.body); }
                Type::Infer
            }
            Expr::Is { value, .. } => {
                self.check_expr(&value.value);
                Type::Bool
            }
        }
    }

    pub fn into_errors(self) -> Vec<CompileError> {
        self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    fn check_src(src: &str) -> std::result::Result<(), CompileError> {
        let mut p = crate::parser::Parser::new(src);
        let program = p.parse_program().expect("parse failed");
        let mut tc = TypeChecker::new();
        tc.check_program(&program)
    }

    fn assert_ok(src: &str) {
        let result = check_src(src);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    // ----- Spec items are ignored -----

    #[test]
    fn test_type_check_spec_is_ok() {
        assert_ok(r##"spec "math" { feat "add" { expect 1 + 1 } }"##);
    }

    #[test]
    fn test_type_check_describe_is_ok() {
        assert_ok(r##"describe "suite" { it "pass" { expect true } }"##);
    }

    #[test]
    fn test_type_check_todo_is_ok() {
        assert_ok("func f() { todo }");
    }

    #[test]
    fn test_type_check_todo_with_message_is_ok() {
        assert_ok(r##"func f() { todo "fix this" }"##);
    }

    #[test]
    fn test_type_check_question_is_ok() {
        assert_ok("func f() { question }");
    }

    #[test]
    fn test_type_check_bench_is_ok() {
        assert_ok("func f() { bench { let x = 1 } }");
    }

    #[test]
    fn test_type_check_bm_is_ok() {
        assert_ok("func f() { bm { let x = 42 } }");
    }

    #[test]
    fn test_type_check_expect_is_ok() {
        assert_ok("func f() { expect 1 + 2 }");
    }

    #[test]
    fn test_type_check_import_is_ok() {
        assert_ok(r##"import "foo.ely""##);
    }

    #[test]
    fn test_type_check_import_as_is_ok() {
        assert_ok(r##"import "foo.ely" as mymod"##);
    }

    // ----- Combined -----

    #[test]
    fn test_type_check_spec_with_bench() {
        assert_ok(r##"
            spec "perf" {
                feat "fib" { bench { let x = 1 } }
            }
        "##);
    }
}
