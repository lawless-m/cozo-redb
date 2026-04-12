/*
 *  Copyright 2022, The Cozo Project Authors.
 *
 *  This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 *  If a copy of the MPL was not distributed with this file,
 *  You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 */
#![feature(test)]

extern crate test;

use cozo::{DataValue, DbInstance, NamedRows, ScriptMutability, Validity};
use itertools::Itertools;
use lazy_static::{initialize, lazy_static};
use rand::Rng;
use rayon::prelude::*;
use std::cmp::max;
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::time::Instant;
use test::Bencher;

fn bench_base() -> usize {
    env::var("COZO_BENCH_TT_BASE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10000)
}

fn bench_max_k() -> usize {
    env::var("COZO_BENCH_TT_MAX_K")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000)
}

fn bench_k_values() -> Vec<usize> {
    let max_k = bench_max_k();
    [1usize, 10, 100, 1000]
        .into_iter()
        .filter(|k| *k <= max_k)
        .collect()
}

fn insert_data(db: &DbInstance) {
    let base = bench_base();

    let insert_plain_time = Instant::now();
    let mut to_import = BTreeMap::new();
    to_import.insert(
        "plain".to_string(),
        NamedRows {
            headers: vec!["k".to_string(), "v".to_string()],
            rows: (0..base)
                .map(|i| vec![DataValue::from(i as i64), DataValue::from(i as i64)])
                .collect_vec(),
            next: None,
        },
    );
    db.import_relations(to_import).unwrap();
    println!(
        "inserted plain ({} rows) in {:?}",
        base,
        insert_plain_time.elapsed()
    );

    for k in bench_k_values() {
        let rel = format!("tt{}", k);
        let insert_time = Instant::now();
        let mut to_import = BTreeMap::new();
        to_import.insert(
            rel.clone(),
            NamedRows {
                headers: vec!["k".to_string(), "vld".to_string(), "v".to_string()],
                rows: (0..base)
                    .flat_map(|i| {
                        (0..k).map(move |vld| {
                            vec![
                                DataValue::from(i as i64),
                                DataValue::Validity(Validity::from((vld as i64, true))),
                                DataValue::from(i as i64),
                            ]
                        })
                    })
                    .collect_vec(),
                next: None,
            },
        );
        db.import_relations(to_import).unwrap();
        println!(
            "inserted {} ({} rows) in {:?}",
            rel,
            base * k,
            insert_time.elapsed()
        );
    }
}

lazy_static! {
    static ref TEST_DB: DbInstance = {
        let engine = env::var("COZO_TEST_DB_ENGINE").unwrap_or_else(|_| "rocksdb".to_string());
        let dir = env::var("COZO_BENCH_TT_DIR").unwrap_or_else(|_| ".".to_string());
        let mut db_path = PathBuf::from(dir);
        db_path.push(format!("time_travel_{}.db", engine));

        if engine != "mem" {
            let _ = std::fs::remove_file(&db_path);
            let _ = std::fs::remove_dir_all(&db_path);
        }

        let path_str = if engine == "mem" {
            "".to_string()
        } else {
            db_path.to_string_lossy().into_owned()
        };
        println!("time_travel bench: engine={} path={:?}", engine, path_str);
        let db = DbInstance::new(&engine, &path_str, "").unwrap();

        let create_res = db.run_script(
            r#"
        {:create plain {k: Int => v}}
        {:create tt1 {k: Int, vld: Validity => v}}
        {:create tt10 {k: Int, vld: Validity => v}}
        {:create tt100 {k: Int, vld: Validity => v}}
        {:create tt1000 {k: Int, vld: Validity => v}}
        "#,
            Default::default(),
            ScriptMutability::Mutable,
        );

        if create_res.is_ok() {
            insert_data(&db);
        } else {
            println!("database already exists, skip import");
        }

        db
    };
}

fn single_plain_read() {
    let i = rand::thread_rng().gen_range(0..10000);
    TEST_DB
        .run_script(
            "?[v] := *plain{k: $id, v}",
            BTreeMap::from([("id".to_string(), DataValue::from(i as i64))]),
            ScriptMutability::Mutable,
        )
        .unwrap();
}

fn plain_aggr() {
    TEST_DB
        .run_script(
            r#"
    ?[sum(v)] := *plain{v}
    "#,
            BTreeMap::default(),
            ScriptMutability::Mutable,
        )
        .unwrap();
}

fn tt_stupid_aggr(k: usize) {
    TEST_DB
        .run_script(
            &format!(
                r#"
    r[k, smallest_by(pack)] := *tt{}{{k, vld, v}}, pack = [v, vld]
    ?[sum(v)] := r[k, v]
    "#,
                k
            ),
            BTreeMap::default(),
            ScriptMutability::Mutable,
        )
        .unwrap();
}

fn tt_travel_aggr(k: usize) {
    TEST_DB
        .run_script(
            &format!(
                r#"
    ?[sum(v)] := *tt{}{{v @ "NOW"}}
    "#,
                k
            ),
            BTreeMap::default(),
            ScriptMutability::Mutable,
        )
        .unwrap();
}

fn single_tt_read(k: usize) {
    let i = rand::thread_rng().gen_range(0..10000);
    TEST_DB
        .run_script(
            &format!(
                r#"
            ?[smallest_by(pack)] := *tt{}{{k: $id, vld, v}}, pack = [v, vld]
            "#,
                k
            ),
            BTreeMap::from([("id".to_string(), DataValue::from(i as i64))]),
            ScriptMutability::Mutable,
        )
        .unwrap();
}

fn single_tt_travel_read(k: usize) {
    let i = rand::thread_rng().gen_range(0..10000);
    TEST_DB
        .run_script(
            &format!(
                r#"
            ?[v] := *tt{}{{k: $id, v @ "NOW"}}
            "#,
                k
            ),
            BTreeMap::from([("id".to_string(), DataValue::from(i as i64))]),
            ScriptMutability::Mutable,
        )
        .unwrap();
}

#[bench]
fn time_travel_init(_: &mut Bencher) {
    initialize(&TEST_DB);

    let count = 100_000;
    let qps_single_plain_time = Instant::now();
    (0..count).into_par_iter().for_each(|_| {
        single_plain_read();
    });
    dbg!((count as f64) / qps_single_plain_time.elapsed().as_secs_f64());

    for k in bench_k_values() {
        let count = 100_000;
        let qps_single_tt_time = Instant::now();
        (0..count).into_par_iter().for_each(|_| {
            single_tt_read(k);
        });
        dbg!(k);
        dbg!((count as f64) / qps_single_tt_time.elapsed().as_secs_f64());
    }

    for k in bench_k_values() {
        let count = 100_000;
        let qps_single_tt_travel_time = Instant::now();
        (0..count).into_par_iter().for_each(|_| {
            single_tt_travel_read(k);
        });
        dbg!(k);
        dbg!((count as f64) / qps_single_tt_travel_time.elapsed().as_secs_f64());
    }

    let count = 100;

    let plain_aggr_time = Instant::now();
    (0..count).for_each(|_| {
        plain_aggr();
    });
    dbg!(plain_aggr_time.elapsed().as_secs_f64() * 1000. / (count as f64));

    for k in bench_k_values() {
        let count = max(1000 / k, 5);
        let tt_stupid_aggr_time = Instant::now();
        (0..count).for_each(|_| {
            tt_stupid_aggr(k);
        });
        dbg!(k);
        dbg!(tt_stupid_aggr_time.elapsed().as_secs_f64() * 1000. / (count as f64));

        let count = 20;
        let tt_travel_aggr_time = Instant::now();
        (0..count).for_each(|_| {
            tt_travel_aggr(k);
        });
        dbg!(k);
        dbg!(tt_travel_aggr_time.elapsed().as_secs_f64() * 1000. / (count as f64));
    }
}
