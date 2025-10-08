use std::path::PathBuf;

use anyhow::Context;
use bootloader_tool::Config;
use bootloader_tool::processors::mbi::{self, cert_block};
use bootloader_tool::processors::objcopy;
use bootloader_tool::processors::otp::Otp;
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
    const CERTIFICATE_IDX: usize = 1;

    let config = Config::read("config.toml").unwrap();
    let (data, base_addr) = read_example("application");
    assert_same(&data, base_addr, false, None, &config, CERTIFICATE_IDX);
}

#[test]
fn test_bootloader() {
    test_bootloader_padding(0);
}

#[test]
fn test_bootloader_padding_1() {
    test_bootloader_padding(1);
}

#[test]
fn test_bootloader_padding_5() {
    test_bootloader_padding(5);
}
#[test]
fn test_bootloader_padding_9() {
    test_bootloader_padding(9);
}
#[test]
fn test_bootloader_padding_17() {
    test_bootloader_padding(17);
}

fn test_bootloader_padding(added_bytes: u8) {
    const CERTIFICATE_IDX: usize = 0;

    let config = Config::read("config.toml").unwrap();

    let (mut data, base_addr) = read_example("bootloader");
    for i in 0..added_bytes {
        data.push(0x42 + i);
    }
    assert_same(&data, base_addr, true, None, &config, CERTIFICATE_IDX);
}

fn assert_same(
    input_data: &[u8],
    base_addr: u32,
    is_bootloader: bool,
    otp: Option<Otp>,
    config: &Config,
    certificate_idx: usize,
) {
    let output_dir = tempfile::tempdir().unwrap();

    let input_path = output_dir.path().join("input.bin");
    std::fs::write(&input_path, input_data).unwrap();

    let pure_out = output_dir.path().join("pure.bin");
    let nxp_out = output_dir.path().join("nxp.bin");

    let cert_block = cert_block::generate("nxpimage", &config, certificate_idx).unwrap();
    let private_key_path = get_private_key(&config, certificate_idx);
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

    let cert_block_config = cert_block::generate_config(&config, certificate_idx, None as Option<PathBuf>);
    mbi::generate_nxp(
        "nxpimage",
        &input_path,
        base_addr,
        &nxp_out,
        is_bootloader,
        cert_block_config,
    )
    .unwrap();

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
