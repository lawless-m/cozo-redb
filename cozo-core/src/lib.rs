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

impl<'s, S: Storage<'s>> Db<S> {
    /// Run `payload` with no parameters in mutable mode.
    pub fn run_default(&'s self, payload: &str) -> Result<NamedRows> {
        self.run_script(payload, BTreeMap::new(), ScriptMutability::Mutable)
    }
}

impl<S> Db<S>
where
    S: for<'a> Storage<'a> + 'static,
{
    /// Spawn a background thread that owns a long-running transaction and
    /// returns a [`MultiTransaction`] handle for driving it over channels.
    pub fn multi_transaction(&self, write: bool) -> MultiTransaction {
        let (app2db_send, app2db_recv) = bounded(1);
        let (db2app_send, db2app_recv) = bounded(1);
        let db = self.clone();
        std::thread::spawn(move || db.run_multi_transaction(write, app2db_recv, db2app_send));
        MultiTransaction {
            sender: app2db_send,
            receiver: db2app_recv,
        }
    }
}

