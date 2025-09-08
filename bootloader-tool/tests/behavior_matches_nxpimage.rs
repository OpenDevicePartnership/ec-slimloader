use anyhow::Context;
use bootloader_tool::Config;
use bootloader_tool::processors::certificates::{Rkth, get_rkth};
use bootloader_tool::processors::otp::Otp;
use bootloader_tool::processors::{mbi, objcopy, otp};
use object::read::elf::ElfFile32;

#[test]
fn test_app() {
    let config = Config::read("config.toml").unwrap();
    let rkth = get_rkth(&config).unwrap();

    let (data, base_addr) = read_example("application");
    assert_same(&data, base_addr, false, None, &rkth);
}

#[test]
fn test_bootloader() {
    let config = Config::read("config.toml").unwrap();
    let otp = otp::generate(&config).unwrap();
    let rkth = get_rkth(&config).unwrap();

    let (data, base_addr) = read_example("bootloader");
    assert_same(&data, base_addr, true, Some(otp), &rkth);
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
    let config = Config::read("config.toml").unwrap();
    let otp = otp::generate(&config).unwrap();
    let rkth = get_rkth(&config).unwrap();

    let (mut data, base_addr) = read_example("bootloader");
    for i in 0..added_bytes {
        data.push(0x42 + i);
    }
    assert_same(&data, base_addr, true, Some(otp), &rkth);
}

fn assert_same(input_data: &[u8], base_addr: u32, is_bootloader: bool, otp: Option<Otp>, rkth: &Rkth) {
    let output_dir = tempfile::tempdir().unwrap();

    let input_path = output_dir.path().join("input.bin");
    std::fs::write(&input_path, input_data).unwrap();

    let pure_out = output_dir.path().join("pure.bin");
    let nxp_out = output_dir.path().join("nxp.bin");

    mbi::generate_pure(&input_path, base_addr, &pure_out, is_bootloader, otp, rkth).unwrap();
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
