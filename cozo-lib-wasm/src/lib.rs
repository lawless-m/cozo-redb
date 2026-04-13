/*
 * Copyright 2022, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::BTreeMap;

use wasm_bindgen::prelude::*;

use cozo::{new_cozo_mem, DataValue, Db, MemStorage, NamedRows, ScriptMutability};

mod utils;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub struct CozoDb {
    db: Db<MemStorage>,
}

#[wasm_bindgen]
impl CozoDb {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        utils::set_panic_hook();
        let db = new_cozo_mem().unwrap();
        Self { db }
    }

    pub fn run(&self, script: &str, params: &str, immutable: bool) -> String {
        let params_map = match parse_params(params) {
            Ok(p) => p,
            Err(msg) => return error_json(&msg),
        };
        let mutability = if immutable {
            ScriptMutability::Immutable
        } else {
            ScriptMutability::Mutable
        };
        match self.db.run_script(script, params_map, mutability) {
            Ok(rows) => ok_rows(rows),
            Err(err) => error_json(&format!("{err:?}")),
        }
    }

    pub fn export_relations(&self, data: &str) -> String {
        let payload: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(e) => return error_json(&format!("bad export payload: {e}")),
        };
        let relations = match payload.get("relations").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return error_json("expected { relations: [...] }"),
        };
        let names: Vec<String> = relations
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        match self.db.export_relations(names.iter()) {
            Ok(map) => {
                let mut obj = serde_json::Map::new();
                obj.insert("ok".to_string(), serde_json::Value::Bool(true));
                let mut data_obj = serde_json::Map::new();
                for (k, v) in map {
                    data_obj.insert(k, v.into_json());
                }
                obj.insert("data".to_string(), serde_json::Value::Object(data_obj));
                serde_json::Value::Object(obj).to_string()
            }
            Err(e) => error_json(&format!("{e:?}")),
        }
    }

    pub fn import_relations(&self, data: &str) -> String {
        let payload: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(e) => return error_json(&format!("bad import payload: {e}")),
        };
        let obj = match payload.as_object() {
            Some(o) => o,
            None => return error_json("expected a JSON object mapping relation names to rows"),
        };
        let mut rels: BTreeMap<String, NamedRows> = BTreeMap::new();
        for (name, val) in obj {
            let headers = val
                .get("headers")
                .and_then(|h| h.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let rows_json = match val.get("rows").and_then(|r| r.as_array()) {
                Some(a) => a,
                None => return error_json(&format!("relation {name} missing rows")),
            };
            let rows = rows_json
                .iter()
                .map(|row| {
                    row.as_array()
                        .map(|cells| {
                            cells
                                .iter()
                                .cloned()
                                .map(DataValue::from)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect();
            rels.insert(name.clone(), NamedRows::new(headers, rows));
        }
        match self.db.import_relations(rels) {
            Ok(()) => serde_json::json!({ "ok": true }).to_string(),
            Err(e) => error_json(&format!("{e:?}")),
        }
    }
}

fn parse_params(params: &str) -> Result<BTreeMap<String, DataValue>, String> {
    if params.is_empty() {
        return Ok(BTreeMap::new());
    }
    let parsed: serde_json::Value =
        serde_json::from_str(params).map_err(|e| format!("bad params: {e}"))?;
    let obj = parsed
        .as_object()
        .ok_or_else(|| "params must be a JSON object".to_string())?;
    Ok(obj
        .iter()
        .map(|(k, v)| (k.clone(), DataValue::from(v.clone())))
        .collect())
}

fn ok_rows(rows: NamedRows) -> String {
    let mut j = rows.into_json();
    if let Some(obj) = j.as_object_mut() {
        obj.insert("ok".to_string(), serde_json::Value::Bool(true));
    }
    j.to_string()
}

fn error_json(msg: &str) -> String {
    serde_json::json!({
        "ok": false,
        "message": msg,
    })
    .to_string()
}
