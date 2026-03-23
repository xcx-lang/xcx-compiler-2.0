//symbol_table.rs
use std::collections::{HashMap, HashSet};
use crate::parser::ast::Type;

#[derive(Clone)]
pub struct SymbolTable<'a> {
    parent: Option<&'a SymbolTable<'a>>,
    scopes: Vec<HashMap<String, Type>>,
    consts: Vec<HashSet<String>>,
}

impl<'a> SymbolTable<'a> {
    pub fn new() -> Self {
        Self {
            parent: None,
            scopes: vec![HashMap::new()],
            consts: vec![HashSet::new()],
        }
    }

    pub fn new_with_parent(parent: &'a SymbolTable<'a>) -> Self {
        Self {
            parent: Some(parent),
            scopes: vec![HashMap::new()],
            consts: vec![HashSet::new()],
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.consts.push(HashSet::new());
    }

    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
            self.consts.pop();
        }
    }

    pub fn has(&self, name: &str) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.contains_key(name) {
                return true;
            }
        }
        if let Some(p) = self.parent {
            return p.has(name);
        }
        false
    }

    pub fn has_in_current_scope(&self, name: &str) -> bool {
        self.scopes.last().map(|s| s.contains_key(name)).unwrap_or(false)
    }

    pub fn define(&mut self, name: String, ty: Type, is_const: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.clone(), ty);
        }
        if is_const {
            if let Some(c_scope) = self.consts.last_mut() {
                c_scope.insert(name);
            }
        }
    }

    pub fn lookup(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        if let Some(p) = self.parent {
            return p.lookup(name);
        }
        None
    }
    
    pub fn is_const(&self, name: &str) -> bool {
        for (i, scope) in self.scopes.iter().enumerate().rev() {
            if scope.contains_key(name) {
                return self.consts[i].contains(name);
            }
        }
        if let Some(p) = self.parent {
            return p.is_const(name);
        }
        false
    }

    pub fn _copy_globals(&self) -> Self {
        let mut curr = self;
        while let Some(p) = curr.parent {
            curr = p;
        }
        Self {
            parent: None,
            scopes: vec![curr.scopes[0].clone()],
            consts: vec![curr.consts[0].clone()],
        }
    }
}
