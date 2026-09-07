use std::io::{self, Write};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_client::{sign_sep53_message, verify_sep53_message, WalletStorage};
use serde_json::json;

const SIGN_USAGE: &str = "usage: fresnica message sign TEXT [--wallet NAME] [-y] [--json]";
const VERIFY_USAGE: &str = "usage: fresnica message verify G... SIGNATURE_BASE64 TEXT [--json]";

pub fn command_message(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    match arguments.first().map(String::as_str) {
        Some("sign") => command_sign(storage, &arguments[1..]),
        Some("verify") => command_verify(&arguments[1..]),
        _ => Err("usage: fresnica message sign|verify ...".to_owned()),
    }
}

fn command_sign(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    crate::diagnostics::stage("message: parse SEP-53 signing request");
    let request = SignRequest::parse(arguments)?;
    if request.json && !request.yes {
        return Err("message sign --json requires -y to keep stdout machine-readable".to_owned());
    }
    let record = storage.resolve(request.wallet.as_deref())?;
    if record.watch_only() {
        return Err("watch-only wallet has no signing material".to_owned());
    }

    if !request.json {
        render_signing_review(&record.name, &record.address, &request.message);
        if !request.yes && !confirm_signing()? {
            println!("Message signing cancelled.");
            return Ok(());
        }
    }

    crate::diagnostics::stage("message: sign exact SEP-53 bytes");
    let passphrase = crate::prompt_hidden("Fresnica passphrase: ")?;
    let signed = sign_sep53_message(&record, passphrase.as_str(), request.message.as_bytes())?;
    let signature_base64 = STANDARD.encode(signed.signature);
    let message_hash_hex = hex(&signed.message_hash);

    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "standard": "SEP-53",
                "wallet": record.name,
                "signer_public_key": signed.signer_public_key,
                "message": request.message,
                "message_hash_hex": message_hash_hex,
                "signature_base64": signature_base64,
            }))
            .map_err(|error| format!("unable to encode message signature: {error}"))?
        );
    } else {
        println!("Message hash: {message_hash_hex}");
        println!("Signature:    {signature_base64}");
    }
    Ok(())
}

pub fn command_verify(arguments: &[String]) -> Result<(), String> {
    crate::diagnostics::stage("message: verify SEP-53 signature");
    let request = VerifyRequest::parse(arguments)?;
    let signature = STANDARD
        .decode(request.signature_base64.as_bytes())
        .map_err(|_| "SEP-53 signature must be valid base64".to_owned())?;
    if signature.len() != 64 {
        return Err("SEP-53 signature must decode to exactly 64 bytes".to_owned());
    }
    let valid = verify_sep53_message(
        &request.signer_public_key,
        request.message.as_bytes(),
        &signature,
    )
    .is_ok();

    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "standard": "SEP-53",
                "signer_public_key": request.signer_public_key,
                "message": request.message,
                "valid": valid,
            }))
            .map_err(|error| format!("unable to encode verification result: {error}"))?
        );
    } else {
        println!("Signer:  {}", request.signer_public_key);
        println!("Message: {}", escaped_text(&request.message));
        println!("Valid:   {}", if valid { "yes" } else { "no" });
    }
    Ok(())
}

fn render_signing_review(wallet: &str, signer: &str, message: &str) {
    println!("Review SEP-53 message signing");
    println!("Wallet:  {wallet}");
    println!("Signer:  {signer}");
    println!("Message: {}", escaped_text(message));
    println!("Domain:  SEP-53 standard message signing (network-independent)");
    println!(
        "Note:    proves control of this signing key, not full authority over a multisig account"
    );
}

fn confirm_signing() -> Result<bool, String> {
    print!("Sign this exact message? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn escaped_text(message: &str) -> String {
    format!("{message:?}")
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SignRequest {
    message: String,
    wallet: Option<String>,
    yes: bool,
    json: bool,
}

impl SignRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let Some(message) = arguments.first() else {
            return Err(SIGN_USAGE.to_owned());
        };
        let mut wallet = None;
        let mut yes = false;
        let mut json = false;
        let mut index = 1;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| SIGN_USAGE.to_owned())?
                            .clone(),
                    );
                    index += 1;
                }
                "-y" | "--yes" => {
                    yes = true;
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(SIGN_USAGE.to_owned()),
            }
        }
        Ok(Self {
            message: message.clone(),
            wallet,
            yes,
            json,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifyRequest {
    signer_public_key: String,
    signature_base64: String,
    message: String,
    json: bool,
}

impl VerifyRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        if arguments.len() < 3 {
            return Err(VERIFY_USAGE.to_owned());
        }
        let mut json = false;
        let mut index = 3;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(VERIFY_USAGE.to_owned()),
            }
        }
        Ok(Self {
            signer_public_key: arguments[0].clone(),
            signature_base64: arguments[1].clone(),
            message: arguments[2].clone(),
            json,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC: &str = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";
    const SIGNATURE: &str =
        "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";

    #[test]
    fn sign_parser_keeps_exact_text_and_options() {
        let args = ["hello\nworld", "--wallet", "alpha", "-y", "--json"].map(str::to_owned);
        let request = SignRequest::parse(&args).unwrap();
        assert_eq!(request.message, "hello\nworld");
        assert_eq!(request.wallet.as_deref(), Some("alpha"));
        assert!(request.yes);
        assert!(request.json);
    }

    #[test]
    fn verify_parser_accepts_official_sep53_vector_shape() {
        let args = [PUBLIC, SIGNATURE, "Hello, World!", "--json"].map(str::to_owned);
        let request = VerifyRequest::parse(&args).unwrap();
        let signature = STANDARD
            .decode(request.signature_base64.as_bytes())
            .unwrap();
        assert_eq!(signature.len(), 64);
        verify_sep53_message(PUBLIC, request.message.as_bytes(), &signature).unwrap();
    }

    #[test]
    fn escaped_review_text_does_not_emit_control_characters() {
        let escaped = escaped_text("line1\n\u{1b}[31mline2");
        assert!(!escaped.contains('\n'));
        assert!(!escaped.contains('\u{1b}'));
        assert!(escaped.contains("\\n"));
        assert!(escaped.contains("\\u{1b}"));
    }
}
