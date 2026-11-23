//! Generate the schema from the real Rust wire types; reject checked-in drift.
use std::{env, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|x| x != "--check") {
        return Err("only --check is accepted".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("bindings/protocol.schema.json");
    let schema = schemars::schema_for!(suzume_api::protocol::ProtocolSchema);
    let expected = serde_json::to_string_pretty(&schema)? + "\n";
    if args.iter().any(|x| x == "--check") {
        if fs::read_to_string(&path)? != expected {
            return Err("protocol schema differs from Rust types; regenerate explicitly".into());
        }
        println!("API schema: synchronized");
    } else {
        fs::create_dir_all(path.parent().ok_or("schema directory missing")?)?;
        fs::write(path, expected)?;
        println!("API schema: generated");
    }
    Ok(())
}
