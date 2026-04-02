// AHAB container + certificate parsing for MCXA family with PQC support.
// Supports hybrid keys: ECDSA P-384 and ML-DSA-87.
use core::mem::size_of;
use embassy_mcxa::{peripherals, Peri};

macro_rules! cert_trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "certificate-logging")]
        {
            defmt_or_log::trace!($($arg)*);
        }
    };
}

macro_rules! cert_error {
    ($($arg:tt)*) => {
        #[cfg(feature = "certificate-logging")]
        {
            defmt_or_log::error!($($arg)*);
        }
    };
}

// 384-bit Root Key Table Hash (SHA-384 digest of RoTK public key X||Y)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rkth([u8; 48]);

impl Rkth {
    pub fn as_be_words(&self) -> [u32; 12] {
        let mut w = [0u32; 12];
        for (i, chunk) in self.0.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        w
    }

    pub fn as_le_words(&self) -> [u32; 12] {
        let mut w = [0u32; 12];
        for (i, chunk) in self.0.chunks_exact(4).enumerate() {
            w[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        w
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug)]
pub enum CertError {
    TooLarge,
    Magic,
    SizeField,
    Bounds,
    EcType,
    Offset,
    SignatureMissing,
    Align,
    Tag,
    Version,
}

// ===== AHAB structures (per MCXN556S RM tables) =====

// Minimal parsed view to feed ROM authentication
pub struct AhabParsed {
    pub container: *const AhabContainerHeaderRaw,
    pub images: *const AhabImageEntryRaw,
    pub images_count: usize,
    pub sigblk: *const AhabSignatureBlockRaw,
    pub srk_array_ptr: *const u8,
    pub srk_array_len: usize,
    pub cert_ptr: *const u8,
    pub cert_len: usize,
    pub sig_ptr: *const u8,
    pub sig_len: usize,
}

// Deeper parser structures
pub struct ParsedSrkRecord<'a> {
    pub hdr: &'a AhabSrkRecordRaw,
}

pub struct ParsedSrkTable<'a> {
    pub hdr: &'a AhabSrkTableHeaderRaw,
    pub records: &'a [AhabSrkRecordRaw],
    pub raw_table_bytes: &'a [u8], // Complete 308-byte SRK table for RKTH calculation
}

pub struct ParsedSrkArray<'a> {
    pub hdr: &'a AhabSrkArrayHeaderRaw,
    pub ecdsa_table: ParsedSrkTable<'a>, // ECDSA table (up to 4 ECDSA SRK records)
    pub mldsa_table: ParsedSrkTable<'a>, // ML-DSA table (up to 4 ML-DSA SRK records)
}

pub struct ParsedCertificate<'a> {
    pub hdr: &'a AhabCertificateHeaderRaw,
    pub payload: &'a [u8],
    pub signature_region: &'a [u8],
}

pub struct ParsedSignatures<'a> {
    pub hdr: &'a AhabSignatureHeaderRaw,
    pub payload: &'a [u8],
}

fn sha512_rkth_48(peri: Peri<'_, peripherals::SGI0>, input: &[u8]) -> Option<[u8; 48]> {
    let mut blocking_hasher = crate::BlockingHasher::new(peri);

    blocking_hasher.hsm_sha512_rkth(input)
}

#[repr(C)]
pub struct AhabContainerHeaderRaw {
    // Word 0: Tag (31-24), Length (23-8), Version (7-0)
    pub word0: u32, // Tag(31-24) | Length(23-8) | Version(7-0)
    // Word 1: Flags
    pub flags: u32, // SRK set (3-0), SRK selection (5-4), reserved (31-6)
    // Word 2: # of images (31-24), Fuse version (23-16), SW version (15-0)
    pub word2: u32, // Images(31-24) | Fuse ver(23-16) | SW ver(15-0)
    // Word 3: Reserved (31-24), Cert version (23-16), Signature block offset (15-0)
    pub word3: u32, // Reserved(31-24) | Cert ver(23-16) | Sigblk offset(15-0)
}

impl AhabContainerHeaderRaw {
    /// Get tag from word0 (bits 31-24)
    pub fn tag(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get the total container length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get version from word0 (bits 7-0)
    pub fn version(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get image count from word2 (bits 31-24)
    pub fn image_count(&self) -> u8 {
        (self.word2 >> 24) as u8
    }

    /// Get fuse version from word2 (bits 23-16)
    pub fn fuse_version(&self) -> u8 {
        ((self.word2 >> 16) & 0xFF) as u8
    }

    /// Get software version from word2 (bits 15-0)
    pub fn sw_version(&self) -> u16 {
        (self.word2 & 0xFFFF) as u16
    }

    /// Get certificate version from word3 (bits 23-16)
    pub fn cert_version(&self) -> u8 {
        ((self.word3 >> 16) & 0xFF) as u8
    }

    /// Get the signature block offset from word3 (bits 15-0)
    pub fn signature_block_offset(&self) -> u32 {
        (self.word3 & 0xFFFF) as u32
    }

    /// Get SRK set from flags (bits 3-0)
    pub fn srk_set(&self) -> u8 {
        (self.flags & 0xF) as u8
    }

    /// Get SRK selection from flags (bits 5-4)
    pub fn srk_selection(&self) -> u8 {
        ((self.flags >> 4) & 0x3) as u8
    }

    /// Compute combined version for anti-rollback check
    pub fn combined_version(&self) -> u16 {
        (self.sw_version() << 8) | (self.fuse_version() as u16)
    }
}

#[repr(C)]
pub struct AhabImageEntryRaw {
    // Word 0: Image offset (32 bits)
    pub offset: u32, // Offset in bytes from start of container header to beginning of image
    // Word 1: Image size (32 bits)
    pub size: u32, // Size of the image in bytes
    // Words 2-3: Load address (64 bits - high word set to zero on MCXN556S)
    pub load_address: u64, // Address where image is copied to in memory by ROM
    // Words 4-5: Entry point (64 bits - high word set to zero on MCXN556S)
    pub entry_point: u64, // Entry point of the image (absolute address)
    // Word 6: Flags (32 bits)
    pub flags: u32, // Image type (3-0), Core ID (7-4), Hash type (11-8), Reserved (31-12)
    // Word 7: Reserved (32 bits)
    pub reserved1: u32, // Reserved
    // Words 8-23: Hash (512 bits = 64 bytes, left-aligned and zero-padded)
    pub hash: [u8; 64], // Hash of image (SHA2_384 or SHA2_512)
    // Words 24-31: Reserved (256 bits = 32 bytes, set to 0)
    pub reserved2: [u8; 32], // Unused, set to 256'b0
}

impl AhabImageEntryRaw {
    /// Get image type from flags (bits 3-0)
    pub fn image_type(&self) -> u8 {
        (self.flags & 0xF) as u8
    }

    /// Check if this is an SB4 file
    pub fn is_sb4_file(&self) -> bool {
        self.image_type() == 0xF
    }

    /// Get core ID from flags (bits 7-4) - Reserved on MCXN556S
    pub fn core_id(&self) -> u8 {
        ((self.flags >> 4) & 0xF) as u8
    }

    /// Get hash type from flags (bits 11-8)
    pub fn hash_type(&self) -> u8 {
        ((self.flags >> 8) & 0xF) as u8
    }

    /// Check if hash type is SHA2_384
    pub fn is_sha384(&self) -> bool {
        self.hash_type() == 0x1
    }

    /// Check if hash type is SHA2_512
    pub fn is_sha512(&self) -> bool {
        self.hash_type() == 0x2
    }
}

#[repr(C)]
pub struct AhabSignatureBlockRaw {
    // Word 0: Tag (31-24), Length (23-8), Version (7-0)
    pub word0: u32, // Tag(31-24) | Length(23-8) | Version(7-0)
    // Word 1: SRK table array offset (31-16), Certificate offset (15-0)
    pub word1: u32, // SRK offset(31-16) | Cert offset(15-0)
    // Word 2: Reserved (31-16), Signature offset (15-0)
    pub word2: u32, // Reserved(31-16) | Signature offset(15-0)
    // Word 3: Reserved (48 bits total, continued from Word 2)
    pub reserved2: u32, // bits 31-0: Reserved (completion of 48-bit reserved field)
}

impl AhabSignatureBlockRaw {
    /// Get tag from word0 (bits 31-24)
    pub fn tag(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get version from word0 (bits 7-0)
    pub fn version(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get SRK array offset from word1 (bits 31-16)
    pub fn srk_array_offset(&self) -> u16 {
        (self.word1 >> 16) as u16
    }

    /// Get certificate offset from word1 (bits 15-0)
    pub fn cert_offset(&self) -> u16 {
        (self.word1 & 0xFFFF) as u16
    }

    /// Get signature offset from word2 (bits 15-0)
    pub fn signature_offset(&self) -> u16 {
        (self.word2 & 0xFFFF) as u16
    }

    /// Get SRK array offset as u32 for compatibility
    pub fn srk_array_offset_u32(&self) -> u32 {
        self.srk_array_offset() as u32
    }

    /// Get certificate offset as u32 for compatibility  
    pub fn cert_offset_u32(&self) -> u32 {
        self.cert_offset() as u32
    }

    /// Get signature offset as u32 for compatibility
    pub fn signature_offset_u32(&self) -> u32 {
        self.signature_offset() as u32
    }

    /// Check if SRK table array is present (offset != 0x0000)
    pub fn has_srk_table(&self) -> bool {
        self.srk_array_offset() != 0x0000
    }

    /// Check if certificate is present (offset != 0x0000)
    /// If false, only SRK is used for signature verification
    pub fn has_certificate(&self) -> bool {
        self.cert_offset() != 0x0000
    }
}

// Note: Component ordering in signature block:
// 1. SRK table array (Required, starts on 64 bit boundary)
// 2. Certificate (optional, starts on 64 bit boundary)
// 3. Signature (required, starts on 64 bit boundary)
// Container signature covers all data from container header start to signature start.
// All padding must be zeros to maintain 64 bit alignment.

// Note: In hybrid mode, SRK selection targets both ECDSA and ML-DSA keys.
// For example, when selecting SRK0, both first SRKs of first and second
// SRK table will be used. No SRK index mix/match is supported.
//
// SRK table array structure:
//  Single mode: 1 SRK table (ECDSA only)
//    SRK table 0: Non-quantum resistant key (ECDSA)
//  Hybrid mode: 2 SRK tables (ECDSA + ML-DSA)
//    SRK table 0: Non-quantum resistant key (ECDSA)
//    SRK table 1: Quantum-resistant key (ML-DSA)
// Keys must match signing key in algorithm and key length.

#[repr(C)]
pub struct AhabSrkArrayHeaderRaw {
    // Word 0: Tag (31-24), Length (23-8), Version (7-0)
    pub word0: u32, // Tag(31-24) | Length(23-8) | Version(7-0)
    // Word 1: Reserved (31-8), # of SRK Tables (7-0)
    pub word1: u32, // Reserved(31-8) | SRK table count(7-0)
                    // Sequential layout after header:
                    // ECDSA table (SRK table 0) - starts immediately after header
                    // ML-DSA table (SRK table 1) - starts immediately after ECDSA data which is after ECDSA table
                    // SRK data sections follow after respective tables
}

impl AhabSrkArrayHeaderRaw {
    /// Get tag from word0 (bits 31-24)
    pub fn tag(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get version from word0 (bits 7-0)
    pub fn version(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get SRK table count from word1 (bits 7-0)
    pub fn srk_table_count(&self) -> u8 {
        (self.word1 & 0xFF) as u8
    }

    /// Check if this is hybrid mode (2 SRK tables: ECDSA + ML-DSA)
    pub fn is_hybrid_mode(&self) -> bool {
        self.srk_table_count() == 2
    }

    /// Check if this is single signature mode (1 SRK table: ECDSA only)
    pub fn is_single_mode(&self) -> bool {
        self.srk_table_count() == 1
    }

    /// Get number of signature algorithms supported
    pub fn signature_count(&self) -> u8 {
        self.srk_table_count()
    }
}

#[repr(C)]
pub struct AhabSrkTableHeaderRaw {
    // Word 0: Version (31-24), Length (23-8), Tag (7-0)
    pub word0: u32, // Version(31-24) | Length(23-8) | Tag(7-0)
                    // Words 1-4: SRK records 0-3 (each record is 32 bits)
                    // Followed by actual SRK record structures
}

impl AhabSrkTableHeaderRaw {
    /// Get version from word0 (bits 31-24)
    pub fn version(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get tag from word0 (bits 7-0)
    pub fn tag(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get the size of SRK table excluding SRK data
    pub fn table_size(&self) -> u16 {
        self.length()
    }

    /// Calculate number of SRK records in this table
    /// Each SRK record is a fixed size structure
    pub fn record_count(&self) -> usize {
        // After header (4 bytes), remaining bytes are SRK records
        let record_area_size = self.length().saturating_sub(4) as usize;
        record_area_size / core::mem::size_of::<AhabSrkRecordRaw>()
    }
}

#[repr(C)]
pub struct AhabSrkRecordRaw {
    // Word 0: Sign alg (31-24), Length (23-8), Tag (7-0)
    pub word0: u32, // Sign alg(31-24) | Length(23-8) | Tag(7-0)
    // Word 1: SRK flags (31-24), Reserved (23-16), Key size (15-8), Hash alg (7-0)
    pub word1: u32, // SRK flags(31-24) | Reserved(23-16) | Key size(15-8) | Hash alg(7-0)
    // Word 2: Parameter lengths (format type dependent)
    pub param_lens: u32, // ECDSA: X size (15-0), Y size (31-16) | ML-DSA: Raw key size (15-0)
    // Word 3+: Hash of public key (SRK data) - 512 bits = 16 words
    pub srk_data_hash: [u8; 64], // Hash of SRK Data, 512-bit, left-aligned and zero-padded
}

impl AhabSrkRecordRaw {
    /// Get sign algorithm from word0 (bits 31-24)
    pub fn sign_alg(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get tag from word0 (bits 7-0)
    pub fn tag(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get SRK flags from word1 (bits 31-24)
    pub fn srk_flags(&self) -> u8 {
        (self.word1 >> 24) as u8
    }

    /// Get reserved field from word1 (bits 23-16)
    pub fn reserved(&self) -> u8 {
        ((self.word1 >> 16) & 0xFF) as u8
    }

    /// Get key size from word1 (bits 15-8)
    pub fn key_size(&self) -> u8 {
        ((self.word1 >> 8) & 0xFF) as u8
    }

    /// Get hash algorithm from word1 (bits 7-0)
    pub fn hash_alg(&self) -> u8 {
        (self.word1 & 0xFF) as u8
    }

    // Check if this is an ECDSA key
    pub fn is_ecdsa(&self) -> bool {
        self.sign_alg() == 0x27
    }

    // Check if this is an ML-DSA key
    pub fn is_mldsa(&self) -> bool {
        self.sign_alg() == 0xD2
    }

    // Check if this is a SEC384R1 key
    pub fn is_sec384r1(&self) -> bool {
        self.key_size() == 0x2
    }

    // Check if this is an MLDSA87 key
    pub fn is_mldsa87(&self) -> bool {
        self.key_size() == 0xA
    }

    // Check if CA flags are set
    pub fn is_ca(&self) -> bool {
        self.srk_flags() & 0x80 != 0
    }

    // Check if hash algorithm is SHA2_384
    pub fn is_sha384(&self) -> bool {
        self.hash_alg() == 0x1
    }

    // Check if hash algorithm is SHA2_512
    pub fn is_sha512(&self) -> bool {
        self.hash_alg() == 0x2
    }

    // Get X parameter size for ECDSA keys (bits 15-0)
    pub fn ecdsa_x_size(&self) -> u16 {
        (self.param_lens & 0xFFFF) as u16
    }

    // Get Y parameter size for ECDSA keys (bits 31-16)
    pub fn ecdsa_y_size(&self) -> u16 {
        (self.param_lens >> 16) as u16
    }

    // Get raw key size for ML-DSA keys (bits 15-0)
    pub fn mldsa_key_size(&self) -> u16 {
        (self.param_lens & 0xFFFF) as u16
    }
}

#[repr(C)]
pub struct AhabSrkDataHeaderRaw {
    // Word 0: Tag (31-24), Length (23-8), Version (7-0)
    pub word0: u32, // Tag(31-24) | Length(23-8) | Version(7-0)
    // Word 1: Reserved (31-8), SRK record # (7-0)
    pub word1: u32, // Reserved(31-8) | SRK record index(7-0)
                    // Word 2+: Key data (format is type dependent)
                    // ECDSA: X (big endian), Y (big endian) - parameter size aligned, padded with leading zeros
                    // ML-DSA: Raw key (big endian) - parameter size aligned, padded with leading zeros
}

impl AhabSrkDataHeaderRaw {
    /// Get tag from word0 (bits 31-24)
    pub fn tag(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get version from word0 (bits 7-0)
    pub fn version(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get record index from word1 (bits 7-0)
    pub fn record_index(&self) -> u8 {
        (self.word1 & 0xFF) as u8
    }

    /// Get the total size of SRK data including header
    pub fn total_size(&self) -> u16 {
        self.length()
    }

    /// Get the size of key data (excluding 8-byte header)
    pub fn key_data_size(&self) -> u16 {
        self.length().saturating_sub(8)
    }

    /// Get the SRK record number this data is associated with
    pub fn srk_record_number(&self) -> u8 {
        self.record_index()
    }

    /// Check if this SRK data is for ECDSA (expects 96 bytes: 48 X + 48 Y)
    pub fn is_ecdsa_size(&self) -> bool {
        self.key_data_size() == 96
    }

    /// Check if this SRK data is for ML-DSA-87 (expects 2592 bytes)
    pub fn is_mldsa87_size(&self) -> bool {
        self.key_data_size() == 2592
    }
}

#[repr(C)]
pub struct AhabSignatureHeaderRaw {
    // Word 0: Tag (31-24), Length (23-8), Version (7-0)
    pub word0: u32, // Tag(31-24) | Length(23-8) | Version(7-0)
    // Word 1: Reserved
    pub reserved: u32, // Reserved
                       // Word 2+: Signature data (format is type dependent)
                       // ECDSA: r and s components (curve size aligned, padded with zeros)
                       // ML-DSA: Raw signature
                       // Signatures are in the same order as associated SRK tables
}

impl AhabSignatureHeaderRaw {
    /// Get tag from word0 (bits 31-24)
    pub fn tag(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get version from word0 (bits 7-0)
    pub fn version(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get the total size of signature block including header
    pub fn total_size(&self) -> u16 {
        self.length()
    }

    /// Get the size of signature data (excluding 8-byte header)
    pub fn signature_data_size(&self) -> u16 {
        self.length().saturating_sub(8)
    }

    /// Check if this signature data is for ECDSA P-384 (expects 96 bytes: 48 r + 48 s)
    pub fn is_ecdsa_size(&self) -> bool {
        self.signature_data_size() == 96
    }

    /// Check if this signature data is for ML-DSA-87
    pub fn is_mldsa_size(&self) -> bool {
        // ML-DSA-87 signatures are variable length but typically around 4627 bytes
        let size = self.signature_data_size();
        size >= 4000 && size <= 5000 // Reasonable range for ML-DSA-87, search says ~4564 bytes.
    }
}

// Certificate format per Tables 168-169
#[repr(C)]
pub struct AhabCertificateHeaderRaw {
    // Word 0: Tag (31-24), Length (23-8), Version (7-0)
    pub word0: u32, // Tag(31-24) | Length(23-8) | Version(7-0)
    // Word 1: Perm (31-16), Signature offset (15-0)
    pub word1: u32, // Permissions(31-16) | Signature offset(15-0)
    // Word 2-4: Permission data (96 bits = 12 bytes)
    pub perm_data: [u8; 12], // 96 bits of complementary information for debug auth
    // Word 5: Fuse version (8 bits, position TODO) + reserved (24 bits)
    pub fuse_version_word: u32, // Word 5: fuse version field (bit layout TBD)
    // Word 6-9: UUID (128 bits = 16 bytes)
    pub uuid: [u8; 16], // unique ID of targeted device
}

impl AhabCertificateHeaderRaw {
    /// Get tag from word0 (bits 31-24)
    pub fn tag(&self) -> u8 {
        (self.word0 >> 24) as u8
    }

    /// Get length from word0 (bits 23-8)
    pub fn length(&self) -> u16 {
        ((self.word0 >> 8) & 0xFFFF) as u16
    }

    /// Get version from word0 (bits 7-0)
    pub fn version(&self) -> u8 {
        (self.word0 & 0xFF) as u8
    }

    /// Get permissions from word1 (bits 31-16)
    pub fn permissions(&self) -> u16 {
        (self.word1 >> 16) as u16
    }

    /// Get signature offset from word1 (bits 15-0)
    pub fn signature_offset(&self) -> u32 {
        (self.word1 & 0xFFFF) as u32
    }

    /// Check if this is a valid certificate (tag 0xAF, version 0x02)
    pub fn is_valid(&self) -> bool {
        self.tag() == 0xAF && self.version() == 0x02
    }

    /// Get permission data as slice  
    pub fn perm_data_slice(&self) -> &[u8] {
        &self.perm_data
    }

    /// Check if permissions are valid (no validation needed without perm_inv)
    pub fn permissions_valid(&self) -> bool {
        true // No perm_inv field to validate against
    }

    /// Get fuse version (bit layout TODO - assuming lower 8 bits for now)
    pub fn fuse_version(&self) -> u8 {
        (self.fuse_version_word & 0xFF) as u8
    }
}

fn is_aligned_4(ptr: *const u8) -> bool {
    (ptr as usize) % 4 == 0
}

#[inline(always)]
fn checked_end(start: usize, len: usize) -> Result<usize, CertError> {
    start.checked_add(len).ok_or(CertError::Bounds)
}

pub unsafe fn parse_ahab_container(
    base: *const u8,
    container_offset: u32,
    image_len: u32,
) -> Result<AhabParsed, CertError> {
    if container_offset >= image_len {
        return Err(CertError::Bounds);
    }
    let start = base.add(container_offset as usize);
    let ch = start as *const AhabContainerHeaderRaw;

    if (*ch).tag() != 0x87 {
        return Err(CertError::Tag);
    }

    if (*ch).version() != 0x02 {
        return Err(CertError::Version);
    }
    let total = (*ch).length() as usize;

    let container_end = checked_end(container_offset as usize, total)?;
    if total == 0 || container_end > image_len as usize {
        return Err(CertError::Bounds);
    }

    if !is_aligned_4(start) {
        return Err(CertError::Align);
    }

    // Image array begins after header; calculate length from sigblk_offset
    let image_array_start = start.add(size_of::<AhabContainerHeaderRaw>());
    let _image_entry_size = size_of::<AhabImageEntryRaw>();
    let sigblk_offset = (*ch).signature_block_offset() as usize;
    if sigblk_offset < size_of::<AhabContainerHeaderRaw>() {
        return Err(CertError::Bounds);
    }
    if checked_end(sigblk_offset, size_of::<AhabSignatureBlockRaw>())? > total {
        return Err(CertError::Bounds);
    }
    let _image_array_size = sigblk_offset - size_of::<AhabContainerHeaderRaw>();
    let images_len = (*ch).image_count() as usize;

    // Store image array pointer and count
    let images_ptr = image_array_start as *const AhabImageEntryRaw;

    // Signature block
    let sigblk_ptr = start.add(sigblk_offset);

    if !is_aligned_4(sigblk_ptr) {
        return Err(CertError::Align);
    }
    let sigblk = sigblk_ptr as *const AhabSignatureBlockRaw;

    if (*sigblk).tag() != 0x90 || (*sigblk).version() != 0x01 {
        return Err(CertError::Tag);
    }

    // SRK array raw slice
    let srk_array_offset = (*sigblk).srk_array_offset() as usize;
    if checked_end(sigblk_offset, srk_array_offset)? > total {
        return Err(CertError::Bounds);
    }
    if checked_end(sigblk_offset + srk_array_offset, size_of::<AhabSrkArrayHeaderRaw>())? > total {
        return Err(CertError::Bounds);
    }
    let srk_array_ptr = sigblk_ptr.add(srk_array_offset);

    if !is_aligned_4(srk_array_ptr) {
        return Err(CertError::Align);
    }
    // Method 1: Use SRK array header's own length field
    let srk_hdr = srk_array_ptr as *const AhabSrkArrayHeaderRaw;
    let srk_header_len = (*srk_hdr).length() as usize;

    // Method 2: Calculate from offsets (signature_offset - srk_array_offset)
    let sig_offset = (*sigblk).signature_offset() as usize;
    if checked_end(sigblk_offset, sig_offset)? > total {
        return Err(CertError::Bounds);
    }
    let srk_offset_len = sig_offset.saturating_sub(srk_array_offset);

    // Use the smaller of the two for safety (prevent buffer overrun)
    let srk_array_len = core::cmp::min(srk_header_len, srk_offset_len);

    // Certificate raw slice
    //Note that certificate may be absent (SRK-only mode), it is optional.
    let (cert_ptr, cert_len) = if (*sigblk).cert_offset() == 0 {
        // SRK-only mode: no certificate present
        (core::ptr::null(), 0)
    } else {
        let cert_offset = (*sigblk).cert_offset() as usize;
        if checked_end(sigblk_offset, cert_offset)? > total {
            return Err(CertError::Bounds);
        }
        if checked_end(sigblk_offset + cert_offset, size_of::<AhabCertificateHeaderRaw>())? > total {
            return Err(CertError::Bounds);
        }
        let cert_ptr = sigblk_ptr.add(cert_offset);
        if !is_aligned_4(cert_ptr) {
            return Err(CertError::Align);
        }
        // Read certificate header to get its total length
        let cert_hdr = cert_ptr as *const AhabCertificateHeaderRaw;

        if (*cert_hdr).tag() != 0xAF || (*cert_hdr).version() != 0x02 {
            return Err(CertError::Tag);
        }
        let cert_len = (*cert_hdr).length() as usize;
        if checked_end(sigblk_offset + cert_offset, cert_len)? > total {
            return Err(CertError::Bounds);
        }
        (cert_ptr, cert_len)
    };
    // Signatures raw slice
    if checked_end(sigblk_offset + sig_offset, size_of::<AhabSignatureHeaderRaw>())? > total {
        return Err(CertError::Bounds);
    }
    let sig_ptr = sigblk_ptr.add(sig_offset);
    if !is_aligned_4(sig_ptr) {
        return Err(CertError::Align);
    }
    // Read signature header to measure length
    let sig_hdr = sig_ptr as *const AhabSignatureHeaderRaw;

    if (*sig_hdr).tag() != 0xD8 || (*sig_hdr).version() != 0x00 {
        return Err(CertError::Tag);
    }
    let sig_len = (*sig_hdr).length() as usize;
    if checked_end(sigblk_offset + sig_offset, sig_len)? > total {
        return Err(CertError::Bounds);
    }
    cert_trace!(
        "Parsed AHAB container: images={}, srk_array_len={}, cert_len={}, sig_len={}",
        images_len,
        srk_array_len,
        cert_len,
        sig_len
    );
    Ok(AhabParsed {
        container: ch,
        images: images_ptr,
        images_count: images_len,
        sigblk,
        srk_array_ptr,
        srk_array_len,
        cert_ptr,
        cert_len,
        sig_ptr,
        sig_len,
    })
}

// Deeper parsers: SRK array, certificate internals, signatures

// Parse SRK array: expects header followed by arrays of table offsets and data offsets.
pub unsafe fn parse_srk_array<'a>(
    srk_array_ptr: *const u8,
    srk_array_len: usize,
) -> Result<ParsedSrkArray<'a>, CertError> {
    let base = srk_array_ptr;
    let hdr = &*(base as *const AhabSrkArrayHeaderRaw);

    if srk_array_len < size_of::<AhabSrkArrayHeaderRaw>() + 16 {
        return Err(CertError::Bounds);
    }
    if hdr.tag() != 0x5A || hdr.version() != 0x00 {
        return Err(CertError::Tag);
    }

    let count = hdr.srk_table_count() as usize;
    if count != 2 {
        return Err(CertError::Bounds);
    } // Only hybrid mode supported (ECDSA + ML-DSA)

    // Sequential layout: ECDSA table starts immediately after header
    let ecdsa_tbl_ptr = base.add(size_of::<AhabSrkArrayHeaderRaw>());
    let ecdsa_tbl_hdr = &*(ecdsa_tbl_ptr as *const AhabSrkTableHeaderRaw);

    if ecdsa_tbl_hdr.tag() != 0xD7 || ecdsa_tbl_hdr.version() != 0x43 {
        return Err(CertError::Tag);
    }
    let ecdsa_rec_count = ecdsa_tbl_hdr.record_count();
    let ecdsa_rec_base = ecdsa_tbl_ptr.add(size_of::<AhabSrkTableHeaderRaw>()) as *const AhabSrkRecordRaw;

    // Check if ECDSA table fits within bounds
    let ecdsa_tbl_offset = size_of::<AhabSrkArrayHeaderRaw>();
    let ecdsa_tbl_total_size = checked_end(
        size_of::<AhabSrkTableHeaderRaw>(),
        ecdsa_rec_count
            .checked_mul(size_of::<AhabSrkRecordRaw>())
            .ok_or(CertError::Bounds)?,
    )?;

    if checked_end(ecdsa_tbl_offset, ecdsa_tbl_total_size)? > srk_array_len {
        return Err(CertError::Bounds);
    }

    let ecdsa_records = core::slice::from_raw_parts(ecdsa_rec_base, ecdsa_rec_count);
    let ecdsa_raw_table = core::slice::from_raw_parts(ecdsa_tbl_ptr, ecdsa_tbl_total_size);

    // Parse ML-DSA table (table 1) - starts after ECDSA table + ECDSA data
    // Find ECDSA data size first
    let ecdsa_data_ptr = ecdsa_tbl_ptr.add(ecdsa_tbl_total_size);
    let ecdsa_data_offset = checked_end(ecdsa_tbl_offset, ecdsa_tbl_total_size)?;
    if checked_end(ecdsa_data_offset, size_of::<AhabSrkDataHeaderRaw>())? > srk_array_len {
        return Err(CertError::Bounds);
    }
    let ecdsa_data_hdr = &*(ecdsa_data_ptr as *const AhabSrkDataHeaderRaw);
    let ecdsa_data_total_size = ecdsa_data_hdr.length() as usize;
    if ecdsa_data_total_size < size_of::<AhabSrkDataHeaderRaw>() {
        return Err(CertError::Bounds);
    }
    if checked_end(ecdsa_data_offset, ecdsa_data_total_size)? > srk_array_len {
        return Err(CertError::Bounds);
    }

    let mldsa_tbl_ptr = ecdsa_data_ptr.add(ecdsa_data_total_size);
    let mldsa_tbl_offset = checked_end(ecdsa_data_offset, ecdsa_data_total_size)?;
    if checked_end(mldsa_tbl_offset, size_of::<AhabSrkTableHeaderRaw>())? > srk_array_len {
        return Err(CertError::Bounds);
    }
    let mldsa_tbl_hdr = &*(mldsa_tbl_ptr as *const AhabSrkTableHeaderRaw);

    if mldsa_tbl_hdr.tag() != 0xD7 || mldsa_tbl_hdr.version() != 0x43 {
        return Err(CertError::Tag);
    }

    let mldsa_rec_count = mldsa_tbl_hdr.record_count();
    let mldsa_rec_base = mldsa_tbl_ptr.add(size_of::<AhabSrkTableHeaderRaw>()) as *const AhabSrkRecordRaw;

    // Check if ML-DSA table fits within bounds
    let mldsa_tbl_total_size = checked_end(
        size_of::<AhabSrkTableHeaderRaw>(),
        mldsa_rec_count
            .checked_mul(size_of::<AhabSrkRecordRaw>())
            .ok_or(CertError::Bounds)?,
    )?;

    if checked_end(mldsa_tbl_offset, mldsa_tbl_total_size)? > srk_array_len {
        return Err(CertError::Bounds);
    }

    let mldsa_records = core::slice::from_raw_parts(mldsa_rec_base, mldsa_rec_count);
    let mldsa_raw_table = core::slice::from_raw_parts(mldsa_tbl_ptr, mldsa_tbl_total_size);
    cert_trace!(
        "Parsed SRK array: ECDSA records={}, ML-DSA records={}",
        ecdsa_rec_count,
        mldsa_rec_count
    );
    Ok(ParsedSrkArray {
        hdr,
        ecdsa_table: ParsedSrkTable {
            hdr: ecdsa_tbl_hdr,
            records: ecdsa_records,
            raw_table_bytes: ecdsa_raw_table,
        },
        mldsa_table: ParsedSrkTable {
            hdr: mldsa_tbl_hdr,
            records: mldsa_records,
            raw_table_bytes: mldsa_raw_table,
        },
    })
}

pub fn derive_image_rkth_pair<'d>(
    mut peri: Peri<'d, peripherals::SGI0>,
    image_base: *const u8,
    container_offset: u32,
    image_len: u32,
) -> (Option<Rkth>, Option<Rkth>) {
    // Parse AHAB container once and extract both ECDSA and ML-DSA RKTH values
    unsafe {
        if let Ok(ahab) = parse_ahab_container(image_base, container_offset, image_len) {
            if let Ok(srk_parsed) = parse_srk_array(ahab.srk_array_ptr, ahab.srk_array_len) {
                // Derive ECDSA RKTH from complete ECDSA table (header + records)
                let ecdsa_rkth = {
                    let table = &srk_parsed.ecdsa_table;

                    // Use the complete SRK table for RKTH calculation
                    let table_bytes = table.raw_table_bytes;
                    match sha512_rkth_48(peri.reborrow(), table_bytes) {
                        //TODO: Verify if SHA-384 is correct here, SRM vs. SPSDK mismatch
                        Some(digest) => Some(Rkth(digest)),
                        None => {
                            cert_error!("SHA-512 unavailable for ECDSA RKTH");
                            None
                        }
                    }
                };

                // Derive ML-DSA RKTH from complete ML-DSA table (header + records)
                let mldsa_rkth = {
                    let table = &srk_parsed.mldsa_table;

                    // Use the complete SRK table for RKTH calculation
                    let table_bytes = table.raw_table_bytes;

                    match sha512_rkth_48(peri.reborrow(), table_bytes) {
                        Some(digest) => Some(Rkth(digest)),
                        None => {
                            cert_error!("SHA-512 unavailable for PQC RKTH");
                            None
                        }
                    }
                };
                cert_trace!("Derived both ECDSA and ML-DSA RKTH values");
                return (ecdsa_rkth, mldsa_rkth);
            } else {
                cert_error!("Failed to parse SRK array");
            }
        } else {
            cert_error!("Failed to parse AHAB container");
        }
    }
    (None, None)
}
