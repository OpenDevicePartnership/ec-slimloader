use std::collections::BTreeSet;
use std::ops::Range;

use anyhow::Context;
use object::elf::{SHT_NOBITS, SHT_PROGBITS};
use object::read::elf::{ElfFile32, ProgramHeader};
use object::{Object, ObjectSegment};

use crate::config::AddressMapping;

const PRELUDE_ADDRESS_RANGE: Range<u32> = 0x08000000..0x08001000;

pub fn objcopy<'a>(
    files: impl Iterator<Item = &'a ElfFile32<'a>>,
    mappings: &BTreeSet<AddressMapping>,
) -> anyhow::Result<(Vec<u8>, u32)> {
    let mut segments = vec![];
    let mut endianness = Option::None;

    // Each file has their own entry point, which we should check.
    // Collect all the segments for concattenation later.
    for (file_i, file) in files.enumerate() {
        if let Some(e) = endianness {
            if e != file.endianness() {
                return Err(anyhow::anyhow!("Passed ELFs with distinct endianness"));
            }
        } else {
            endianness = Some(file.endianness());
        }
        let endianness = endianness.unwrap();

        let mut last_paddr = 0;

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

            let paddr = segment.elf_program_header().p_paddr(endianness);
            if PRELUDE_ADDRESS_RANGE.contains(&paddr) {
                continue;
            }
            if paddr < last_paddr {
                return Err(anyhow::anyhow!(
                    "Segments not in order of physical address or overlapping segments"
                ));
            }
            last_paddr = paddr + segment.elf_program_header().p_memsz(endianness);

            segments.push(segment);
        }

        let base_addr = segments
            .iter()
            .map(|segment| segment.elf_program_header().p_paddr.get(endianness))
            .min()
            .unwrap();
        let top_addr = segments
            .iter()
            .map(|segment| {
                segment.elf_program_header().p_paddr(endianness) + segment.elf_program_header().p_filesz(endianness)
            })
            .max()
            .unwrap();
        let output_size = top_addr - base_addr;

        log::debug!("Image[{file_i}] base address: 0x{base_addr:0x}");
        log::debug!("Image[{file_i}] entry address: 0x{:0x}", file.entry());
        log::debug!("Image[{file_i}] output size: 0x{output_size:0x}");

        // The bootrom will start executing at offset 0x130 of the final image. Add 1 for thumb mode.
        let expected_entry = (base_addr + 0x131) as u64;
        if file.entry() != expected_entry {
            return Err(anyhow::anyhow!(format!(
                "Image[{file_i}] entrypoint 0x{:0x} not at expected address 0x{:0x}",
                file.entry(),
                expected_entry
            )));
        }
    }

    let endianness = endianness.unwrap();

    let base_addr = segments
        .iter()
        .map(|segment| segment.elf_program_header().p_paddr.get(endianness))
        .min()
        .unwrap();
    let top_addr = segments
        .iter()
        .map(|segment| {
            segment.elf_program_header().p_paddr(endianness) + segment.elf_program_header().p_filesz(endianness)
        })
        .max()
        .unwrap();
    let output_size = top_addr - base_addr;

    log::debug!("Image base address: 0x{base_addr:0x}");
    log::debug!("Image output size: 0x{output_size:0x}");

    // TODO check VTOR
    // TODO check image size
    // TODO check execution address

    // Assemble BIN image by copying all segments directly
    let mut image = vec![0; output_size as usize];
    for segment in segments {
        let paddr = segment.elf_program_header().p_paddr(endianness);

        image[paddr as usize - base_addr as usize..paddr as usize - base_addr as usize + segment.size() as usize]
            .copy_from_slice(segment.data().unwrap());
    }

    Ok((image, base_addr))
}

pub fn remove_non_prelude(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut builder = object::build::elf::Builder::read32(data).context("Could not parse ELF")?;

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
