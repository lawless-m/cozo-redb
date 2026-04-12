use cozo::{new_cozo_mem, ScriptMutability};

fn main() {
    let db = new_cozo_mem().unwrap();
    let script = "?[a] := a in [1, 2, 3]";
    let result = db
        .run_script(script, Default::default(), ScriptMutability::Immutable)
        .unwrap();
    println!("{:?}", result);
}
