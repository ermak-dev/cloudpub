fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tell Cargo to rerun this build script if the .proto file changes
    println!("cargo:rerun-if-changed=src/protocol.proto");

    // Configure prost-build with experimental proto3 optional support
    let mut config = prost_build::Config::new();
    config.protoc_arg("--experimental_allow_proto3_optional");

    // Compile the .proto file
    config.type_attribute(".", "#[derive(serde::Serialize,serde::Deserialize)]");
    config.compile_protos(&["src/protocol.proto"], &["src/"])?;

    // Post process the generated code
    let out_dir = std::env::var("OUT_DIR")?;
    let mut generated = std::fs::read_to_string(format!("{}/protocol.rs", out_dir))?;

    // Remove PartialEq from ClientEndpoint derive attributes
    for name in ["ClientEndpoint", "ServerEndpoint"] {
        generated = generated.replace(
            format!(
                "#[derive(Clone, PartialEq, ::prost::Message)]
pub struct {} {{",
                name
            )
            .as_str(),
            format!(
                "#[derive(Clone, ::prost::Message)]
pub struct {} {{",
                name
            )
            .as_str(),
        );
    }

    std::fs::write(format!("{}/protocol.rs", out_dir), generated)?;

    std::fs::write(
        format!("{}/build-vars.rs", out_dir),
        format!(
            r#"// Build variables
// Defaults baked from the build profile; DOMAIN, PORT, SITE_NAME and
// TRAFFIC_LIMIT can be overridden at runtime via CLOUDPUB_* env vars
// (see runtime statics in lib.rs).
pub const DEFAULT_DOMAIN: &str = "cloudpub.ru";
pub const DEFAULT_PORT: u16 = 443;
pub const VERSION: &str = {:?};
pub const LONG_VERSION: &str = "{}-{} (cloudpub.ru)";
pub const DEFAULT_SITE_NAME: &str = "CloudPub";
pub const ONPREM: bool = false;
pub const BRANCH: &str = "stable";
#[cfg(target_pointer_width = "64")]
pub const DEFAULT_TRAFFIC_LIMIT: usize = 0;
"#,
            std::env::var("CARGO_PKG_VERSION").unwrap(),
            std::env::var("CARGO_PKG_VERSION").unwrap(),
            chrono::Local::now().format("%Y%m%d%H%M%S"),
        ),
    )?;

    Ok(())
}
