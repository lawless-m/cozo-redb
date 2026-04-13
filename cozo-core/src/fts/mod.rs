/*
 * Copyright 2026, Matt Lawless, for the cozo-redb fork.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Full-text search backed by [tantivy](https://github.com/quickwit-oss/tantivy).
//!
//! This module replaces the deleted upstream cozo FTS engine with a thin
//! bridge to tantivy. A single FTS index is stored as a sidecar directory
//! alongside the `.redb` database file and is created/queried through
//! CozoScript:
//!
//! ```cozoscript
//! {:create notes {id: Int => title: String, body: String}}
//! ::fts create notes:ft { fields: [title, body] }
//!
//! ?[id, score] := ~notes:ft{query: "+rust graph", k: 10, bind_score: score}
//! ```
//!
//! The `query` string is passed verbatim to tantivy's
//! [`QueryParser`](tantivy::query::QueryParser), so the full Lucene-style
//! query language is available: `+required -excluded "phrase"~slop field:term`,
//! boolean operators, boosts, fuzzy matching, etc. The query parser searches
//! all indexed text fields by default.
//!
//! The cozo-side integration points are:
//!
//! * [`FtsIndexManifest`] — the serializable metadata that describes an FTS
//!   index. Stored on [`RelationHandle`] alongside hnsw/regular index
//!   manifests, serialised with the relation into the meta relation.
//! * [`FtsIndexCache`] — a per-[`Db`] cache of live tantivy `Index` and
//!   `IndexWriter` handles, keyed by `(base_relation, index_name)`. Writers
//!   are created lazily on first write and reused across transactions.
//! * [`FtsIndexRuntime`] — one entry in the cache, wrapping the tantivy
//!   `Index`, its `Schema`, and the held `IndexWriter`.
//!
//! The sidecar directory layout is `<redb_path>.ft/<rel_name>/<idx_name>/`.
//! On database open, any indices declared by relation manifests have their
//! directories created (or opened if already present) via
//! [`FtsIndexRuntime::open_or_create`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use miette::{IntoDiagnostic, Result};
use serde_derive::{Deserialize, Serialize};
use smartstring::{LazyCompact, SmartString};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::data::value::DataValue;

/// The name used for the hidden field that stores the primary-key tuple of the
/// indexed cozo relation, so tantivy results can be mapped back to rows.
pub(crate) const KEY_FIELD: &str = "_key";

/// Default memory budget for a tantivy [`IndexWriter`]. Tantivy recommends
/// at least 15 MB; 50 MB is the value used in their own examples.
const WRITER_HEAP_BUDGET: usize = 50 * 1024 * 1024;

/// Persistent metadata describing a full-text search index built on top of a
/// cozo relation. This struct is serialised into the meta relation alongside
/// the `RelationHandle` that owns it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct FtsIndexManifest {
    /// Name of the base cozo relation being indexed.
    pub(crate) base_relation: SmartString<LazyCompact>,
    /// Logical name of this FTS index, e.g. `ft` in `notes:ft`.
    pub(crate) index_name: SmartString<LazyCompact>,
    /// Names of the columns on the base relation that should be fed into
    /// the tantivy index as searchable text fields. Order is preserved and
    /// used for the default query-parser field list.
    pub(crate) fields: Vec<SmartString<LazyCompact>>,
}

impl FtsIndexManifest {
    /// Resolve the on-disk directory for this index, given the cozo database
    /// file path. For an index on `notes:ft` with a redb file at `my.db`,
    /// the tantivy directory is `my.db.ft/notes/ft/`.
    pub(crate) fn index_path(&self, db_path: &Path) -> PathBuf {
        let mut p = db_path.as_os_str().to_owned();
        p.push(".ft");
        let mut p = PathBuf::from(p);
        p.push(self.base_relation.as_str());
        p.push(self.index_name.as_str());
        p
    }
}

/// Live tantivy handles for a single FTS index.
///
/// The [`FtsIndexCache`] holds one of these per `(base_relation, index_name)`
/// pair. The `Index` + `IndexWriter` + `IndexReader` are created once on
/// first access and reused across cozo transactions. Writes are buffered
/// in the writer and flushed on cozo transaction commit via
/// [`FtsIndexCache::commit_pending`].
pub(crate) struct FtsIndexRuntime {
    pub(crate) index: Index,
    pub(crate) writer: Mutex<IndexWriter>,
    pub(crate) reader: IndexReader,
    pub(crate) text_fields: Vec<Field>,
    pub(crate) key_field: Field,
    /// Whether any buffered writes exist that have not yet been committed
    /// to the tantivy index.
    pub(crate) dirty: std::sync::atomic::AtomicBool,
}

impl FtsIndexRuntime {
    /// Build (or reopen) a tantivy index at the given directory, with a
    /// schema derived from the manifest's declared field names. Called once
    /// per manifest per database open. When compiled without `fts-mmap`
    /// (e.g. the WASM build) the directory argument is ignored and the
    /// index is held entirely in RAM — each database open starts with an
    /// empty FTS index that must be repopulated.
    pub(crate) fn open_or_create(manifest: &FtsIndexManifest, dir: &Path) -> Result<Self> {
        let mut builder = Schema::builder();
        builder.add_text_field(KEY_FIELD, STRING | STORED);
        for name in &manifest.fields {
            builder.add_text_field(name.as_str(), TEXT | STORED);
        }
        let schema = builder.build();

        #[cfg(feature = "fts-mmap")]
        let index = {
            std::fs::create_dir_all(dir).into_diagnostic()?;
            // Try to open an existing index first; fall back to creating a
            // new one with the current schema. A schema mismatch on reopen
            // causes tantivy to error; the user must drop and recreate the
            // index in that case.
            match Index::open_in_dir(dir) {
                Ok(existing) => existing,
                Err(_) => Index::create_in_dir(dir, schema.clone()).into_diagnostic()?,
            }
        };
        #[cfg(not(feature = "fts-mmap"))]
        let index = {
            let _ = dir;
            Index::create_in_ram(schema.clone())
        };

        let writer: IndexWriter = index.writer(WRITER_HEAP_BUDGET).into_diagnostic()?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .into_diagnostic()?;

        let text_fields: Vec<Field> = manifest
            .fields
            .iter()
            .map(|name| schema.get_field(name.as_str()).unwrap())
            .collect();
        let key_field = schema.get_field(KEY_FIELD).unwrap();

        Ok(Self {
            index,
            writer: Mutex::new(writer),
            reader,
            text_fields,
            key_field,
            dirty: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Add a document to the tantivy index. `key_bytes` is the encoded
    /// cozo primary-key tuple; `field_values` are the stringified values
    /// of the indexed columns, in the same order as `manifest.fields`.
    pub(crate) fn add_document(
        &self,
        key_bytes: &[u8],
        field_values: &[Option<String>],
    ) -> Result<()> {
        let mut doc = TantivyDocument::default();
        doc.add_text(self.key_field, encode_key(key_bytes));
        for (field, value) in self.text_fields.iter().zip(field_values.iter()) {
            if let Some(v) = value {
                doc.add_text(*field, v);
            }
        }
        let writer = self.writer.lock().unwrap();
        writer.add_document(doc).into_diagnostic()?;
        self.dirty.store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    /// Remove any documents matching the given encoded cozo primary-key
    /// tuple from the tantivy index. Cozo guarantees primary keys are
    /// unique within a relation, so this deletes at most one document.
    pub(crate) fn delete_document(&self, key_bytes: &[u8]) -> Result<()> {
        let writer = self.writer.lock().unwrap();
        let term = Term::from_field_text(self.key_field, &encode_key(key_bytes));
        writer.delete_term(term);
        self.dirty.store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    /// Flush any buffered writes to disk. Called at cozo transaction commit
    /// time after the underlying redb transaction has committed successfully.
    pub(crate) fn commit_pending(&self) -> Result<()> {
        if !self.dirty.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        let mut writer = self.writer.lock().unwrap();
        writer.commit().into_diagnostic()?;
        drop(writer);
        // Force the reader to pick up the new segments immediately — by
        // default the reload policy is `OnCommitWithDelay`, which is fine
        // for production servers but means tests (or any synchronous code
        // that queries right after a write) see a stale view.
        self.reader.reload().into_diagnostic()?;
        self.dirty
            .store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    /// Run a free-text query against the index and return up to `k` matches
    /// as `(encoded_key, score)` tuples. The `query` string is parsed by
    /// tantivy's [`QueryParser`] against all indexed text fields.
    pub(crate) fn search(&self, query: &str, k: usize) -> Result<Vec<(Vec<u8>, f32)>> {
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(&self.index, self.text_fields.clone());
        let parsed = parser.parse_query(query).into_diagnostic()?;
        let top = searcher
            .search(&parsed, &TopDocs::with_limit(k))
            .into_diagnostic()?;
        let mut results = Vec::with_capacity(top.len());
        for (score, address) in top {
            let retrieved: TantivyDocument = searcher.doc(address).into_diagnostic()?;
            let Some(key_value) = retrieved.get_first(self.key_field) else {
                continue;
            };
            let Some(key_str) = key_value.as_str() else {
                continue;
            };
            if let Some(bytes) = decode_key(key_str) {
                results.push((bytes, score));
            }
        }
        Ok(results)
    }
}

/// Per-[`Db`] cache of live FTS indices.
///
/// Keyed by `(base_relation_name, index_name)`. Runtimes are created lazily
/// on first read or write and reused across cozo transactions.
#[derive(Default)]
pub(crate) struct FtsIndexCache {
    entries: Mutex<BTreeMap<FtsKey, Arc<FtsIndexRuntime>>>,
}

type FtsKey = (SmartString<LazyCompact>, SmartString<LazyCompact>);

impl FtsIndexCache {
    /// Get or create the runtime for a given index manifest, rooted at the
    /// database file path.
    pub(crate) fn get_or_open(
        &self,
        manifest: &FtsIndexManifest,
        db_path: &Path,
    ) -> Result<Arc<FtsIndexRuntime>> {
        let key = (manifest.base_relation.clone(), manifest.index_name.clone());
        let mut entries = self.entries.lock().unwrap();
        if let Some(existing) = entries.get(&key) {
            return Ok(existing.clone());
        }
        let dir = manifest.index_path(db_path);
        let runtime = Arc::new(FtsIndexRuntime::open_or_create(manifest, &dir)?);
        entries.insert(key, runtime.clone());
        Ok(runtime)
    }

    /// Commit all dirty runtimes. Called from the cozo transaction commit
    /// path after the underlying redb transaction has succeeded.
    pub(crate) fn commit_all_pending(&self) -> Result<()> {
        let entries = self.entries.lock().unwrap();
        for runtime in entries.values() {
            runtime.commit_pending()?;
        }
        Ok(())
    }

    /// Drop the runtime for a removed index and delete its on-disk
    /// directory. Called from the `::fts drop` / `::index remove` path.
    pub(crate) fn drop_index(
        &self,
        base_relation: &str,
        index_name: &str,
        db_path: &Path,
    ) -> Result<()> {
        let key = (
            SmartString::from(base_relation),
            SmartString::from(index_name),
        );
        self.entries.lock().unwrap().remove(&key);
        let manifest = FtsIndexManifest {
            base_relation: key.0,
            index_name: key.1,
            fields: vec![],
        };
        #[cfg(feature = "fts-mmap")]
        {
            let dir = manifest.index_path(db_path);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).into_diagnostic()?;
            }
        }
        #[cfg(not(feature = "fts-mmap"))]
        let _ = (manifest, db_path);
        Ok(())
    }
}

/// Convert a [`DataValue`] to the text representation fed into tantivy.
///
/// Non-string values are ignored (returned as `None`) — FTS indexing only
/// makes sense over textual columns, and silently skipping non-strings lets
/// a user index a mixed relation without having to scrub the schema.
pub(crate) fn value_as_indexable_text(value: &DataValue) -> Option<String> {
    match value {
        DataValue::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

/// Encode the raw primary-key bytes as a hex string for round-tripping
/// through tantivy's `STRING` field. Hex is simpler and more robust than
/// base64 for this purpose; the performance cost is negligible at FTS query
/// rates.
fn encode_key(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Reverse of [`encode_key`]. Returns `None` on malformed input (odd length
/// or non-hex characters); in practice this only happens if someone hand-
/// edits the index outside of cozo, in which case dropping the row is
/// safer than panicking.
fn decode_key(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let hi = hex_digit(chunk[0])?;
        let lo = hex_digit(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn hex_roundtrip() {
        let bytes = vec![0, 1, 2, 0xff, 0xab, 0x7e];
        let encoded = encode_key(&bytes);
        assert_eq!(encoded, "000102ffab7e");
        assert_eq!(decode_key(&encoded), Some(bytes));
    }

    #[test]
    fn decode_rejects_odd_length() {
        assert_eq!(decode_key("abc"), None);
    }

    #[test]
    fn decode_rejects_non_hex() {
        assert_eq!(decode_key("xyzz"), None);
    }

    /// End-to-end smoke test exercising the full
    /// create-index → add-docs → commit → search → delete → commit → search
    /// path against a real tantivy directory in a tempdir. Validates that
    /// the schema, the hex key round-trip, and the query parser all line up.
    #[test]
    fn roundtrip_index_search_delete() {
        let tmp = TempDir::new().unwrap();
        let manifest = FtsIndexManifest {
            base_relation: SmartString::from("notes"),
            index_name: SmartString::from("ft"),
            fields: vec![SmartString::from("title"), SmartString::from("body")],
        };
        let runtime = FtsIndexRuntime::open_or_create(&manifest, tmp.path()).unwrap();

        // Three documents with distinct primary keys.
        runtime
            .add_document(
                b"key-001",
                &[
                    Some("Rust graph database".to_string()),
                    Some("A fast embedded datalog engine".to_string()),
                ],
            )
            .unwrap();
        runtime
            .add_document(
                b"key-002",
                &[
                    Some("Python notebook".to_string()),
                    Some("An interactive exploration tool for data analysis".to_string()),
                ],
            )
            .unwrap();
        runtime
            .add_document(
                b"key-003",
                &[
                    Some("Graph theory lecture".to_string()),
                    Some("Covers adjacency lists and breadth-first search in Rust".to_string()),
                ],
            )
            .unwrap();
        runtime.commit_pending().unwrap();

        // Query for "rust" — expects keys 001 and 003 back, but not 002.
        let hits = runtime.search("rust", 10).unwrap();
        let hit_keys: Vec<Vec<u8>> = hits.into_iter().map(|(k, _)| k).collect();
        assert!(
            hit_keys.contains(&b"key-001".to_vec()),
            "expected key-001 in {:?}",
            hit_keys
        );
        assert!(
            hit_keys.contains(&b"key-003".to_vec()),
            "expected key-003 in {:?}",
            hit_keys
        );
        assert!(
            !hit_keys.contains(&b"key-002".to_vec()),
            "did not expect key-002 in {:?}",
            hit_keys
        );

        // Delete key-001 and confirm it no longer matches.
        runtime.delete_document(b"key-001").unwrap();
        runtime.commit_pending().unwrap();
        let hits = runtime.search("rust", 10).unwrap();
        let hit_keys: Vec<Vec<u8>> = hits.into_iter().map(|(k, _)| k).collect();
        assert!(
            !hit_keys.contains(&b"key-001".to_vec()),
            "key-001 should be gone after delete + commit"
        );
        assert!(
            hit_keys.contains(&b"key-003".to_vec()),
            "key-003 should still match"
        );
    }
}
