/*
 * Copyright 2022, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::BTreeMap;

use smartstring::{LazyCompact, SmartString};

use crate::NamedRows;

/// The kind of mutation accumulated into a [`CallbackCollector`].
///
/// Historically this was used to notify registered callbacks of changes to a
/// relation. That callback machinery was removed; the enum is kept because
/// the `:returning` query clause still distinguishes inserts from deletes
/// when reporting which rows were affected by a write.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CallbackOp {
    /// A row was inserted or replaced.
    Put,
    /// A row was deleted.
    Rm,
}

impl CallbackOp {
    /// Get the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            CallbackOp::Put => "Put",
            CallbackOp::Rm => "Rm",
        }
    }
}

/// Accumulates mutations observed during a write transaction so that the
/// `:returning` clause can report which rows were affected at commit time.
/// The key is the relation name, the value is a list of
/// (operation, new-rows, old-rows) tuples.
pub(crate) type CallbackCollector =
    BTreeMap<SmartString<LazyCompact>, Vec<(CallbackOp, NamedRows, NamedRows)>>;
