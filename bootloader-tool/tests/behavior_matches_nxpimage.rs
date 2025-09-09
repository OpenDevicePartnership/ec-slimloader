use std::path::{Path, PathBuf};

use anyhow::Context;
use bootloader_tool::Config;
use bootloader_tool::processors::mbi::cert_block::{self, CertBlock};
use bootloader_tool::processors::otp::Otp;
use bootloader_tool::processors::{mbi, objcopy, otp};
use object::read::elf::ElfFile32;

fn get_private_key(config: &Config, certificate_idx: usize) -> PathBuf {
    config.certificates[certificate_idx]
        .0
        .last()
        .as_ref()
        .unwrap()
        .prototype
        .as_ref()
        .unwrap()
        .key_path
        .clone()
}

#[test]
fn test_app() {
    const CERTIFICATE_IDX: usize = 0;

    let config = Config::read("config.toml").unwrap();
    let cert_block = cert_block::generate("nxpimage", &config, CERTIFICATE_IDX).unwrap();
    let private_key_path = get_private_key(&config, CERTIFICATE_IDX);

    let (data, base_addr) = read_example("application");
    assert_same(&data, base_addr, false, None, cert_block, private_key_path);
}

#[test]
fn test_bootloader() {
    const CERTIFICATE_IDX: usize = 0;

    let config = Config::read("config.toml").unwrap();
    let otp = otp::generate(&config).unwrap();
    let cert_block = cert_block::generate("nxpimage", &config, CERTIFICATE_IDX).unwrap();
    let private_key_path = get_private_key(&config, CERTIFICATE_IDX);

    let (data, base_addr) = read_example("bootloader");
    assert_same(&data, base_addr, true, Some(otp), cert_block, private_key_path);
}

#[test]
fn test_bootloader_padding_1() {
    test_bootloader_padding(1);
}

#[test]
fn test_bootloader_padding_5() {
    test_bootloader_padding(1);
}
#[test]
fn test_bootloader_padding_9() {
    test_bootloader_padding(1);
}
#[test]
fn test_bootloader_padding_17() {
    test_bootloader_padding(1);
}

fn test_bootloader_padding(added_bytes: u8) {
    const CERTIFICATE_IDX: usize = 0;

    let config = Config::read("config.toml").unwrap();
    let otp = otp::generate(&config).unwrap();
    let cert_block = cert_block::generate("nxpimage", &config, CERTIFICATE_IDX).unwrap();
    let private_key_path = config.certificates[CERTIFICATE_IDX]
        .0
        .last()
        .as_ref()
        .unwrap()
        .prototype
        .as_ref()
        .unwrap()
        .key_path
        .clone();

    let (mut data, base_addr) = read_example("bootloader");
    for i in 0..added_bytes {
        data.push(0x42 + i);
    }
    assert_same(&data, base_addr, true, Some(otp), cert_block, private_key_path);
}

fn assert_same(
    input_data: &[u8],
    base_addr: u32,
    is_bootloader: bool,
    otp: Option<Otp>,
    cert_block: CertBlock,
    private_key_path: impl AsRef<Path>,
) {
    let output_dir = tempfile::tempdir().unwrap();

    let input_path = output_dir.path().join("input.bin");
    std::fs::write(&input_path, input_data).unwrap();

    let pure_out = output_dir.path().join("pure.bin");
    let nxp_out = output_dir.path().join("nxp.bin");

    mbi::generate_pure(
        &input_path,
        base_addr,
        &pure_out,
        is_bootloader,
        otp,
        cert_block,
        private_key_path,
    )
    .unwrap();
    mbi::generate("nxpimage", &input_path, base_addr, &nxp_out, is_bootloader).unwrap();

    let pure = std::fs::read(&pure_out).unwrap();
    let nxp = std::fs::read(&nxp_out).unwrap();

    if pure != nxp {
        let evidence = output_dir.keep();
        panic!("Outputs differ, see {} for generated files.", evidence.display());
    }
}

fn read_example(app_or_boot: &str) -> (Vec<u8>, u32) {
    let path = format!("../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-{app_or_boot}",);
    let input = match std::fs::read(&path) {
        Ok(input) => input,
        Err(e) => {
            panic!(
                "Could not load example binary at '{path}'!\n -> Go to example/{app_or_boot} and run cargo build --release.\nError: {e}",
            );
        }
    };
    let file = ElfFile32::parse(&input[..])
        .context("Could not parse ELF file")
        .unwrap();
    objcopy::objcopy(&file).unwrap()
}
