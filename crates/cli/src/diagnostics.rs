use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

static VERBOSITY: AtomicU8 = AtomicU8::new(0);
static LAST_STAGE: Mutex<Option<&'static str>> = Mutex::new(None);

const FRESNICA_REVISION: &str = include_str!("../../../FRESNICA_REV");

pub fn leading_verbosity(arguments: &[String]) -> u8 {
    let mut verbosity = 0u8;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "-v" | "--verbose" => {
                verbosity = (verbosity + 1).min(2);
                index += 1;
            }
            "-vv" => {
                verbosity = 2;
                index += 1;
            }
            "--home" | "--network" => {
                if index + 1 >= arguments.len() {
                    break;
                }
                index += 2;
            }
            _ => break,
        }
    }
    verbosity
}

pub fn set_verbosity(verbosity: u8) {
    VERBOSITY.store(verbosity.min(2), Ordering::Relaxed);
}

pub fn stage(stage: &'static str) {
    if let Ok(mut current) = LAST_STAGE.lock() {
        *current = Some(stage);
    }
    if VERBOSITY.load(Ordering::Relaxed) >= 1 {
        eprintln!("[fresnica] {stage}");
    }
}

pub fn startup(network: &str) {
    if VERBOSITY.load(Ordering::Relaxed) < 2 {
        return;
    }
    eprintln!("[fresnica] cli-version={}", env!("CARGO_PKG_VERSION"));
    eprintln!("[fresnica] fresnica-source={}", fresnica_revision());
    eprintln!("[fresnica] network={network}");
    eprintln!("[fresnica] diagnostics=raw-arguments-and-hidden-input-omitted");
}

pub fn render_error(error: &str) {
    eprintln!("Error: {error}");
    let verbosity = VERBOSITY.load(Ordering::Relaxed);
    if verbosity == 0 {
        eprintln!("Hint: rerun with -v for diagnostic context.");
        return;
    }
    if let Ok(current) = LAST_STAGE.lock() {
        if let Some(stage) = *current {
            eprintln!("Failed during: {stage}");
        }
    }
    if verbosity == 1 {
        eprintln!("Hint: use -vv for version/network diagnostics.");
    }
}

pub fn fresnica_revision() -> &'static str {
    FRESNICA_REVISION.trim()
}

pub fn short_fresnica_revision() -> &'static str {
    let revision = fresnica_revision();
    revision.get(..12).unwrap_or(revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_is_read_only_from_leading_global_options() {
        let args = ["-v", "--network", "testnet", "--verbose", "account"].map(str::to_owned);
        assert_eq!(leading_verbosity(&args), 2);

        let memo = ["send", "1", "XLM", "to", "G...", "--memo", "-vv"].map(str::to_owned);
        assert_eq!(leading_verbosity(&memo), 0);
    }
}
