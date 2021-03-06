// Copyright 2019 The Starlark in Rust Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Utilities to work with scope local variables.

use std::collections::hash_map;
use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
struct Scope {
    /// Name to slot mapping in current scope
    name_to_slot: HashMap<String, usize>,
    nested_scopes: Vec<Scope>,
}

/// Mapping of local variables and scopes to local variable slots
#[derive(Default, Debug, Clone)]
#[doc(hidden)]
pub struct Locals {
    locals: Scope,
    local_count: usize,
}

/// Utility to assign slots to local variables
#[derive(Default)]
pub(crate) struct LocalsBuilder {
    locals: Locals,
    current_scope_path: Vec<usize>,
}

/// Utility to query slots assigned to local variables
pub(crate) struct LocalsQuery<'a> {
    locals: &'a Locals,
    current_scope_path: Vec<usize>,
    next: usize,
}

impl Scope {
    /// Find local variable index in given scope
    fn local_index(&self, name: &str, scope_path: &[usize]) -> Option<usize> {
        let deepest_index = if let Some((first, rem)) = scope_path.split_first() {
            self.nested_scopes[*first].local_index(name, rem)
        } else {
            None
        };
        match deepest_index {
            Some(index) => Some(index),
            None => self.name_to_slot.get(name).cloned(),
        }
    }

    fn scope_by_path<'a>(&'a self, path: &[usize]) -> &'a Scope {
        match path.split_first() {
            Some((&first, rem)) => self.nested_scopes[first].scope_by_path(rem),
            None => self,
        }
    }
}

impl Locals {
    /// Return the number of local variable slots
    pub fn len(&self) -> usize {
        self.local_count
    }

    pub fn top_level_name_to_slot(&self, name: &str) -> Option<usize> {
        self.locals.local_index(name, &[])
    }
}

impl LocalsBuilder {
    fn current_locals(&mut self) -> &mut Scope {
        let mut locals = &mut self.locals.locals;
        for &index in &self.current_scope_path {
            locals = &mut locals.nested_scopes[index];
        }
        locals
    }

    /// Create a new nested scope
    pub fn push_scope(&mut self) {
        let locals = self.current_locals();
        locals.nested_scopes.push(Scope::default());
        let n = locals.nested_scopes.len() - 1;
        self.current_scope_path.push(n);
    }

    /// Go to one scope down
    pub fn pop_scope(&mut self) {
        self.current_scope_path.pop().unwrap();
    }

    /// Register a variable in current scope
    pub fn register_local(&mut self, name: &str) {
        let local_count = self.locals.local_count;
        if let hash_map::Entry::Vacant(e) =
            self.current_locals().name_to_slot.entry(name.to_owned())
        {
            e.insert(local_count);
        }
        self.locals.local_count += 1;
    }

    /// Finish the building
    pub fn build(self) -> Locals {
        // sanity check
        assert!(self.current_scope_path.is_empty());

        self.locals
    }
}

impl<'a> LocalsQuery<'a> {
    pub fn new(locals: &'a Locals) -> LocalsQuery<'a> {
        LocalsQuery {
            locals,
            current_scope_path: Vec::new(),
            next: 0,
        }
    }

    /// Return a slot for a variable visible in current scope.
    /// Local could be registered in current scope or in parent scopes,
    /// but not in nested scopes.
    pub fn local_slot(&self, name: &str) -> Option<usize> {
        self.locals
            .locals
            .local_index(name, &self.current_scope_path)
    }

    /// Go to the next nested scope
    pub fn push_next_scope(&mut self) {
        self.current_scope_path.push(self.next);
        self.next = 0;
    }

    /// Pop a scope
    pub fn pop_scope(&mut self) {
        // We must not leave the current scope if
        // nested scopes were not traversed
        assert_eq!(
            self.next,
            self.locals
                .locals
                .scope_by_path(&self.current_scope_path)
                .nested_scopes
                .len()
        );

        self.next = self.current_scope_path.pop().unwrap() + 1;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn one_level() {
        let mut builder = LocalsBuilder::default();
        builder.register_local("a");
        builder.register_local("b");
        builder.register_local("a");
        let locals = builder.build();
        let query = LocalsQuery::new(&locals);
        assert_eq!(Some(0), query.local_slot("a"));
        assert_eq!(Some(1), query.local_slot("b"));
        assert_eq!(None, query.local_slot("c"));
    }

    #[test]
    fn override_on_second_level() {
        let mut builder = LocalsBuilder::default();
        builder.register_local("a");
        builder.push_scope();
        builder.register_local("a");
        builder.pop_scope();
        let locals = builder.build();
        let mut query = LocalsQuery::new(&locals);
        assert_eq!(Some(0), query.local_slot("a"));
        query.push_next_scope();
        assert_eq!(Some(1), query.local_slot("a"));
        query.pop_scope();
        assert_eq!(Some(0), query.local_slot("a"));
    }

    #[test]
    fn overrride_twice_on_second_level() {
        // Here we have three distinct `a` variables:
        // in the top scope, and in two nested scopes
        let mut builder = LocalsBuilder::default();
        builder.register_local("a");
        builder.push_scope();
        builder.register_local("a");
        builder.pop_scope();
        builder.push_scope();
        builder.register_local("a");
        builder.pop_scope();
        let locals = builder.build();
        let mut query = LocalsQuery::new(&locals);
        assert_eq!(Some(0), query.local_slot("a"));
        query.push_next_scope();
        assert_eq!(Some(1), query.local_slot("a"));
        query.pop_scope();
        assert_eq!(Some(0), query.local_slot("a"));
        query.push_next_scope();
        assert_eq!(Some(2), query.local_slot("a"));
        query.pop_scope();
        assert_eq!(Some(0), query.local_slot("a"));
    }
}
