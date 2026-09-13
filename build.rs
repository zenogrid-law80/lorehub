fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure().compile_protos(
        &["proto/auth_api.proto", "proto/rebac_api.proto"],
        &["proto"],
    )?;
    Ok(())
}
