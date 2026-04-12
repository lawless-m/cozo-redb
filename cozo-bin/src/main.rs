/*
 * Copyright 2022, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::process::exit;

use clap::Parser;
use cozo::new_cozo_mem;
#[cfg(feature = "storage-redb")]
use cozo::new_cozo_redb;

use crate::repl::{repl_main, ReplArgs};

mod repl;

fn main() {
    let args = ReplArgs::parse();
    let result = match args.engine.as_str() {
        "mem" => repl_main(new_cozo_mem().unwrap()),
        #[cfg(feature = "storage-redb")]
        "redb" => repl_main(new_cozo_redb(&args.path).unwrap()),
        engine => {
            eprintln!("unknown engine: {engine}");
            exit(-1);
        }
    };
    if let Err(e) = result {
        eprintln!("{e}");
        exit(-1);
    }
}
