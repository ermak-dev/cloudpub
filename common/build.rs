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

    println!("cargo:rerun-if-env-changed=NEXT_PUBLIC_DOMAIN");
    println!("cargo:rerun-if-env-changed=NEXT_PUBLIC_VERSION");
    println!("cargo:rerun-if-env-changed=NEXT_PUBLIC_ONPREM");
    println!("cargo:rerun-if-env-changed=NEXT_PUBLIC_SITE_NAME");

    std::fs::write(
        format!("{}/build-vars.rs", out_dir),
        format!(
            r#"// Build variables
pub const DOMAIN: &str = {:?};
pub const VERSION: &str = {:?};
pub const LONG_VERSION: &str = "{}-{} ({})";
pub const SITE_NAME: &str = {:?};
pub const ONPREM: bool = {};
"#,
            std::env::var("NEXT_PUBLIC_DOMAIN").unwrap(),
            std::env::var("NEXT_PUBLIC_VERSION").unwrap(),
            std::env::var("NEXT_PUBLIC_VERSION").unwrap(),
            chrono::Local::now().format("%Y%m%d%H%M%S"),
            std::env::var("NEXT_PUBLIC_DOMAIN").unwrap(),
            std::env::var("NEXT_PUBLIC_SITE_NAME").unwrap(),
            std::env::var("NEXT_PUBLIC_ONPREM").unwrap()
        ),
    )?;

    Ok(())
}
