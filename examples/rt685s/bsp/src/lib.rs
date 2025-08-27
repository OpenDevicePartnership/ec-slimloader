#![no_std]

#[cfg(feature = "bootloader")]
partition_manager::macros::create_partition_map!(
    name: ExternalStorageConfig,
    map_name: ExternalStorageMap,
    variant: "bootloader",
    manifest: "src/ext-flash.toml"
);

#[cfg(feature = "application")]
partition_manager::macros::create_partition_map!(
    name: ExternalStorageConfig,
    map_name: ExternalStorageMap,
    variant: "application",
    manifest: "src/ext-flash.toml"
);
