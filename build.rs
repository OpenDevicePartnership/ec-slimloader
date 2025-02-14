use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    if env::var("CARGO_FEATURE_IMXRT").is_ok() {
        // Put corresponding linker script in our output directory and ensure it's
        // on the linker search path.
        let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
        File::create(out.join("link-imxrt.x"))
            .unwrap()
            .write_all(include_bytes!("link-imxrt.x"))
            .unwrap();
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rerun-if-changed=link-imxrt.x");
        println!("cargo:rustc-link-arg=-Tlink-imxrt.x");

        // Inject crate version into the .biv section.
        File::create(out.join("biv.rs"))
            .unwrap()
            .write_all(
                format!(
                    r##"
#[link_section = ".biv"]
#[used]
static BOOT_IMAGE_VERSION: u32 = 0x{:02x}{:02x}{:02x}00;
"##,
                    env!("CARGO_PKG_VERSION_MAJOR")
                        .parse::<u8>()
                        .expect("should have major version"),
                    env!("CARGO_PKG_VERSION_MINOR")
                        .parse::<u8>()
                        .expect("should have minor version"),
                    env!("CARGO_PKG_VERSION_PATCH")
                        .parse::<u8>()
                        .expect("should have patch version"),
                )
                .as_bytes(),
            )
            .unwrap();
    }
}
