use std::collections::BTreeSet;
use std::ops::Range;

use anyhow::Context;
use object::elf::{SHT_NOBITS, SHT_PROGBITS};
use object::read::elf::{ElfFile32, ProgramHeader};
use object::{Object, ObjectSegment};

use crate::config::AddressMapping;

const PRELUDE_ADDRESS_RANGE: Range<u32> = 0x08000000..0x08001000;

#[derive(Debug)]
pub struct Config<'a> {
    pub mappings: &'a BTreeSet<AddressMapping>,
    pub max_size: Option<u32>,
}

impl Config<'_> {
    pub fn map_paddr(&self, paddr: u32) -> u32 {
        for mapping in self.mappings {
            let range = mapping.from_start..=mapping.from_end;
            if range.contains(&(paddr as u64)) {
                let offset = paddr - mapping.from_start as u32;
                let new_paddr = mapping.to_start as u32 + offset;
                log::debug!("Mapped section 0x{paddr:0x} to 0x{new_paddr:0x}");
                return new_paddr;
            }
        }
        paddr
    }
}

struct Segment<'a> {
    paddr: u32,
    data: &'a [u8],
}

fn get_segments<'a>(
    file_i: usize,
    file: &ElfFile32<'a>,
    config: &Config,
    last_paddr: &mut u32,
) -> Result<Vec<Segment<'a>>, anyhow::Error> {
    // Segments must be globally ordered.
    // If the files are not passed in the correct order, an error is thrown.
    let mut segments = vec![];
    let endianness = file.endianness();

    for segment in file.segments() {
        let filesz = segment.elf_program_header().p_filesz(endianness);
        let memsz = segment.elf_program_header().p_memsz(endianness);

        if filesz == 0 {
            // Skip bss to reduce size of image to flash. The bss will be cleared during startup anyway.
            continue;
        }

        if filesz > memsz {
            return Err(anyhow::anyhow!("p_filesz larger than p_memsz"));
        }
        if memsz > filesz {
            return Err(anyhow::anyhow!("Segment only partially a bss segment"));
        }

        let paddr = config.map_paddr(segment.elf_program_header().p_paddr(endianness));
        if PRELUDE_ADDRESS_RANGE.contains(&paddr) {
            continue;
        }

        if paddr < *last_paddr {
            return Err(anyhow::anyhow!(
                "Segments not in order of physical address or overlapping segments"
            ));
        }

        let data = segment.data().unwrap();
        *last_paddr = paddr + data.len() as u32;

        segments.push(Segment { data, paddr });
    }

    let base_addr = segments.iter().map(|segment| segment.paddr).min().unwrap();
    let top_addr = segments
        .iter()
        .map(|segment| segment.paddr + segment.data.len() as u32)
        .max()
        .unwrap();
    let output_size = top_addr - base_addr;

    log::debug!("Image[{file_i}] base address: 0x{base_addr:0x}");
    log::debug!("Image[{file_i}] entry address: 0x{:0x}", file.entry());
    log::debug!("Image[{file_i}] output size: 0x{output_size:0x}");

    // The bootrom will start executing at offset 0x130 of the final image. Add 1 for thumb mode.
    let expected_entry = base_addr + 0x131;
    let actual_entry = config.map_paddr(file.entry() as u32);
    if actual_entry != expected_entry {
        return Err(anyhow::anyhow!(format!(
            "Image[{file_i}] entrypoint 0x{:0x} not at expected address 0x{:0x}",
            actual_entry, expected_entry
        )));
    }

    Ok(segments)
}

/// Copy one or more ELF files into a binary format.
///
/// Optionally moves the physical load address (LMA) according to the [AddressMapping].
///
/// Each file must be a valid binary image with a VTOR and file entry.
pub fn objcopy<'a>(
    files: impl IntoIterator<Item = &'a ElfFile32<'a>>,
    config: Config<'a>,
) -> anyhow::Result<(Vec<u8>, u32)> {
    // Segments must be globally ordered.
    // If the files are not passed in the correct order, an error is thrown.
    let mut segments = vec![];
    let mut last_paddr = 0;

    for (file_i, file) in files.into_iter().enumerate() {
        segments.extend(get_segments(file_i, file, &config, &mut last_paddr)?);
    }

    let base_addr = segments.iter().map(|segment| segment.paddr).min().unwrap();
    let top_addr = segments
        .iter()
        .map(|segment| segment.paddr + segment.data.len() as u32)
        .max()
        .unwrap();
    let output_size = top_addr - base_addr;

    log::debug!("Image base address: 0x{base_addr:0x}");
    log::debug!("Image output size: 0x{output_size:0x}");

    // TODO check VTOR
    // TODO check execution address

    if let Some(max_size) = config.max_size
        && output_size > max_size
    {
        return Err(anyhow::anyhow!(
            "Image output size 0x{output_size:0x} exceeded maximum size 0x{max_size:0x}"
        ));
    }

    // Assemble BIN image by copying all segments directly
    let mut image = vec![0; output_size as usize];
    for segment in segments {
        let paddr = segment.paddr;

        image[paddr as usize - base_addr as usize..paddr as usize - base_addr as usize + segment.data.len()]
            .copy_from_slice(segment.data);
    }

    Ok((image, base_addr))
}

pub fn remove_non_prelude(file: &ElfFile32) -> anyhow::Result<Vec<u8>> {
    let mut builder = object::build::elf::Builder::read32(file.data()).context("Could not parse ELF")?;

    for section in builder.sections.iter_mut() {
        if PRELUDE_ADDRESS_RANGE.contains(&(section.sh_addr as u32)) {
            // This segment is part of the prelude
            continue;
        }

        if section.sh_type != SHT_PROGBITS && section.sh_type != SHT_NOBITS {
            // This is not a data section so keep it
            continue;
        }

        section.delete = true;
    }

    builder.delete_orphans();

    let mut out = vec![];
    builder.write(&mut out)?;
    Ok(out)
}
