use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Context;
use rsa::{RsaPublicKey, pkcs1v15::VerifyingKey, pkcs8::DecodePublicKey, traits::PublicKeyParts};
use serde::Serialize;
use sha2::{Digest, Sha256};
use x509_parser::public_key::PublicKey;

use crate::{
    Config,
    processors::{certificates::Rkth, mbi::parse_x509_cert},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertBlockConfig {
    pub family: String,
    pub revision: String,

    #[serde(flatten)]
    pub certificates: BTreeMap<String, PathBuf>,

    pub main_root_cert_id: usize,
    pub container_output_file: Option<PathBuf>,
}

/// Best-effort attempt to canonicalize the path, if it exists.
fn canonicalize_or_leave(path: impl AsRef<Path>) -> PathBuf {
    match path.as_ref().canonicalize() {
        Ok(path) => path,
        Err(_) => path.as_ref().to_owned(),
    }
}

pub fn generate_config(
    config: &Config,
    certificate_idx: usize,
    output_file: Option<impl AsRef<Path>>,
) -> CertBlockConfig {
    let mut certificates = BTreeMap::default();
    for (chain_i, chain) in config.certificates.iter().enumerate() {
        for (cert_i, cert) in chain.0.iter().enumerate() {
            let name = if cert_i == 0 {
                format!("rootCertificate{}File", chain_i)
            } else {
                format!("chainCertificate{}File{}", chain_i, cert_i - 1)
            };
            certificates.insert(name, canonicalize_or_leave(&cert.path));
        }
    }

    CertBlockConfig {
        family: "mimxrt685s".to_owned(),
        revision: "latest".to_owned(),
        certificates,
        main_root_cert_id: certificate_idx,
        container_output_file: output_file.map(|output_file| output_file.as_ref().to_owned()),
    }
}

pub fn generate(nxpimage: impl AsRef<Path>, config: &Config, certificate_idx: usize) -> anyhow::Result<CertBlock> {
    // nxpimage cert-block export -c ./cert-block.yaml

    let mut input_file = tempfile::NamedTempFile::new()?;
    let output_file = tempfile::NamedTempFile::new()?;
    serde_yml::to_writer(
        &mut input_file,
        &generate_config(config, certificate_idx, Some(output_file.path())),
    )?;

    let mut command = Command::new(nxpimage.as_ref());

    command.args(["cert-block", "export", "-c"]);
    command.arg(input_file.path());

    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(move || format!("Could not execute `{}`, is it installed?", nxpimage.as_ref().display()))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(format!(
            "Failed to build certificate block from {}",
            input_file.as_ref().display()
        ))
        .context(String::from_utf8(output.stdout)?));
    }

    let rkth_str = String::from_utf8(output.stdout)?
        .lines()
        .find(|line| line.contains("RKTH"))
        .map(|line| line.trim().trim_start_matches("RKTH: "))
        .ok_or_else(|| anyhow::anyhow!("nxpimage output does not contain RKTH"))?
        .to_owned();
    let rkth = Rkth::from_hex(&rkth_str)?;
    let rkth_str = rkth.as_hex(); // Canonicalize

    log::info!("RKTH: {rkth_str}");

    CertBlock::from_file(output_file.path(), Some(&rkth))
}

/// Cert block as described in UM11147 Fig 239
#[derive(Debug, Clone)]
pub struct CertBlock {
    data: Vec<u8>,
}

impl CertBlock {
    /// Read cert block from file that was generated with `nxpimage cert-block export -c ./cert-block.yaml`
    ///
    /// That file contains padding data, so we read out the lengths from the header in that file to determine the correct length.
    pub fn from_file(filename: impl AsRef<Path>, rkth: Option<&Rkth>) -> anyhow::Result<Self> {
        let data = std::fs::read(filename)?;
        let mut me = Self { data };

        // Strip padding
        let cert_block_len = me.header_len() + me.cert_table_len() + 4 * Sha256::output_size() as u32;
        assert!(me.data.len() >= cert_block_len as usize);
        me.data.truncate(cert_block_len as usize);

        // Ensure the cert block is valid
        me.verify(rkth)?;

        Ok(me)
    }

    /// Get the raw bytes of the cert block
    pub fn raw(&self) -> &[u8] {
        &self.data
    }

    fn cert_count(&self) -> u32 {
        u32::from_le_bytes(self.data[0x18..0x1c].try_into().unwrap())
    }

    fn cert_table_len(&self) -> u32 {
        u32::from_le_bytes(self.data[0x1c..0x20].try_into().unwrap())
    }

    fn header_len(&self) -> u32 {
        let len = u32::from_le_bytes(self.data[0x08..0x0c].try_into().unwrap());

        if len != 0x20 {
            log::warn!("Header length mismatch, expected {:x?}, got {:x?}", 0x20, len);
        }

        len
    }

    /// Sets the total length of signed bytes, this needs to be updated before signing
    pub fn set_total_image_length_in_bytes(&mut self, total_image_length_in_bytes: usize) {
        let total_image_length_in_bytes = total_image_length_in_bytes.try_into().unwrap();
        self.data[0x14..0x18].copy_from_slice(&u32::to_le_bytes(total_image_length_in_bytes));
    }

    fn root_key_hashes(&self) -> [[u8; 256 / 8]; 4] {
        let rkh_start = (self.header_len() + self.cert_table_len()) as usize;
        let data = &self.data[rkh_start..];
        assert_eq!(data.len(), 4 * Sha256::output_size());

        data.chunks_exact(Sha256::output_size())
            .map(|chunk| chunk.try_into().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    /// Calculate the RKTH of the contained root key hashes
    pub fn rkth(&self) -> Rkth {
        let mut hash = Sha256::default();
        for rkh in self.root_key_hashes() {
            hash.update(rkh);
        }
        Rkth(hash.finalize().into())
    }

    fn certificates(&self) -> anyhow::Result<Vec<Vec<u8>>> {
        let mut out = vec![];
        let mut next_cert = self.header_len() as usize;

        for i in 0..self.cert_count() {
            let cert_len = u32::from_le_bytes(self.data[next_cert..next_cert + 4].try_into().unwrap()) as usize;
            if !cert_len.is_multiple_of(4) {
                return Err(anyhow::anyhow!("Certificate of cert {i} length is not divisible by 4"));
            }
            next_cert += 4;
            out.push(self.data[next_cert..next_cert + cert_len].to_vec());
            next_cert += cert_len;
        }

        assert_eq!(next_cert, (self.header_len() + self.cert_table_len()) as usize);

        Ok(out)
    }

    /// Check if this CertBlock is consistent with itself and the Rkth
    fn verify(&self, rkth: Option<&Rkth>) -> anyhow::Result<()> {
        log::info!("Checking certificate block is consistent");

        // Ensure the rkth is correct if the user has one
        match rkth {
            Some(rkth) if &self.rkth() != rkth => {
                return Err(anyhow::anyhow!(
                    "CertBlock RKTH does not match provided Rkth, in block: {:x?}, to check: {rkth:x?}",
                    self.rkth()
                ));
            }
            _ => {}
        }

        // Unpack the certificates
        let certs = self.certificates().context("Could not parse certificate list")?;

        log::info!("Got {} certificates in certificate block", certs.len());

        // Check root cert is in root key hashes
        let Some(raw_root_cert) = certs.first() else {
            return Err(anyhow::anyhow!("Certificate block contains no certificates"));
        };

        // Get the root public key
        let root_cert = parse_x509_cert(raw_root_cert)?;
        let root_public_key = root_cert
            .public_key()
            .parsed()
            .context("could not parse root public key")?;
        let PublicKey::RSA(root_public_key) = root_public_key else {
            return Err(anyhow::anyhow!(
                "Invalid public key type: {root_public_key:?} Must be RSA"
            ));
        };

        // Hash modulus || exponent, but ensure to strip leading zero bytes from the modulus
        let mut rkh = Sha256::default();
        rkh.update(
            root_public_key
                .modulus
                .strip_prefix(&[0])
                .unwrap_or(root_public_key.modulus),
        );
        rkh.update(root_public_key.exponent);
        let rkh: [u8; 32] = rkh.finalize().into();

        // Walk the root key hashes to find the right slot
        match self.root_key_hashes().iter().position(|&slot| slot == rkh) {
            Some(slot) => log::info!("Found key hash in RKT slot {slot}"),
            None => {
                return Err(anyhow::anyhow!(
                    "Root cert is not in root key hashes! Cert hash: {rkh:x?}, Hash table: {:x?}",
                    self.root_key_hashes()
                ));
            }
        }

        // Check root_cert is a CA cert
        if !root_cert.is_ca() {
            return Err(anyhow::anyhow!("Root cert is not marked as CA"));
        }

        // Walk the certificate chain
        let mut signing_cert = root_cert;
        for (i, cert) in certs.iter().skip(1).enumerate() {
            // Check that all intermediate certificates are CA certs
            if !signing_cert.is_ca() {
                return Err(anyhow::anyhow!("Root cert is not marked as CA"));
            }

            let cert = parse_x509_cert(cert)?;
            cert.verify_signature(Some(signing_cert.public_key()))
                .with_context(|| format!("Could not verify cert number {i}"))?;

            signing_cert = cert;
        }

        // Check that the last cert is NOT a CA cert, except for if it is the root cert
        if signing_cert.is_ca() && certs.len() > 1 {
            return Err(anyhow::anyhow!("Signing cert is marked as CA cert"));
        }

        // Check that the last public key can be parsed
        RsaPublicKey::from_public_key_der(signing_cert.public_key().raw)
            .context("Could not parse image signing key")?;

        Ok(())
    }

    /// Get the public key of the certificates that is used to sign the image
    pub fn verifying_key(&self) -> VerifyingKey<Sha256> {
        let certs = self.certificates().expect("Ensured in verify");
        let signing_cert = certs.last().expect("Ensured in verify").as_slice();
        let signing_cert = parse_x509_cert(signing_cert).expect("Ensured in verify");

        // Parse the final public key into something the rsa crate can work with
        let image_signing_key = RsaPublicKey::from_public_key_der(signing_cert.public_key().raw)
            .expect("Ensured by verify being called in from file");

        VerifyingKey::new(image_signing_key)
    }

    /// Get the length of the signature for the image
    pub fn signature_len(&self) -> usize {
        let key: RsaPublicKey = self.verifying_key().into();
        key.size()
    }
}
