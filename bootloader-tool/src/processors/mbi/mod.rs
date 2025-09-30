#![allow(clippy::len_without_is_empty)]

pub mod cert_block;

use sha2::Digest;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

use crate::processors::certificates::Rkth;
use crate::processors::mbi::cert_block::{CertBlock, CertBlockConfig};
use crate::processors::otp::Otp;
use anyhow::{Context, anyhow, bail};
use hmac::{Hmac, Mac};
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::{Signature, SigningKey};
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{SignatureEncoding, SignerMut, Verifier};
use sha2::Sha256;
use x509_parser::asn1_rs::FromDer;
use x509_parser::certificate::X509Certificate;
use x509_parser::oid_registry::Oid;

type HmacSha256 = Hmac<Sha256>;

/// Image header, this is a vector table with some fields modified
#[derive(Debug, Clone)]
pub struct ImageHeader {
    ivt: Vec<u8>,
}

impl ImageHeader {
    /// Take a vector table and merge in the header information
    pub fn new(mut ivt: Vec<u8>, image_type: ImageType, header_offset: u32, load_addr: u32) -> Self {
        assert_eq!(ivt.len(), 0x40);

        ivt[0x24..0x28].copy_from_slice(&u32::to_le_bytes(image_type.as_u32()));
        ivt[0x28..0x2C].copy_from_slice(&u32::to_le_bytes(header_offset));
        ivt[0x34..0x38].copy_from_slice(&u32::to_le_bytes(load_addr));

        Self { ivt }
    }

    /// Set the image length, this includes the header, data and cert block
    pub fn set_image_length(&mut self, image_length: usize) {
        let image_length = image_length.try_into().unwrap();
        self.ivt[0x20..0x24].copy_from_slice(&u32::to_le_bytes(image_length));
    }

    /// Get the raw data
    pub fn raw(&self) -> &[u8] {
        &self.ivt
    }

    /// Get the [ImageKind]
    pub fn image_kind(&self) -> ImageKind {
        ImageKind::from_u8(self.ivt[0x24]).expect("image kind was set in new")
    }
}

/// Bitset for the image type header field
#[derive(Debug, Copy, Clone)]
pub struct ImageType {
    pub key_store_included: bool,
    pub tz_m_image_type: TrustZone,
    pub tz_m_preset: TrustZonePreset,
    pub enable_hw_user_mode_keys: bool,
    pub image_kind: ImageKind,
}

impl ImageType {
    pub fn new(image_kind: ImageKind) -> Self {
        Self {
            key_store_included: false,
            tz_m_image_type: TrustZone::Enabled,
            tz_m_preset: TrustZonePreset::NotIncluded,
            enable_hw_user_mode_keys: false,
            image_kind,
        }
    }

    fn as_u32(&self) -> u32 {
        let mut out = 0;
        let Self {
            key_store_included,
            tz_m_image_type,
            tz_m_preset,
            enable_hw_user_mode_keys,
            image_kind: image_type,
        } = *self;

        out |= (key_store_included as u32 & 1) << 15;
        out |= (tz_m_image_type as u32 & 1) << 14;
        out |= (tz_m_preset as u32 & 1) << 13;
        out |= (enable_hw_user_mode_keys as u32 & 1) << 12;
        out |= image_type as u32;

        out
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ImageKind {
    Plain = 0,
    /// Note: This uses the Encrypted image layout, but omits the Enc. Image Header?
    PlainSigned = 1,
    PlainWithCrc = 2,
    EncryptedSigned = 3,
    XipPlainSigned = 4,
    XipPlainWithCrc = 5,
}

impl ImageKind {
    fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Plain,
            1 => Self::PlainSigned,
            2 => Self::PlainWithCrc,
            3 => Self::EncryptedSigned,
            4 => Self::XipPlainSigned,
            5 => Self::XipPlainWithCrc,
            _ => return None,
        })
    }
}

impl ImageKind {
    pub fn has_hmac(&self) -> bool {
        match self {
            ImageKind::Plain | ImageKind::PlainWithCrc | ImageKind::XipPlainSigned | ImageKind::XipPlainWithCrc => {
                false
            }
            ImageKind::PlainSigned | ImageKind::EncryptedSigned => true,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum TrustZone {
    Enabled = 0,
    Disabled = 1,
}

#[derive(Debug, Copy, Clone)]
pub enum TrustZonePreset {
    NotIncluded = 0,
    Included = 1,
}

fn parse_x509_cert(bytes: &[u8]) -> anyhow::Result<X509Certificate<'_>> {
    let (rem, cert) = X509Certificate::from_der(bytes)?;
    if rem.len() >= 4 {
        bail!(
            "parsed certificate has too much padding: {} expected at most 3",
            rem.len()
        );
    }

    const ALGO: Oid<'static> = x509_parser::oid_registry::OID_PKCS1_SHA256WITHRSA;
    if cert.signature_algorithm.algorithm != ALGO {
        bail!(
            "Wrong signature algorithm, got: {:?}, expected {:?}",
            cert.signature_algorithm,
            ALGO
        );
    }

    Ok(cert)
}

/// All data that is contained in an MBI
#[derive(Debug, Clone)]
pub struct Image {
    header: ImageHeader,
    data: Vec<u8>,
    cert_block: CertBlock,
}

impl Image {
    const DATA_ALIGN: usize = 4;

    pub fn new(image: Vec<u8>, base_addr: u32, image_type: ImageType, mut cert_block: CertBlock) -> Self {
        let (ivt, data) = image.split_at(0x40);

        // Make sure we do not use the original image without padding by accident
        let ivt = ivt.to_vec();
        let mut data = data.to_vec();
        drop(image);

        // Pad out data so the cert block is aligned correctly
        while data.len() % Self::DATA_ALIGN != 0 {
            data.push(0);
        }

        let image_len = ivt.len() + data.len();

        let mut header = ImageHeader::new(ivt, image_type, image_len as u32, base_addr);

        let signed_len = image_len + cert_block.raw().len();
        cert_block.set_total_image_length_in_bytes(signed_len);

        let hmac_len = if image_type.image_kind.has_hmac() {
            Sha256::output_size()
        } else {
            0
        };
        header.set_image_length(signed_len + hmac_len + cert_block.signature_len());

        Self {
            header,
            data,
            cert_block,
        }
    }

    fn hmac(&self, otp: Otp) -> anyhow::Result<Vec<u8>> {
        let hmac_key = otp.hmac_key()?;

        let mut mac = HmacSha256::new_from_slice(&hmac_key.0)?;

        mac.update(self.header.raw());
        let result = mac.finalize();

        let hmac = &result.into_bytes()[..];

        Ok(Vec::from(hmac))
    }

    /// Get the binary as it should be signed
    ///
    /// This does not include the HMAC of the header since that is not included in the signature.
    pub fn sign_me(&self) -> Vec<u8> {
        let mut out = vec![];
        out.extend(self.header.raw());
        out.extend(&self.data);
        out.extend(self.cert_block.raw());

        out
    }

    /// Check if the signature matches this image and RKTH
    pub fn check(&self, signature: &[u8], rkth: &Rkth) -> anyhow::Result<()> {
        log::info!("Checking if signature matches image");

        if signature.len() != self.cert_block.signature_len() {
            bail!(
                "signature length mismatch, expected {} got {}",
                self.cert_block.signature_len(),
                signature.len()
            );
        }

        if &self.cert_block.rkth() != rkth {
            bail!(
                "CertBlock RKTH does not match expected. CertBlock: {:x?}, Expected: {rkth:x?}",
                self.cert_block.rkth()
            );
        }

        let signature = Signature::try_from(signature)?;
        self.cert_block
            .verifying_key()
            .verify(&self.sign_me(), &signature)
            .context("Could not verify signature with the current image")?;

        log::info!("OK - Signature matches image");

        Ok(())
    }

    /// Combine the signature with this image
    ///
    /// This also inserts the HMAC of the header if it is needed for this image type.
    pub fn merge(&self, signature: &[u8], otp: Option<Otp>) -> anyhow::Result<Vec<u8>> {
        // Ensure the signature matches what we have
        self.check(signature, &self.cert_block.rkth())?;

        let mut out = vec![];
        out.extend(self.header.raw());

        if self.header.image_kind().has_hmac() {
            let otp = otp.ok_or_else(|| anyhow::anyhow!("Expected OTP to be passed for signing the bootloader"))?;
            out.extend(self.hmac(otp)?)
        }

        out.extend(&self.data);
        out.extend(self.cert_block.raw());
        out.extend(signature);

        Ok(out)
    }
}

fn load_image(
    input_path: &impl AsRef<Path>,
    base_addr: u32,
    is_bootloader: bool,
    cert_block: CertBlock,
) -> anyhow::Result<Image> {
    let plain_image = std::fs::read(input_path)?;

    let image_type = if is_bootloader {
        // Note: ROM bootloader only accepts xip plain signed images when secure_boot_en bit is unset.
        ImageKind::XipPlainSigned
    } else {
        // Note: ec-slimloader loads the application to RAM, but skboot_authenticate requires the image to be marked for XIP.
        ImageKind::XipPlainSigned
    };

    let image = Image::new(plain_image, base_addr, ImageType::new(image_type), cert_block);
    Ok(image)
}

/// Produce an image that can be signed by an external source
pub fn prepare_to_sign(
    input_path: impl AsRef<Path>,
    base_addr: u32,
    output_path: impl AsRef<Path>,
    is_bootloader: bool,
    cert_block: CertBlock,
) -> anyhow::Result<()> {
    let image = load_image(&input_path, base_addr, is_bootloader, cert_block)?;
    let signing_body = image.sign_me();

    std::fs::write(output_path, &signing_body).context("Could not write prepared image")?;

    Ok(())
}

/// Merge an image with a signature
pub fn merge_with_signature(
    input_path: impl AsRef<Path>,
    base_addr: u32,
    signature_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    is_bootloader: bool,
    otp: Option<Otp>,
    cert_block: CertBlock,
) -> anyhow::Result<()> {
    let image = load_image(&input_path, base_addr, is_bootloader, cert_block)?;
    let signature = std::fs::read(&signature_path)?;

    std::fs::write(output_path.as_ref(), image.merge(&signature, otp)?)?;

    Ok(())
}

/// Sign the image (useful for testing without HSM)
pub fn sign(
    signature_path: impl AsRef<Path>,
    prepared_path: impl AsRef<Path>,
    private_key_path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let private_key = std::fs::read_to_string(private_key_path)?;
    let private_key = RsaPrivateKey::from_pkcs8_pem(&private_key)?;
    let mut signing_key = SigningKey::<Sha256>::new(private_key);

    let signing_body = std::fs::read(&prepared_path)?;
    let signature = signing_key.sign(&signing_body);

    std::fs::write(&signature_path, signature.to_bytes()).context("Could not write signature")?;
    Ok(())
}

/// Generate a MBI using our pure Rust implementation.
pub fn generate_pure(
    input_path: impl AsRef<Path>,
    base_addr: u32,
    output_path: impl AsRef<Path>,
    is_bootloader: bool,
    otp: Option<Otp>,
    cert_block: CertBlock,
    private_key_path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let mut prepared_path = output_path.as_ref().to_path_buf();
    prepared_path.set_extension(".prepared.bin");
    prepare_to_sign(
        &input_path,
        base_addr,
        &prepared_path,
        is_bootloader,
        cert_block.clone(),
    )?;

    let mut signature_path = output_path.as_ref().to_path_buf();
    signature_path.set_extension(".signature.bin");
    sign(&signature_path, &prepared_path, private_key_path)?;

    merge_with_signature(
        input_path,
        base_addr,
        signature_path,
        output_path,
        is_bootloader,
        otp,
        cert_block,
    )?;

    Ok(())
}

/// Generate a MBI using the original NXP SPSDK tooling.
pub fn generate_nxp(
    nxpimage: impl AsRef<Path>,
    input_path: impl AsRef<Path>,
    base_addr: u32,
    output_path: impl AsRef<Path>,
    is_bootloader: bool,
    cert_block: CertBlockConfig,
) -> anyhow::Result<()> {
    let mut config: BTreeMap<String, String> = BTreeMap::default();

    let mut cert_block_file = NamedTempFile::new()?;
    serde_yml::to_writer(&mut cert_block_file, &cert_block)?;

    config.insert(
        "certBlock".to_owned(),
        cert_block_file
            .path()
            .to_str()
            .ok_or_else(|| anyhow!("Path not a string"))?
            .to_owned(),
    );

    config.insert("outputImageExecutionAddress".to_owned(), format!("{base_addr:#x}"));

    config.insert(
        "inputImageFile".to_owned(),
        input_path
            .as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("Path not a string"))?
            .into(),
    );

    let output_path_abs = std::env::current_dir()?.join(output_path.as_ref());
    config.insert(
        "masterBootOutputFile".to_owned(),
        output_path_abs
            .to_str()
            .ok_or_else(|| anyhow!("Path not a string"))?
            .into(),
    );

    log::debug!("Config: {config:#?}");

    let mut command = Command::new(nxpimage.as_ref());
    command.current_dir("./artifacts");

    let mbi_conf_path = if is_bootloader {
        "mbi-bootloader.yaml"
    } else {
        "mbi-application.yaml"
    };

    command.args(["mbi", "export", "-c", mbi_conf_path]);

    for (k, v) in config {
        command.args(["-oc", &format!("{k}={v}")]);
    }

    eprintln!("{:?}", command);

    let output = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("Could not execute command {command:?}"))?;

    if !output.status.success() {
        return Err(
            anyhow::anyhow!(format!("Failed to build MBI image from {}", mbi_conf_path))
                .context(String::from_utf8(output.stdout)?),
        );
    }

    let input = std::fs::read(&input_path)?;
    let output = std::fs::read(&output_path)?;
    let diff_len = output.len() - input.len();

    log::debug!("Output len: 0x{:x}", output.len());
    log::debug!("Added len: 0x{diff_len:x}");

    if diff_len > 0xC47 {
        return Err(anyhow::anyhow!(
            "Added more than expected to output image when signing: {diff_len}"
        ));
    }

    // Performing checks on output image
    #[allow(clippy::if_same_then_else)]
    let expected_image_type = if is_bootloader { 0x0004u32 } else { 0x0004u32 };

    let image_type = u32::from_le_bytes((&output[0x24..0x28]).try_into().unwrap());
    if image_type != expected_image_type {
        return Err(anyhow::anyhow!(
            "Failed to generate expected image type 0x{:x}, got 0x{:x}",
            expected_image_type,
            image_type
        ));
    }
    log::debug!("Got expected image type 0x{expected_image_type:x}");

    // TODO more checks

    Ok(())
}
