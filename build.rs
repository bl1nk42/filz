fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let mut config = prost_build::Config::new();
    config.out_dir(&out_dir);
    config.compile_protos(&["proto/registry.proto"], &["proto/"])?;
    Ok(())
}
