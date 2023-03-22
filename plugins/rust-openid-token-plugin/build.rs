fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("request-rewrite-plugin.proto")?;
    Ok(())
}
