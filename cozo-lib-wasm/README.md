# Cozo in web assembly

This crate provides Cozo web assembly modules for browsers.
If you are targeting NodeJS, use [this](../cozo-lib-nodejs) instead: 
native code is still _much_ faster than WASM.

This document describes how to set up the Cozo WASM module for use.
To learn how to use CozoDB (CozoScript), read the [docs](https://docs.cozodb.org/en/latest/index.html).

## Installation

```
npm install cozo-lib-wasm
```

Alternatively, you can download `cozo_wasm-<VERSION>-wasm32-unknown-unknown.zip`
from the [release page](https://github.com/cozodb/cozo/releases) and include
the JS and WASM files directly in your project: see the `index.html` example 
[here](https://rustwasm.github.io/docs/wasm-bindgen/examples/without-a-bundler.html) for
what is required in your code.

## Usage

See the code [here](wasm-react-demo/src/App.js). Basically, you write

```js
import init, {CozoDb} from "cozo-lib-wasm";
```

and call

```js
let db;
init().then(() => {
    db = CozoDb.new();
    // db can only be used after the promise resolves 
})
```

## API

```ts
export class CozoDb {
    free(): void;

    static new(): CozoDb;

    run(script: string, params: string, immutable: boolean): string;

    export_relations(data: string): string;

    // Note that triggers are _not_ run for the relations, if any exists.
    // If you need to activate triggers, use queries with parameters.
    import_relations(data: string): string;
}
```

Note that this API is synchronous. If your computation runs for a long time, 
**it will block the main thread**. If you know that some of your queries are going to be heavy,
you should consider running Cozo in a web worker. However, the published module
may not work across browsers in web workers (look for the row "Support for ECMAScript
modules" [here](https://developer.mozilla.org/en-US/docs/Web/API/Worker/Worker#browser_compatibility)).

The next section contains some pointers for how to alleviate this, but expect a lot of work.

## Compiling

You will need [Rust](https://rustup.rs/),
[wasm-pack](https://github.com/rustwasm/wasm-pack), and a wasm-capable
clang (Debian: `apt install clang-19`). The clang dependency is forced
by `zstd-sys`, which `tantivy-sstable` pulls in unconditionally and
which compiles a small C shim under `wasm-shim/` when targeting
`wasm32-unknown-unknown`.

Then run:

```bash
./build.sh
```

`build.sh` defaults to `clang-19` and `llvm-ar-19`; override with the
`CC_WASM` and `AR_WASM` environment variables if your binaries are
named differently. The script is a thin wrapper around:

```bash
CC_wasm32_unknown_unknown=clang-19 \
AR_wasm32_unknown_unknown=llvm-ar-19 \
CARGO_PROFILE_RELEASE_LTO=fat \
    wasm-pack build --target web --release
```

The important option is `--target web`: the above usage instructions
only work for this target. See the documentation
[here](https://rustwasm.github.io/wasm-pack/book/commands/build.html#target).

if you are interested in running Cozo in a web worker and expect it to run across browsers,
you will need to use the `--target no-modules` option, and write a lot of gluing code.
See [here](https://rustwasm.github.io/wasm-bindgen/examples/wasm-in-web-worker.html) for tips.