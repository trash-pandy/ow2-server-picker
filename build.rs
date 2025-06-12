use embed_manifest::manifest::{ExecutionLevel, Setting};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(
            new_manifest("dropship-rs")
                .long_path_aware(Setting::Enabled)
                .requested_execution_level(ExecutionLevel::RequireAdministrator),
        )
        .expect("unable to embed manifest file");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
