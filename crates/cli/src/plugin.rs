use std::io;
use std::process::{Command, ExitStatus};

const PLUGIN_PREFIXES: [&str; 3] = ["fresnica", "stellar", "soroban"];

pub fn dispatch(command: &[String]) -> Result<Option<ExitStatus>, String> {
    let chain_len = command
        .iter()
        .take_while(|argument| !argument.starts_with("--"))
        .count();

    for len in (1..=chain_len).rev() {
        let name = command[..len].join("-");
        for prefix in PLUGIN_PREFIXES {
            let program = format!("{prefix}-{name}");
            match Command::new(&program).args(&command[len..]).status() {
                Ok(status) => return Ok(Some(status)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!("unable to run plugin {program}: {error}"));
                }
            }
        }
    }

    Ok(None)
}
