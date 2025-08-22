# ec-slimloader-descriptors

Descriptors for use with the ec-slimloader bootloader. Can be leveraged by an OTA update process such as CFU to enable firmware update.

## dependencies & acknowledgements

constmuck (zlib) is used for raw byte interpretation for compile time computation of CRCs.

## theory of operation

The descriptors are built into two pieces: a descriptor header (BootableRegionDescriptorHeader) and a list of application image descriptors (AppImageDescriptor). Together, these describe the layout of a bootable region. These can be managed via a manager struct (BootableRegionDescriptors).

The expected boot flow would be as follows:

1. Bootloader reads BootableRegionDescriptorHeader from chip-specific offset
2. Descriptor header is validated and CRC is checked. App image descriptors are also validated for integrity.
3. Bootloader loads the active app image descriptor per the active slot indicated in the header
4. Depending on image flags, bootloader may perform a CRC integrity check over the app image in place
5. Depending on image flags, the bootloader may copy the validated image from stored_image_address to execution_image_address
6. If everything has been validated and copied, the bootloader will set the VTOR and main stack pointer, then branch to the reset vector

## format

| BootableRegionDescriptorHeader | | |
| ----- | ----- | ----------- |
| Field | Width | Description |
| signature | u32 | 0x22222222 |
| descriptor_version | u32 | crate version |
| descriptor_header_size_bytes | u32 | size of this header (must match crate version) |
| app_descriptor_size_bytes | u32 | size of each app image descriptor (must match crate version) |
| app_descriptor_base_address | u32 | start of AppImageDescriptor region |
| num_app_slots | u32 | number of AppImageDescriptors located at app_descriptor_base_address |
| active_app_slot | u32 | current active app image to boot to |
| header_crc | u32 | CRC over above fields |
| Total size | 32 | bytes |

| AppImageDescriptor | | |
| ----- | ----- | ----------- |
| Field | Width | Description |
| descriptor_version | u32 | crate version |
| app_slot_number | u32 | which slot this descriptor corresponds to |
| app_version | u32 | application firmware version, useful for fallback or rollback protection |
| security_version | u32 | application security version, useful for rollback protection |
| flags | u32 | app image flags, such as ignore CRC or copy to RAM |
| stored_address | u32 | typically a flash memory mapped address to read the bootable image from |
| image_size_bytes | u32 | size of the whole image at stored_address |
| stored_crc_address | u32 | if CRC check is enabled, the address where this integrity CRC is held |
| execution_copy_size_bytes | u32 | how much to copy to execution_address, typically the same as image_size_bytes or 0 |
| execution_address | u32 | where to begin execution from, the same as stored_address if XIP |
| descriptor_crc | u32 | CRC over above fields |
| Total size | 44 | bytes |
