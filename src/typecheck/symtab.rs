use std::collections::HashMap;
use crate::span::Span;
use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub ty: Type,
    pub is_mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSig {
    pub name: String,
    pub param_names: Vec<String>,
    pub param_types: Vec<Type>,
    pub return_ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ScopeEnvironment {
    scopes: Vec<HashMap<String, Symbol>>,
    functions: HashMap<String, FunctionSig>,
}

impl Default for ScopeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeEnvironment {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    pub fn define_variable(&mut self, symbol: Symbol) -> Result<(), Symbol> {
        if let Some(current_scope) = self.scopes.last_mut() {
            if let Some(existing) = current_scope.get(&symbol.name) {
                return Err(existing.clone());
            }
            current_scope.insert(symbol.name.clone(), symbol);
            Ok(())
        } else {
            Err(symbol)
        }
    }

    pub fn lookup_variable(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn define_function(&mut self, sig: FunctionSig) -> Result<(), FunctionSig> {
        if let Some(existing) = self.functions.get(&sig.name) {
            return Err(existing.clone());
        }
        self.functions.insert(sig.name.clone(), sig);
        Ok(())
    }

    pub fn lookup_function(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.get(name)
    }

    pub fn functions(&self) -> &HashMap<String, FunctionSig> {
        &self.functions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_and_shadowing() {
        let mut env = ScopeEnvironment::new();
        let sym1 = Symbol {
            name: "x".to_string(),
            ty: Type::I64,
            is_mutable: false,
            span: Span::new(0, 5),
        };
        assert!(env.define_variable(sym1.clone()).is_ok());

        // Duplicate in same scope rejected
        assert!(env.define_variable(sym1.clone()).is_err());

        // Enter nested scope and shadow x
        env.enter_scope();
        let sym2 = Symbol {
            name: "x".to_string(),
            ty: Type::F64,
            is_mutable: true,
            span: Span::new(10, 15),
        };
        assert!(env.define_variable(sym2).is_ok());
        assert_eq!(env.lookup_variable("x").unwrap().ty, Type::F64);
        assert!(env.lookup_variable("x").unwrap().is_mutable);

        // Exit scope restores outer x
        env.exit_scope();
        assert_eq!(env.lookup_variable("x").unwrap().ty, Type::I64);
        assert!(!env.lookup_variable("x").unwrap().is_mutable);
    }

    #[test]
    fn test_function_registration() {
        let mut env = ScopeEnvironment::new();
        let sig = FunctionSig {
            name: "add".to_string(),
            param_names: vec!["a".to_string(), "b".to_string()],
            param_types: vec![Type::F64, Type::F64],
            return_ty: Type::F64,
            span: Span::new(0, 20),
        };

        assert!(env.define_function(sig.clone()).is_ok());
        assert!(env.define_function(sig).is_err());
        assert_eq!(env.lookup_function("add").unwrap().return_ty, Type::F64);
        assert!(env.lookup_function("unknown").is_none());
    }
}
