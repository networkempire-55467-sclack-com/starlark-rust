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
//! Starlark call stack.

use crate::values::error::ValueError;
use std::cell::Cell;
use std::fmt;

/// Starlark call stack.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct CallStack {
    stack: Vec<(String, String)>,
}

impl CallStack {
    /// Push an element to the stack
    pub fn push(&mut self, function_id: &str, call_descr: &str, file_name: &str, line: u32) {
        self.stack.push((
            function_id.to_owned(),
            format!(
                "call to {} at {}:{}",
                call_descr,
                file_name,
                line + 1, // line 1 is 0, so add 1 for human readable.
            ),
        ));
    }

    /// Test if call stack contains a function with given id.
    pub fn contains(&self, function_id: &str) -> bool {
        self.stack.iter().any(|(n, _)| n == function_id)
    }

    /// Print call stack as multiline string
    /// with each line beginning with newline.
    pub fn print_with_newline_before<'a>(&'a self) -> impl fmt::Display + 'a {
        DisplayWithNewlineBefore { call_stack: self }
    }
}

struct DisplayWithNewlineBefore<'a> {
    call_stack: &'a CallStack,
}

impl<'a> fmt::Display for DisplayWithNewlineBefore<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (_fname, descr) in self.call_stack.stack.iter().rev() {
            write!(f, "\n    {}", descr)?;
        }
        Ok(())
    }
}

// Maximum recursion level for comparison
// TODO(dmarting): those are rather short, maybe make it configurable?
#[cfg(debug_assertions)]
const MAX_RECURSION: u32 = 200;

#[cfg(not(debug_assertions))]
const MAX_RECURSION: u32 = 3000;

// A thread-local counter is used to detect too deep recursion.
//
// Thread-local is chosen instead of explicit function "recursion" parameter
// for two reasons:
// * It's possible to propagate stack depth across external functions like
//   `Display::to_string` where passing a stack depth parameter is hard
// * We need to guarantee that stack depth is not lost in complex invocation
//   chains like function calls compare which calls native function which calls
//   starlark function which calls to_str. We could change all evaluation stack
//   signatures to accept some "context" parameters, but passing it as thread-local
//   is easier.
thread_local!(static STACK_DEPTH: Cell<u32> = Cell::new(0));

/// Stored previous stack depth before calling `try_inc`.
///
/// Stores that previous stack depths back to thread-local on drop.
#[must_use]
pub struct StackGuard {
    prev_depth: u32,
}

impl Drop for StackGuard {
    fn drop(&mut self) {
        STACK_DEPTH.with(|c| c.set(self.prev_depth));
    }
}

/// Increment stack depth.
fn inc() -> StackGuard {
    let prev_depth = STACK_DEPTH.with(|c| {
        let prev = c.get();
        c.set(prev + 1);
        prev
    });
    StackGuard { prev_depth }
}

/// Check stack depth does not exceed configured max stack depth.
fn check() -> Result<(), ValueError> {
    if STACK_DEPTH.with(Cell::get) >= MAX_RECURSION {
        return Err(ValueError::TooManyRecursionLevel);
    }
    Ok(())
}

/// Try increment stack depth.
///
/// Return opaque `StackGuard` object which resets stack to previous value
/// on `drop`.
///
/// If stack depth exceeds configured limit, return error.
pub fn try_inc() -> Result<StackGuard, ValueError> {
    check()?;
    Ok(inc())
}