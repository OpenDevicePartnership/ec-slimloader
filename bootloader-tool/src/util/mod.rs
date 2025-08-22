use anyhow::Context;
use itertools::Itertools;

pub fn parse_hex(s: &str) -> anyhow::Result<Vec<u8>> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16))
        .try_collect::<_, Vec<_>, _>()
        .context("Input not hexidecimal")
}

pub fn generate_hex(buf: &[u8]) -> String {
    let mut result = String::new();
    for b in buf {
        result.push_str(&format!("{b:02X}"));
    }
    result
}

pub fn bytes_to_u32_le(b: &[u8]) -> Vec<u32> {
    b.chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<u32>>()
}

pub fn bytes_to_u32_be(b: &[u8]) -> Vec<u32> {
    b.chunks_exact(4)
        .map(|chunk| u32::from_be_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<u32>>()
}
