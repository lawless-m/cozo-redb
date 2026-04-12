/*
 * Copyright 2022, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::process::exit;

use clap::Parser;

use crate::repl::{repl_main, ReplArgs};

mod repl;

fn main() {
    let args = ReplArgs::parse();
    if let Err(e) = repl_main(args) {
        eprintln!("{e}");
        exit(-1);
    }
}
