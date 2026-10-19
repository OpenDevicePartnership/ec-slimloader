#![no_std]

#[cfg(feature = "bootloader")]
pub mod bootloader {
    partition_manager::macros::create_partition_map!(
        name: ExternalStorageConfig,
        map_name: ExternalStorageMap,
        variant: "bootloader",
        manifest: "src/ext-flash.toml"
    );
}

#[cfg(feature = "application")]
pub mod application {
    partition_manager::macros::create_partition_map!(
        name: ExternalStorageConfig,
        map_name: ExternalStorageMap,
        variant: "application",
        manifest: "src/ext-flash.toml"
    );
}
