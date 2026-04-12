/*
 * Copyright 2022, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! This crate provides the core functionalities of [CozoDB](https://cozodb.org).
//! It may be used to embed CozoDB in your application.
//!
//! This doc describes the Rust API. To learn how to use CozoDB to query (CozoScript), see:
//!
//! * [The CozoDB documentation](https://docs.cozodb.org)
//!
//! Rust API usage:
//! ```
//! use cozo::*;
//!
//! let db = new_cozo_mem().unwrap();
//! let script = "?[a] := a in [1, 2, 3]";
//! let result = db.run_script(script, Default::default(), ScriptMutability::Immutable).unwrap();
//! println!("{:?}", result);
//! ```
//! The example above creates an in-memory database. For a persistent database,
//! enable the `storage-redb` feature and use [`new_cozo_redb`].
//!
#![doc = document_features::document_features!()]
#![warn(rust_2018_idioms, future_incompatible)]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use std::collections::BTreeMap;

pub use miette::Error;
use miette::Result;

pub use data::value::{DataValue, Num, RegexWrapper, UuidWrapper, Validity, ValidityTs};
pub use fixed_rule::{FixedRule, FixedRuleInputRelation, FixedRulePayload};
pub use runtime::db::Db;
pub use runtime::db::NamedRows;
pub use runtime::relation::decode_tuple_from_kv;
pub use runtime::temp_store::RegularTempStore;
pub use storage::mem::{new_cozo_mem, MemStorage};
#[cfg(feature = "storage-redb")]
pub use storage::re::{new_cozo_redb, RedbStorage};
pub use storage::{Storage, StoreTx};

pub use crate::data::expr::Expr;
pub use crate::data::symb::Symbol;
pub use crate::data::value::{JsonData, Vector};
pub use crate::fixed_rule::SimpleFixedRule;
pub use crate::parse::SourceSpan;
pub use crate::runtime::callback::CallbackOp;
pub use crate::runtime::db::evaluate_expressions;
pub use crate::runtime::db::get_variables;
pub use crate::runtime::db::Poison;
pub use crate::runtime::db::ScriptMutability;

pub(crate) mod data;
pub(crate) mod fixed_rule;
#[cfg(feature = "fts")]
pub(crate) mod fts;
pub(crate) mod parse;
pub(crate) mod query;
pub(crate) mod runtime;
pub(crate) mod storage;
pub(crate) mod utils;


impl<'s, S: Storage<'s>> Db<S> {
    /// Run `payload` with no parameters in mutable mode.
    pub fn run_default(&'s self, payload: &str) -> Result<NamedRows> {
        self.run_script(payload, BTreeMap::new(), ScriptMutability::Mutable)
    }
}

