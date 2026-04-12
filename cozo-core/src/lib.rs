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
//! let db = DbInstance::new("mem", "", Default::default()).unwrap();
//! let script = "?[a] := a in [1, 2, 3]";
//! let result = db.run_script(script, Default::default(), ScriptMutability::Immutable).unwrap();
//! println!("{:?}", result);
//! ```
//! We created an in-memory database above. There are other persistent options:
//! see [DbInstance::new]. It is perfectly fine to run multiple storage engines in the same process.
//!
#![doc = document_features::document_features!()]
#![warn(rust_2018_idioms, future_incompatible)]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crossbeam::channel::{bounded, Receiver, Sender};
use data::functions::current_validity;
pub use miette::Error;
use miette::{bail, Result};
use parse::parse_script;
use parse::CozoScript;

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
pub use crate::runtime::db::Payload;
pub use crate::runtime::db::Poison;
pub use crate::runtime::db::ScriptMutability;
pub use crate::runtime::db::TransactionPayload;

pub mod data;
pub(crate) mod fixed_rule;
pub(crate) mod fts;
pub mod parse;
pub(crate) mod query;
pub(crate) mod runtime;
pub(crate) mod storage;
pub(crate) mod utils;

/// A dispatcher for concrete storage implementations, wrapping [Db]. This is done so that
/// client code does not have to deal with generic code constantly. You may prefer to use
/// [Db] directly, especially if you provide a custom storage engine.
///
/// Many methods are dispatching methods for the corresponding methods on [Db].
///
/// Other methods are wrappers simplifying signatures to deal with only strings.
/// These methods made code for interop with other languages much easier,
/// but are not desirable if you are using Rust.
#[derive(Clone)]
pub enum DbInstance {
    /// In memory storage (not persistent)
    Mem(Db<MemStorage>),
    #[cfg(feature = "storage-redb")]
    /// Redb storage
    Redb(Db<RedbStorage>),
}

impl Default for DbInstance {
    fn default() -> Self {
        Self::new("mem", "", Default::default()).unwrap()
    }
}

impl DbInstance {
    /// Create a DbInstance, which is a dispatcher for various concrete implementations.
    /// The valid engines are:
    ///
    /// * `mem`
    ///
    /// assuming all features are enabled during compilation. Otherwise only
    /// some of the engines are available. The `mem` engine is always available.
    ///
    /// `path` is ignored for the `mem` engine.
    /// The `options` string is currently unused.
    #[allow(unused_variables)]
    pub fn new(engine: &str, path: impl AsRef<Path>, _options: &str) -> Result<Self> {
        Ok(match engine {
            "mem" => Self::Mem(new_cozo_mem()?),
            #[cfg(feature = "storage-redb")]
            "redb" => Self::Redb(new_cozo_redb(path)?),
            k => bail!(
                "database engine '{}' not supported (maybe not compiled in)",
                k
            ),
        })
    }
    /// Dispatcher method.  See [crate::Db::get_fixed_rules].
    pub fn get_fixed_rules(&self) -> BTreeMap<String, Arc<Box<dyn FixedRule>>> {
        match self {
            DbInstance::Mem(db) => db.get_fixed_rules(),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.get_fixed_rules(),
        }
    }
    /// Dispatcher method. See [crate::Db::run_script].
    pub fn run_script(
        &self,
        payload: &str,
        params: BTreeMap<String, DataValue>,
        mutability: ScriptMutability,
    ) -> Result<NamedRows> {
        let cur_vld = current_validity();
        self.run_script_ast(
            parse_script(payload, &params, &self.get_fixed_rules(), cur_vld)?,
            cur_vld,
            mutability,
        )
    }
    /// `run_script` with mutable script and no parameters
    pub fn run_default(&self, payload: &str) -> Result<NamedRows> {
        self.run_script(payload, BTreeMap::new(), ScriptMutability::Mutable)
    }
    /// Run a parsed (AST) program. If you have a string script, use `run_script` or `run_default`.
    pub fn run_script_ast(
        &self,
        payload: CozoScript,
        cur_vld: ValidityTs,
        mutability: ScriptMutability,
    ) -> Result<NamedRows> {
        match self {
            DbInstance::Mem(db) => db.run_script_ast(payload, cur_vld, mutability),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.run_script_ast(payload, cur_vld, mutability),
        }
    }
    /// Dispatcher method. See [crate::Db::export_relations].
    pub fn export_relations<I, T>(&self, relations: I) -> Result<BTreeMap<String, NamedRows>>
    where
        T: AsRef<str>,
        I: Iterator<Item = T>,
    {
        match self {
            DbInstance::Mem(db) => db.export_relations(relations),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.export_relations(relations),
        }
    }
    /// Dispatcher method. See [crate::Db::import_relations].
    pub fn import_relations(&self, data: BTreeMap<String, NamedRows>) -> Result<()> {
        match self {
            DbInstance::Mem(db) => db.import_relations(data),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.import_relations(data),
        }
    }
    /// Dispatcher method. See [crate::Db::register_callback].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register_callback(
        &self,
        relation: &str,
        capacity: Option<usize>,
    ) -> (u32, Receiver<(CallbackOp, NamedRows, NamedRows)>) {
        match self {
            DbInstance::Mem(db) => db.register_callback(relation, capacity),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.register_callback(relation, capacity),
        }
    }

    /// Dispatcher method. See [crate::Db::unregister_callback].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn unregister_callback(&self, id: u32) -> bool {
        match self {
            DbInstance::Mem(db) => db.unregister_callback(id),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.unregister_callback(id),
        }
    }
    /// Dispatcher method. See [crate::Db::register_fixed_rule].
    pub fn register_fixed_rule<R>(&self, name: String, rule_impl: R) -> Result<()>
    where
        R: FixedRule + 'static,
    {
        match self {
            DbInstance::Mem(db) => db.register_fixed_rule(name, rule_impl),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.register_fixed_rule(name, rule_impl),
        }
    }
    /// Dispatcher method. See [crate::Db::unregister_fixed_rule]
    pub fn unregister_fixed_rule(&self, name: &str) -> Result<bool> {
        match self {
            DbInstance::Mem(db) => db.unregister_fixed_rule(name),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.unregister_fixed_rule(name),
        }
    }

    /// Dispatcher method. See [crate::Db::run_multi_transaction]
    pub fn run_multi_transaction(
        &self,
        write: bool,
        payloads: Receiver<TransactionPayload>,
        results: Sender<Result<NamedRows>>,
    ) {
        match self {
            DbInstance::Mem(db) => db.run_multi_transaction(write, payloads, results),
            #[cfg(feature = "storage-redb")]
            DbInstance::Redb(db) => db.run_multi_transaction(write, payloads, results),
        }
    }
    /// A higher-level, blocking wrapper for [crate::Db::run_multi_transaction]. Runs the transaction on a dedicated thread.
    /// Write transactions _may_ block other reads, but we guarantee that this does not happen for the RocksDB backend.
    pub fn multi_transaction(&self, write: bool) -> MultiTransaction {
        let (app2db_send, app2db_recv) = bounded(1);
        let (db2app_send, db2app_recv) = bounded(1);
        let db = self.clone();
        #[cfg(target_arch = "wasm32")]
        std::thread::spawn(move || db.run_multi_transaction(write, app2db_recv, db2app_send));
        #[cfg(not(target_arch = "wasm32"))]
        rayon::spawn(move || db.run_multi_transaction(write, app2db_recv, db2app_send));
        MultiTransaction {
            sender: app2db_send,
            receiver: db2app_recv,
        }
    }
}

/// A multi-transaction handle.
/// You should use either the fields directly, or the associated functions.
pub struct MultiTransaction {
    /// Commands can be sent into the transaction through this channel
    pub sender: Sender<TransactionPayload>,
    /// Results can be retrieved from the transaction from this channel
    pub receiver: Receiver<Result<NamedRows>>,
}

impl MultiTransaction {
    /// Runs a single script in the transaction.
    pub fn run_script(
        &self,
        payload: &str,
        params: BTreeMap<String, DataValue>,
    ) -> Result<NamedRows> {
        if let Err(err) = self
            .sender
            .send(TransactionPayload::Query((payload.to_string(), params)))
        {
            bail!(err);
        }
        match self.receiver.recv() {
            Ok(r) => r,
            Err(err) => bail!(err),
        }
    }
    /// Commits the multi-transaction
    pub fn commit(&self) -> Result<()> {
        if let Err(err) = self.sender.send(TransactionPayload::Commit) {
            bail!(err);
        }
        match self.receiver.recv() {
            Ok(_) => Ok(()),
            Err(err) => bail!(err),
        }
    }
    /// Aborts the multi-transaction
    pub fn abort(&self) -> Result<()> {
        if let Err(err) = self.sender.send(TransactionPayload::Abort) {
            bail!(err);
        }
        match self.receiver.recv() {
            Ok(_) => Ok(()),
            Err(err) => bail!(err),
        }
    }
}

