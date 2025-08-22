use anyhow::Context;
use object::elf::{SHT_NOBITS, SHT_PROGBITS};
use object::read::elf::{ElfFile32, ProgramHeader};
use object::{Object, ObjectSegment};
use std::ops::Range;

const PRELUDE_ADDRESS_RANGE: Range<u32> = 0x08000000..0x08001000;

pub fn objcopy(file: &ElfFile32) -> anyhow::Result<(Vec<u8>, u32)> {
    // Sanity checks
    let mut last_paddr = 0;
    let mut segments = vec![];
    for segment in file.segments() {
        let filesz = segment.elf_program_header().p_filesz(file.endianness());
        let memsz = segment.elf_program_header().p_memsz(file.endianness());

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

        let paddr = segment.elf_program_header().p_paddr(file.endianness());
        if PRELUDE_ADDRESS_RANGE.contains(&paddr) {
            continue;
        }
        if paddr < last_paddr {
            return Err(anyhow::anyhow!(
                "Segments not in order of physical address or overlapping segments"
            ));
        }
        last_paddr = paddr + segment.elf_program_header().p_memsz(file.endianness());

        segments.push(segment);
    }

    let base_addr = segments
        .iter()
        .map(|segment| segment.elf_program_header().p_paddr.get(file.endianness()))
        .min()
        .unwrap();
    let top_addr = segments
        .iter()
        .map(|segment| {
            segment.elf_program_header().p_paddr(file.endianness())
                + segment.elf_program_header().p_filesz(file.endianness())
        })
        .max()
        .unwrap();
    let output_size = top_addr - base_addr;

    log::debug!("Image base address: 0x{base_addr:0x}");
    log::debug!("Image entry address: 0x{:0x}", file.entry());
    log::debug!("Image output size: 0x{output_size:0x}");

    // The bootrom will start executing at offset 0x130 of the final image. Add 1 for thumb mode.
    let expected_entry = (base_addr + 0x131) as u64;
    if file.entry() != expected_entry {
        return Err(anyhow::anyhow!(format!(
            "Image entrypoint 0x{:0x} not at expected address 0x{:0x}",
            file.entry(),
            expected_entry
        )));
    }

    // TODO check VTOR
    // TODO check image size
    // TODO check execution address

    // Assemble BIN image by copying all segments directly
    let mut image = vec![0; output_size as usize];
    for segment in segments {
        let paddr = segment.elf_program_header().p_paddr(file.endianness());

        image[paddr as usize - base_addr as usize
            ..paddr as usize - base_addr as usize + segment.size() as usize]
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
