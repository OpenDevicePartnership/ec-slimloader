use ec_slimloader_descriptors::{AppImageDescriptor, BootableRegionDescriptorHeader};

#[repr(C)]
struct DescriptorBlock {
    header: BootableRegionDescriptorHeader,
    images: [AppImageDescriptor; 4],
}

#[link_section = ".descriptors"]
#[used]
static DESCRIPTORS: DescriptorBlock = DescriptorBlock {
    header: BootableRegionDescriptorHeader::new(
        4,
        0x08_009_000 + core::mem::size_of::<BootableRegionDescriptorHeader>() as u32,
    ),
    images: [
        AppImageDescriptor::new_ram_image(0, 0x800_D000, 1024 * 944),
        AppImageDescriptor::new_ram_image(1, 0x80F_9000, 1024 * 944),
    ],
};
