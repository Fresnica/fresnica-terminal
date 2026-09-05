mod app;
mod render;
mod state;

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use fresnica_client::FresnicaClient;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use app::App;

const HELP: &str = r#"Fresnica native Rust TUI

Usage:
  fresnica-tui [--home PATH] [--network mainnet|testnet] [--wallet NAME]

Keys:
  q / Esc     quit
  r           refresh balances, offers, and recent activity
  [ / ]       previous / next wallet on the selected network
  s           prepare a payment from the selected signing wallet
  t           manage issued-asset trustlines
  d           open the DEX market selector

Write flow:
  form -> shared service preparation -> review -> Fresnica passphrase -> SDK/Core signing -> Horizon

The Rust TUI is an engineering/reference UI over fresnica-client. It does not
implement separate wallet, cryptographic, transaction, or Horizon semantics.
"#;

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments == ["--help"] || arguments == ["-h"] {
        print!("{HELP}");
        return Ok(());
    }
    if arguments == ["--version"] || arguments == ["-V"] {
        println!("fresnica-tui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let options = Options::parse(&arguments)?;
    let client = FresnicaClient::new(&options.home, &options.network)?;
    let mut app = App::new(client, options.wallet.as_deref())?;

    ratatui::run(|terminal| -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| app.render(frame))?;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press && app.handle_key(key.code) => {
                    break Ok(());
                }
                _ => {}
            }
        }
    })
    .map_err(|error| format!("terminal error: {error}"))
}

#[derive(Debug)]
struct Options {
    home: PathBuf,
    network: String,
    wallet: Option<String>,
}

impl Options {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut home = None;
        let mut network = "mainnet".to_owned();
        let mut wallet = None;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--home" => {
                    index += 1;
                    home = Some(expand_path(
                        arguments
                            .get(index)
                            .ok_or_else(|| "--home requires a path".to_owned())?,
                    )?);
                    index += 1;
                }
                "--network" => {
                    index += 1;
                    network = arguments
                        .get(index)
                        .ok_or_else(|| "--network requires mainnet or testnet".to_owned())?
                        .to_owned();
                    if !matches!(network.as_str(), "mainnet" | "testnet") {
                        return Err(format!("unknown network: {network}"));
                    }
                    index += 1;
                }
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| "--wallet requires a name".to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                other => return Err(format!("unknown option: {other}\n\n{HELP}")),
            }
        }
        let home = match home {
            Some(home) => home,
            None => default_home()?,
        };
        Ok(Self {
            home,
            network,
            wallet,
        })
    }
}

fn default_home() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("FRESNICA_HOME") {
        return expand_path(&home.to_string_lossy());
    }
    let base = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "unable to determine home directory; set FRESNICA_HOME".to_owned())?;
    Ok(PathBuf::from(base).join(".fresnica"))
}

fn expand_path(value: &str) -> Result<PathBuf, String> {
    if value == "~" {
        return env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned());
    }
    if let Some(rest) = value.strip_prefix("~/") {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned())?;
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(Path::new(value).to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    use fresnica_client::{OfferRequest, OfferSide, TrustlineAction, WalletRecord};
    use ratatui::crossterm::event::KeyCode;

    use super::render::compact_asset;
    use super::state::{
        MarketForm, Mode, OfferForm, OfferFormAction, SendForm, TrustlineForm, TrustlineFormAction,
    };

    #[test]
    fn parses_network_and_wallet_options() {
        let options = Options::parse(&[
            "--home".to_owned(),
            "/tmp/fresnica".to_owned(),
            "--network".to_owned(),
            "testnet".to_owned(),
            "--wallet".to_owned(),
            "alpha".to_owned(),
        ])
        .unwrap();
        assert_eq!(options.home, PathBuf::from("/tmp/fresnica"));
        assert_eq!(options.network, "testnet");
        assert_eq!(options.wallet.as_deref(), Some("alpha"));
    }

    #[test]
    fn rejects_unknown_network_before_starting_terminal() {
        let error = Options::parse(&["--network".to_owned(), "future".to_owned()]).unwrap_err();
        assert_eq!(error, "unknown network: future");
    }

    #[test]
    fn send_form_builds_shared_payment_request() {
        let mut form = SendForm::new();
        form.amount = "1.25".to_owned();
        form.destination = "Alice".to_owned();
        form.memo = "hello".to_owned();
        let request = form.request("primary");
        assert_eq!(request.wallet.as_deref(), Some("primary"));
        assert_eq!(request.amount, "1.25");
        assert_eq!(request.asset, "XLM");
        assert_eq!(request.destination, "Alice");
        assert_eq!(request.memo.as_deref(), Some("hello"));
    }

    #[test]
    fn market_offer_form_uses_dex_pair_and_action_focus() {
        let buy = OfferForm::for_market(
            OfferFormAction::Buy,
            "XLM".to_owned(),
            "USD:GISSUER".to_owned(),
        );
        assert_eq!(buy.action, OfferFormAction::Buy);
        assert_eq!(buy.base, "XLM");
        assert_eq!(buy.counter, "USD:GISSUER");
        assert_eq!(buy.active, 3);

        let edit = OfferForm::for_market(
            OfferFormAction::Update,
            "XLM".to_owned(),
            "USD:GISSUER".to_owned(),
        );
        assert_eq!(edit.action, OfferFormAction::Update);
        assert_eq!(edit.active, 1);
    }

    #[test]
    fn offer_form_builds_shared_create_request() {
        let mut form = OfferForm::new();
        form.action = OfferFormAction::Sell;
        form.base = "XLM".to_owned();
        form.counter = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        form.amount = "4.5".to_owned();
        form.price = "1.25".to_owned();
        form.allow_trustline = true;
        let request = form.request("primary").unwrap();
        assert!(matches!(
            request,
            OfferRequest::Create {
                wallet: Some(ref wallet),
                side: OfferSide::Sell,
                allow_trustline: true,
                ref amount,
                ref price,
                ..
            } if wallet == "primary" && amount == "4.5" && price == "1.25"
        ));
    }

    #[test]
    fn offer_form_builds_shared_cancel_request() {
        let mut form = OfferForm::new();
        form.action = OfferFormAction::Cancel;
        form.offer_id = "42".to_owned();
        assert!(matches!(
            form.request("primary").unwrap(),
            OfferRequest::Cancel {
                wallet: Some(ref wallet),
                offer_id: 42,
            } if wallet == "primary"
        ));
    }

    #[test]
    fn offer_form_builds_shared_update_request() {
        let mut form = OfferForm::new();
        form.action = OfferFormAction::Update;
        form.offer_id = "42".to_owned();
        form.base = "XLM".to_owned();
        form.counter = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        form.amount = "3.5".to_owned();
        form.price = "1.75".to_owned();
        assert!(matches!(
            form.request("primary").unwrap(),
            OfferRequest::Update {
                wallet: Some(ref wallet),
                offer_id: 42,
                ref amount,
                ref price,
                ..
            } if wallet == "primary" && amount == "3.5" && price == "1.75"
        ));
    }

    #[test]
    fn market_form_keeps_full_asset_pair_identity() {
        let mut form = MarketForm::new();
        form.base = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        let (base, counter) = form.pair();
        assert_eq!(
            base,
            "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
        );
        assert_eq!(counter, "XLM");
    }

    #[test]
    fn compact_asset_keeps_code_and_shortens_only_issuer() {
        let asset = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        assert_eq!(compact_asset("XLM"), "XLM");
        assert_eq!(compact_asset(asset), "USD:GAAAAA...AWHF");
    }

    #[test]
    fn trustline_form_builds_shared_service_request() {
        let mut form = TrustlineForm::new();
        form.action = TrustlineFormAction::SetLimit;
        form.asset = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        form.limit = "2500".to_owned();
        let request = form.request("primary");
        assert_eq!(request.wallet.as_deref(), Some("primary"));
        assert_eq!(
            request.action,
            TrustlineAction::SetLimit {
                limit: "2500".to_owned()
            }
        );
    }

    fn local_app(watch_only: bool) -> (App, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home = std::env::temp_dir().join(format!(
            "fresnica-tui-local-state-{}-{nonce}",
            std::process::id()
        ));
        let client = FresnicaClient::new(&home, "testnet").unwrap();
        let wallet = WalletRecord {
            name: "primary".to_owned(),
            address: "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR".to_owned(),
            wallet_type: if watch_only { "watch-only" } else { "secret" }.to_owned(),
            network: "testnet".to_owned(),
            secret: None,
            metadata: Default::default(),
        };
        (
            App {
                client,
                wallets: vec![wallet],
                selected: 0,
                balances: Vec::new(),
                operations: Vec::new(),
                offers: Vec::new(),
                status: String::new(),
                mode: Mode::Browse,
            },
            home,
        )
    }

    #[test]
    fn browse_send_enters_form_without_horizon() {
        let (mut app, home) = local_app(false);
        assert!(!app.handle_key(KeyCode::Char('s')));
        assert!(matches!(app.mode, Mode::Send(_)));
        assert_eq!(app.status, "Preparing payment");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn watch_only_send_is_blocked_without_horizon() {
        let (mut app, home) = local_app(true);
        assert!(!app.handle_key(KeyCode::Char('s')));
        assert!(matches!(app.mode, Mode::Browse));
        assert!(app.status.contains("watch-only"));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn send_form_navigation_and_cancel_stay_local() {
        let (mut app, home) = local_app(false);
        app.handle_key(KeyCode::Char('s'));
        app.handle_key(KeyCode::Char('1'));
        match &app.mode {
            Mode::Send(form) => {
                assert_eq!(form.amount, "1");
                assert_eq!(form.active, 0);
            }
            _ => panic!("expected send form"),
        }
        app.handle_key(KeyCode::Enter);
        assert!(matches!(&app.mode, Mode::Send(form) if form.active == 1));
        app.handle_key(KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Browse));
        assert_eq!(app.status, "Payment cancelled before review");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn trustline_action_and_cancel_stay_local() {
        let (mut app, home) = local_app(false);
        app.handle_key(KeyCode::Char('t'));
        app.handle_key(KeyCode::Right);
        assert!(matches!(
            &app.mode,
            Mode::Trustline(form) if form.action == TrustlineFormAction::SetLimit
        ));
        app.handle_key(KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Browse));
        assert_eq!(app.status, "Trustline change cancelled before review");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn market_entry_is_read_only_and_cancellable_for_watch_only_wallet() {
        let (mut app, home) = local_app(true);
        app.handle_key(KeyCode::Char('d'));
        assert!(matches!(app.mode, Mode::Market(_)));
        app.handle_key(KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Browse));
        assert_eq!(app.status, "Market selection cancelled");
        let _ = std::fs::remove_dir_all(home);
    }
}
