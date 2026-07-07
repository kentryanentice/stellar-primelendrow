use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use hmac::{Hmac, Mac};
use mongodb::{
    Database,
    bson::{Binary, doc, spec::BinarySubtype},
};
use serde::Deserialize;
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// KYC document storage backed by MongoDB.
///
/// Each document lives in the `kyc_files` collection keyed by its
/// bucket-relative path (`_id`), with the raw bytes as a BSON Binary. Images
/// are capped at 8MB by the submit handler, comfortably inside the 16MB BSON
/// document limit, so GridFS is unnecessary.
///
/// Reads go through short-lived HMAC-signed URLs served by this API itself
/// (`GET /kyc/files/{path}?exp=..&sig=..`) — the same access model as the old
/// Supabase signed URLs: possession of a fresh URL is the authorization, and
/// URLs expire in minutes.
#[derive(Clone)]
pub struct MongoStorage {
    db: Database,
    sign_secret: Vec<u8>,
    /// Absolute base URL of this API, used to build signed URLs the admin
    /// frontend can load directly (e.g. "http://localhost:8080").
    public_base: String,
}

#[derive(Deserialize)]
struct FileDoc {
    data: Binary,
    content_type: String,
}

impl MongoStorage {
    pub fn new(db: Database, sign_secret: Vec<u8>, public_base: String) -> Self {
        Self {
            db,
            sign_secret,
            public_base: public_base.trim_end_matches('/').to_string(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.sign_secret.is_empty()
    }

    fn files(&self) -> mongodb::Collection<FileDoc> {
        self.db.collection::<FileDoc>("kyc_files")
    }

    /// Store raw bytes under `path`. Private by construction: nothing serves
    /// this collection except the signed-URL route. No upsert — KYC paths are
    /// UUID-keyed, so an overwrite would mean a path collision bug, and the
    /// duplicate-key error surfaces it.
    pub async fn upload_private(
        &self,
        path: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<(), String> {
        self.db
            .collection::<mongodb::bson::Document>("kyc_files")
            .insert_one(doc! {
                "_id": path,
                "data": Binary { subtype: BinarySubtype::Generic, bytes: data },
                "content_type": content_type,
                "created_at": Utc::now().timestamp(),
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Fetch a stored document's bytes and content type.
    pub async fn fetch(&self, path: &str) -> Result<Option<(Vec<u8>, String)>, String> {
        let found = self
            .files()
            .find_one(doc! { "_id": path })
            .await
            .map_err(|e| e.to_string())?;
        Ok(found.map(|f| (f.data.bytes, f.content_type)))
    }

    /// Delete a single object. Idempotent: deleting a missing path is Ok.
    pub async fn delete(&self, path: &str) -> Result<(), String> {
        self.files()
            .delete_one(doc! { "_id": path })
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// MAC over the path and expiry, domain-separated so the signing key
    /// (shared with the KYC blind index) can never be confused across uses.
    fn file_mac(&self, path: &str, exp: i64) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(&self.sign_secret)
            .expect("HMAC accepts any key length");
        mac.update(b"kyc-file-url\n");
        mac.update(path.as_bytes());
        mac.update(b"\n");
        mac.update(exp.to_string().as_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    /// Mint a short-lived signed URL for a private object — the only way KYC
    /// documents are ever read. Expiry is the caller's choice; keep it to
    /// minutes, not days.
    pub async fn signed_url(&self, path: &str, expires_in_secs: u32) -> Result<String, String> {
        if !self.is_configured() {
            return Err("storage signing secret not configured".to_string());
        }
        let exp = Utc::now().timestamp() + i64::from(expires_in_secs);
        let sig = hex::encode(self.file_mac(path, exp));
        Ok(format!(
            "{}/kyc/files/{path}?exp={exp}&sig={sig}",
            self.public_base
        ))
    }

    /// Constant-time check of a presented URL signature.
    pub fn verify_signature(&self, path: &str, exp: i64, sig_hex: &str) -> bool {
        if !self.is_configured() || exp <= Utc::now().timestamp() {
            return false;
        }
        let Ok(sig) = hex::decode(sig_hex) else {
            return false;
        };
        let expected = self.file_mac(path, exp);
        expected.as_slice().ct_eq(sig.as_slice()).into()
    }
}

/// Decode a standard base64 string to bytes.
pub fn decode_b64(s: &str) -> Result<Vec<u8>, String> {
    STANDARD.decode(s).map_err(|e| e.to_string())
}
