use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Inject crate version into version.rs
    File::create(out.join("version.rs"))
        .unwrap()
        .write_all(
            format!(
                r##"
pub const CRATE_VERSION: u32 = 0x{:02x}{:04x}{:02x};
"##,
                env!("CARGO_PKG_VERSION_MAJOR")
                    .parse::<u8>()
                    .expect("should have major version"),
                env!("CARGO_PKG_VERSION_MINOR")
                    .parse::<u16>()
                    .expect("should have minor version"),
                env!("CARGO_PKG_VERSION_PATCH")
                    .parse::<u8>()
                    .expect("should have patch version"),
            )
            .as_bytes(),
        )
        .unwrap();
}
