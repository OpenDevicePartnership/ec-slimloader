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
        AppImageDescriptor::new_ram_image(0, 0x008_100_000, 0x100_000),
        AppImageDescriptor::new_ram_image(1, 0x008_200_000, 0x100_000),
        AppImageDescriptor::new_ram_image(2, 0x008_300_000, 0x100_000),
        AppImageDescriptor::new_ram_image(3, 0x008_400_000, 0x100_000),
    ],
};
