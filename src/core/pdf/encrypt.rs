use lopdf::encryption::crypt_filters::{Aes128CryptFilter, CryptFilter};
use lopdf::{EncryptionState, EncryptionVersion, Object, Permissions};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::types::{PdfToolError, Result};

pub(crate) fn encrypt_pdf(
    input_path: &Path,
    output_path: &Path,
    user_password: &str,
    owner_password: Option<&str>,
) -> Result<()> {
    let mut doc = lopdf::Document::load(input_path)
        .map_err(|e| PdfToolError::new(format!("Failed to read document: {e}")))?;
    if doc.is_encrypted() {
        return Err(PdfToolError::new(
            "Input file is already encrypted. Decrypt it first before re-encrypting.",
        ));
    }

    if doc.trailer.get(b"ID").is_err() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos();
        let doc_id = format!("multus-{}-{nonce}", std::process::id());
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal(doc_id.clone().into_bytes()),
                Object::string_literal(doc_id.into_bytes()),
            ]),
        );
    }

    let owner_password_owned = owner_password.unwrap_or(user_password).to_string();
    let permissions = Permissions::PRINTABLE
        | Permissions::COPYABLE
        | Permissions::COPYABLE_FOR_ACCESSIBILITY
        | Permissions::PRINTABLE_IN_HIGH_QUALITY;

    let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
    let version = EncryptionVersion::V4 {
        document: &doc,
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: &owner_password_owned,
        user_password,
        permissions,
    };

    let state = EncryptionState::try_from(version)
        .map_err(|e| PdfToolError::new(format!("Failed to prepare encryption: {e}")))?;
    doc.encrypt(&state)
        .map_err(|e| PdfToolError::new(format!("Failed to encrypt file: {e}")))?;
    doc.save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save encrypted output: {e}")))?;
    Ok(())
}
