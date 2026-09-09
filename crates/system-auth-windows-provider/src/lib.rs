use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey};
use rsa::traits::PublicKeyParts;
use rsa::{BigUint, RsaPublicKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use signature::Verifier;
use std::io::{Read, Write};
use zeroize::Zeroizing;

pub const PIPE_NAME: &str = r"\\.\pipe\FresnicaSystemAuth.v1";
pub const SERVICE_NAME: &str = "FresnicaSystemAuth";
pub const RP_ID: &str = "system-auth.fresnica.com";
pub const RP_NAME: &str = "Fresnica System Auth";
pub const ORIGIN: &str = "https://system-auth.fresnica.com";
pub const UNLOCK_KEY_LEN: usize = 32;
pub const CHALLENGE_LEN: usize = 32;
pub const MAX_FRAME_LEN: usize = 16 * 1024;

const FLAG_USER_PRESENT: u8 = 0x01;
const FLAG_USER_VERIFIED: u8 = 0x04;
const FLAG_ATTESTED_CREDENTIAL: u8 = 0x40;
const FLAG_EXTENSION_DATA: u8 = 0x80;
const COSE_KTY_RSA: i64 = 3;
const COSE_ALG_RS256: i64 = -257;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Probe,
    Has {
        slot: String,
    },
    Enroll {
        slot: String,
        unlock_key: Vec<u8>,
    },
    Credential {
        credential_id: Vec<u8>,
        authenticator_data: Vec<u8>,
    },
    Release {
        slot: String,
    },
    Assertion {
        credential_id: Vec<u8>,
        authenticator_data: Vec<u8>,
        signature: Vec<u8>,
    },
    Delete {
        slot: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Missing,
    CreateCredential {
        client_data_json: Vec<u8>,
        user_id: Vec<u8>,
    },
    AssertionChallenge {
        client_data_json: Vec<u8>,
        credential_id: Vec<u8>,
    },
    Key {
        unlock_key: Vec<u8>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainRecord {
    pub version: u32,
    pub sid: String,
    pub credential_id: Vec<u8>,
    pub public_key_cose: Vec<u8>,
    pub fingerprint: String,
}

impl DomainRecord {
    pub fn new(sid: &str, credential_id: Vec<u8>, public_key_cose: Vec<u8>) -> Self {
        let fingerprint = domain_fingerprint(&credential_id, &public_key_cose);
        Self {
            version: 1,
            sid: sid.to_owned(),
            credential_id,
            public_key_cose,
            fingerprint,
        }
    }

    pub fn validate(&self, expected_sid: &str) -> Result<(), String> {
        if self.version != 1
            || self.sid != expected_sid
            || self.credential_id.is_empty()
            || self.credential_id.len() > 1024
            || self.public_key_cose.is_empty()
        {
            return Err("Windows System Auth domain state is invalid".to_owned());
        }
        parse_cose_rsa_public_key(&self.public_key_cose)?;
        if self.fingerprint != domain_fingerprint(&self.credential_id, &self.public_key_cose) {
            return Err("Windows System Auth domain fingerprint is invalid".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignerRecord {
    pub version: u32,
    pub sid: String,
    pub slot: String,
    pub domain_fingerprint: String,
    pub unlock_key: Vec<u8>,
}

impl SignerRecord {
    pub fn new(sid: &str, slot: &str, domain_fingerprint: &str, unlock_key: Vec<u8>) -> Self {
        Self {
            version: 1,
            sid: sid.to_owned(),
            slot: slot.to_owned(),
            domain_fingerprint: domain_fingerprint.to_owned(),
            unlock_key,
        }
    }

    pub fn validate(
        &self,
        expected_sid: &str,
        expected_slot: &str,
        expected_domain: &DomainRecord,
    ) -> Result<(), String> {
        if self.version != 1
            || self.sid != expected_sid
            || self.slot != expected_slot
            || self.domain_fingerprint != expected_domain.fingerprint
            || self.unlock_key.len() != UNLOCK_KEY_LEN
        {
            return Err("Windows System Auth signer state is invalid".to_owned());
        }
        Ok(())
    }
}

pub fn write_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    let payload = Zeroizing::new(
        serde_json::to_vec(value)
            .map_err(|error| format!("unable to encode Windows System Auth frame: {error}"))?,
    );
    if payload.is_empty() || payload.len() > MAX_FRAME_LEN {
        return Err("Windows System Auth frame is too large".to_owned());
    }
    writer
        .write_all(&(payload.len() as u32).to_le_bytes())
        .and_then(|_| writer.write_all(&payload))
        .and_then(|_| writer.flush())
        .map_err(|error| format!("unable to write Windows System Auth frame: {error}"))
}

pub fn read_frame<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<T, String> {
    let mut length = [0u8; 4];
    reader
        .read_exact(&mut length)
        .map_err(|error| format!("unable to read Windows System Auth frame length: {error}"))?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME_LEN {
        return Err("Windows System Auth frame length is invalid".to_owned());
    }
    let mut payload = Zeroizing::new(vec![0u8; length]);
    reader
        .read_exact(payload.as_mut_slice())
        .map_err(|error| format!("unable to read Windows System Auth frame: {error}"))?;
    serde_json::from_slice(&payload)
        .map_err(|error| format!("unable to decode Windows System Auth frame: {error}"))
}

pub fn validate_registration_authenticator_data(
    authenticator_data: &[u8],
    claimed_credential_id: &[u8],
) -> Result<Vec<u8>, String> {
    if authenticator_data.len() < 55 {
        return Err("Windows WebAuthn registration data is truncated".to_owned());
    }
    validate_rp_and_flags(
        authenticator_data,
        FLAG_USER_PRESENT | FLAG_USER_VERIFIED | FLAG_ATTESTED_CREDENTIAL,
    )?;
    if authenticator_data[32] & FLAG_EXTENSION_DATA != 0 {
        return Err("Windows WebAuthn registration unexpectedly contains extensions".to_owned());
    }
    let credential_len =
        u16::from_be_bytes([authenticator_data[53], authenticator_data[54]]) as usize;
    let credential_start = 55usize;
    let credential_end = credential_start
        .checked_add(credential_len)
        .ok_or_else(|| "Windows WebAuthn credential length overflowed".to_owned())?;
    if credential_end > authenticator_data.len() {
        return Err("Windows WebAuthn credential id is truncated".to_owned());
    }
    if authenticator_data[credential_start..credential_end] != *claimed_credential_id {
        return Err("Windows WebAuthn credential id does not match registration data".to_owned());
    }
    let cose = authenticator_data[credential_end..].to_vec();
    parse_cose_rsa_public_key(&cose)?;
    Ok(cose)
}

pub fn verify_webauthn_assertion(
    public_key_cose: &[u8],
    expected_credential_id: &[u8],
    actual_credential_id: &[u8],
    client_data_json: &[u8],
    authenticator_data: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    if actual_credential_id != expected_credential_id {
        return Err("Windows WebAuthn assertion used the wrong credential".to_owned());
    }
    if client_data_json.is_empty() || client_data_json.len() > 4096 {
        return Err("Windows WebAuthn client data is invalid".to_owned());
    }
    validate_rp_and_flags(authenticator_data, FLAG_USER_PRESENT | FLAG_USER_VERIFIED)?;
    if authenticator_data[32] & FLAG_ATTESTED_CREDENTIAL != 0 {
        return Err("Windows WebAuthn assertion contains registration-only data".to_owned());
    }

    let key = parse_cose_rsa_public_key(public_key_cose)?;
    let mut signed = Vec::with_capacity(authenticator_data.len() + 32);
    signed.extend_from_slice(authenticator_data);
    signed.extend_from_slice(&Sha256::digest(client_data_json));
    let signature = RsaSignature::try_from(signature)
        .map_err(|_| "Windows WebAuthn assertion signature length is invalid".to_owned())?;
    VerifyingKey::<Sha256>::new(key)
        .verify(&signed, &signature)
        .map_err(|_| "Windows WebAuthn assertion signature is invalid".to_owned())
}

fn validate_rp_and_flags(authenticator_data: &[u8], required_flags: u8) -> Result<(), String> {
    if authenticator_data.len() < 37 {
        return Err("Windows WebAuthn authenticator data is truncated".to_owned());
    }
    let expected_rp = Sha256::digest(RP_ID.as_bytes());
    if authenticator_data[..32] != expected_rp[..] {
        return Err("Windows WebAuthn relying-party hash is invalid".to_owned());
    }
    if authenticator_data[32] & required_flags != required_flags {
        return Err("Windows WebAuthn user-presence verification is missing".to_owned());
    }
    Ok(())
}

pub fn parse_cose_rsa_public_key(cose: &[u8]) -> Result<RsaPublicKey, String> {
    let mut cursor = CborCursor::new(cose);
    let entries = cursor.read_map_len()?;
    if entries == 0 || entries > 16 {
        return Err("Windows WebAuthn COSE key map is invalid".to_owned());
    }
    let mut kty = None;
    let mut alg = None;
    let mut modulus = None;
    let mut exponent = None;
    for _ in 0..entries {
        let key = cursor.read_int()?;
        match key {
            1 => set_once(&mut kty, cursor.read_int()?, "COSE key type")?,
            3 => set_once(&mut alg, cursor.read_int()?, "COSE algorithm")?,
            -1 => set_once(
                &mut modulus,
                cursor.read_bytes()?.to_vec(),
                "COSE RSA modulus",
            )?,
            -2 => set_once(
                &mut exponent,
                cursor.read_bytes()?.to_vec(),
                "COSE RSA exponent",
            )?,
            _ => cursor.skip_value(0)?,
        }
    }
    if !cursor.finished() {
        return Err("Windows WebAuthn COSE key contains trailing data".to_owned());
    }
    if kty != Some(COSE_KTY_RSA) || alg != Some(COSE_ALG_RS256) {
        return Err("Windows WebAuthn credential is not RSA-2048/RS256".to_owned());
    }
    let modulus = modulus.ok_or_else(|| "Windows WebAuthn RSA modulus is missing".to_owned())?;
    let exponent = exponent.ok_or_else(|| "Windows WebAuthn RSA exponent is missing".to_owned())?;
    if modulus.is_empty() || exponent.is_empty() || exponent.len() > 8 {
        return Err("Windows WebAuthn RSA key material is invalid".to_owned());
    }
    let key = RsaPublicKey::new(
        BigUint::from_bytes_be(&modulus),
        BigUint::from_bytes_be(&exponent),
    )
    .map_err(|_| "Windows WebAuthn RSA public key is invalid".to_owned())?;
    if key.n().bits() != 2048 {
        return Err("Windows WebAuthn RSA public key is not 2048 bits".to_owned());
    }
    Ok(key)
}

fn set_once<T>(slot: &mut Option<T>, value: T, label: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        Err(format!("Windows WebAuthn {label} is duplicated"))
    } else {
        Ok(())
    }
}

struct CborCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> CborCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn finished(&self) -> bool {
        self.position == self.bytes.len()
    }

    fn read_map_len(&mut self) -> Result<usize, String> {
        let (major, value) = self.read_head()?;
        if major != 5 {
            return Err("Windows WebAuthn COSE key is not a CBOR map".to_owned());
        }
        usize::try_from(value).map_err(|_| "Windows WebAuthn COSE map is too large".to_owned())
    }

    fn read_int(&mut self) -> Result<i64, String> {
        let (major, value) = self.read_head()?;
        match major {
            0 => i64::try_from(value)
                .map_err(|_| "Windows WebAuthn CBOR integer is too large".to_owned()),
            1 => {
                if value > i64::MAX as u64 {
                    return Err("Windows WebAuthn CBOR negative integer is too small".to_owned());
                }
                Ok(-1 - value as i64)
            }
            _ => Err("Windows WebAuthn CBOR value is not an integer".to_owned()),
        }
    }

    fn read_bytes(&mut self) -> Result<&'a [u8], String> {
        let (major, value) = self.read_head()?;
        if major != 2 {
            return Err("Windows WebAuthn COSE key value is not a byte string".to_owned());
        }
        let length = usize::try_from(value)
            .map_err(|_| "Windows WebAuthn CBOR byte string is too large".to_owned())?;
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| "Windows WebAuthn CBOR length overflowed".to_owned())?;
        if end > self.bytes.len() {
            return Err("Windows WebAuthn CBOR byte string is truncated".to_owned());
        }
        let start = self.position;
        self.position = end;
        Ok(&self.bytes[start..end])
    }

    fn skip_value(&mut self, depth: u8) -> Result<(), String> {
        if depth > 8 {
            return Err("Windows WebAuthn CBOR nesting is too deep".to_owned());
        }
        let (major, value) = self.read_head()?;
        match major {
            0 | 1 | 7 => Ok(()),
            2 | 3 => {
                let length = usize::try_from(value)
                    .map_err(|_| "Windows WebAuthn CBOR value is too large".to_owned())?;
                self.advance(length)
            }
            4 => {
                for _ in 0..value {
                    self.skip_value(depth + 1)?;
                }
                Ok(())
            }
            5 => {
                for _ in 0..value {
                    self.skip_value(depth + 1)?;
                    self.skip_value(depth + 1)?;
                }
                Ok(())
            }
            6 => self.skip_value(depth + 1),
            _ => Err("Windows WebAuthn CBOR value type is unsupported".to_owned()),
        }
    }

    fn read_head(&mut self) -> Result<(u8, u64), String> {
        let first = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| "Windows WebAuthn CBOR data is truncated".to_owned())?;
        self.position += 1;
        let major = first >> 5;
        let additional = first & 0x1f;
        let value = match additional {
            0..=23 => additional as u64,
            24 => self.read_uint(1)?,
            25 => self.read_uint(2)?,
            26 => self.read_uint(4)?,
            27 => self.read_uint(8)?,
            _ => return Err("Windows WebAuthn indefinite-length CBOR is unsupported".to_owned()),
        };
        Ok((major, value))
    }

    fn read_uint(&mut self, length: usize) -> Result<u64, String> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| "Windows WebAuthn CBOR integer overflowed".to_owned())?;
        if end > self.bytes.len() {
            return Err("Windows WebAuthn CBOR integer is truncated".to_owned());
        }
        let mut value = 0u64;
        for byte in &self.bytes[self.position..end] {
            value = (value << 8) | u64::from(*byte);
        }
        self.position = end;
        Ok(value)
    }

    fn advance(&mut self, length: usize) -> Result<(), String> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| "Windows WebAuthn CBOR length overflowed".to_owned())?;
        if end > self.bytes.len() {
            return Err("Windows WebAuthn CBOR value is truncated".to_owned());
        }
        self.position = end;
        Ok(())
    }
}

pub fn make_client_data(kind: &str, challenge: &[u8]) -> Result<Vec<u8>, String> {
    if !matches!(kind, "webauthn.create" | "webauthn.get") || challenge.len() != CHALLENGE_LEN {
        return Err("Windows WebAuthn client-data request is invalid".to_owned());
    }
    Ok(format!(
        "{{\"type\":\"{kind}\",\"challenge\":\"{}\",\"origin\":\"{ORIGIN}\",\"crossOrigin\":false}}",
        base64url(challenge)
    )
    .into_bytes())
}

fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut index = 0usize;
    while index + 3 <= bytes.len() {
        let block = (u32::from(bytes[index]) << 16)
            | (u32::from(bytes[index + 1]) << 8)
            | u32::from(bytes[index + 2]);
        output.push(TABLE[((block >> 18) & 63) as usize] as char);
        output.push(TABLE[((block >> 12) & 63) as usize] as char);
        output.push(TABLE[((block >> 6) & 63) as usize] as char);
        output.push(TABLE[(block & 63) as usize] as char);
        index += 3;
    }
    match bytes.len() - index {
        1 => {
            let block = u32::from(bytes[index]) << 16;
            output.push(TABLE[((block >> 18) & 63) as usize] as char);
            output.push(TABLE[((block >> 12) & 63) as usize] as char);
        }
        2 => {
            let block = (u32::from(bytes[index]) << 16) | (u32::from(bytes[index + 1]) << 8);
            output.push(TABLE[((block >> 18) & 63) as usize] as char);
            output.push(TABLE[((block >> 12) & 63) as usize] as char);
            output.push(TABLE[((block >> 6) & 63) as usize] as char);
        }
        _ => {}
    }
    output
}

fn domain_fingerprint(credential_id: &[u8], public_key_cose: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(credential_id);
    digest.update(public_key_cose);
    hex(&digest.finalize())
}

pub fn valid_slot(slot: &str) -> bool {
    let Some((public_key, fingerprint)) = slot.split_once(':') else {
        return false;
    };
    public_key.len() == 56
        && public_key.starts_with('G')
        && public_key
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
        && fingerprint.len() == 64
        && fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn sid_storage_id(sid: &str) -> String {
    hex(&Sha256::digest(sid.as_bytes()))
}

pub fn slot_storage_id(slot: &str) -> String {
    hex(&Sha256::digest(slot.as_bytes()))
}

pub fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use rsa::pkcs1v15::SigningKey;
    use rsa::RsaPrivateKey;
    use signature::{SignatureEncoding, Signer};

    fn slot() -> String {
        format!("G{}:{}", "A".repeat(55), "a".repeat(64))
    }

    fn encode_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
        match bytes.len() {
            0..=23 => output.push(0x40 | bytes.len() as u8),
            24..=255 => {
                output.push(0x58);
                output.push(bytes.len() as u8);
            }
            _ => {
                output.push(0x59);
                output.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
            }
        }
        output.extend_from_slice(bytes);
    }

    fn rsa_cose(public: &RsaPublicKey) -> Vec<u8> {
        let mut cose = vec![0xa4, 0x01, 0x03, 0x03, 0x39, 0x01, 0x00, 0x20];
        encode_bytes(&mut cose, &public.n().to_bytes_be());
        cose.push(0x21);
        encode_bytes(&mut cose, &public.e().to_bytes_be());
        cose
    }

    fn registration_data(credential_id: &[u8], cose: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&Sha256::digest(RP_ID.as_bytes()));
        data.push(FLAG_USER_PRESENT | FLAG_USER_VERIFIED | FLAG_ATTESTED_CREDENTIAL);
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&[0u8; 16]);
        data.extend_from_slice(&(credential_id.len() as u16).to_be_bytes());
        data.extend_from_slice(credential_id);
        data.extend_from_slice(cose);
        data
    }

    fn assertion_data() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&Sha256::digest(RP_ID.as_bytes()));
        data.push(FLAG_USER_PRESENT | FLAG_USER_VERIFIED);
        data.extend_from_slice(&0u32.to_be_bytes());
        data
    }

    #[test]
    fn protocol_round_trips_binary_fields_without_argv_encoding() {
        let request = Request::Enroll {
            slot: slot(),
            unlock_key: vec![7; UNLOCK_KEY_LEN],
        };
        let encoded = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<Request>(&encoded).unwrap(),
            request
        );
    }

    #[test]
    fn exact_slot_shape_is_path_safe() {
        let valid = slot();
        assert!(valid_slot(&valid));
        assert!(!valid_slot("../../wallet"));
        assert!(!valid_slot(&valid.replace(':', "/")));
        assert!(!valid_slot(&valid.to_uppercase()));
    }

    #[test]
    fn registration_and_assertion_require_exact_windows_hello_proof() {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public = private.to_public_key();
        let cose = rsa_cose(&public);
        let credential_id = vec![4u8; 32];
        let registered = validate_registration_authenticator_data(
            &registration_data(&credential_id, &cose),
            &credential_id,
        )
        .unwrap();
        assert_eq!(registered, cose);

        let client_data = make_client_data("webauthn.get", &[9u8; CHALLENGE_LEN]).unwrap();
        let authenticator_data = assertion_data();
        let mut signed = authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&client_data));
        let signature = SigningKey::<Sha256>::new(private).sign(&signed).to_vec();
        verify_webauthn_assertion(
            &cose,
            &credential_id,
            &credential_id,
            &client_data,
            &authenticator_data,
            &signature,
        )
        .unwrap();

        let mut wrong_client_data = client_data;
        wrong_client_data[0] ^= 1;
        assert!(verify_webauthn_assertion(
            &cose,
            &credential_id,
            &credential_id,
            &wrong_client_data,
            &authenticator_data,
            &signature,
        )
        .is_err());
    }

    #[test]
    fn domain_and_signer_state_bind_sid_slot_and_credential() {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let cose = rsa_cose(&private.to_public_key());
        let sid = "S-1-5-21-1-2-3-1001";
        let domain = DomainRecord::new(sid, vec![3u8; 32], cose);
        domain.validate(sid).unwrap();
        let signer = SignerRecord::new(sid, &slot(), &domain.fingerprint, vec![5; UNLOCK_KEY_LEN]);
        signer.validate(sid, &slot(), &domain).unwrap();
        assert!(signer.validate("S-1-5-21-other", &slot(), &domain).is_err());
    }

    #[test]
    fn storage_ids_do_not_expose_sid_or_slot() {
        let sid = "S-1-5-21-1-2-3-1001";
        let slot = slot();
        let sid_id = sid_storage_id(sid);
        let slot_id = slot_storage_id(&slot);
        assert_eq!(sid_id.len(), 64);
        assert_eq!(slot_id.len(), 64);
        assert!(!sid_id.contains(sid));
        assert!(!slot_id.contains(&slot));
    }
}
