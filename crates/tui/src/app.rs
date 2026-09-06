use std::time::{SystemTime, UNIX_EPOCH};

use fresnica_client::{BalanceSnapshot, FresnicaClient, HistorySnapshot, OpenOffer, WalletRecord};
use ratatui::crossterm::event::KeyCode;
use serde_json::Value;
use zeroize::Zeroize;

use super::state::{
    MarketForm, MarketSnapshot, Mode, OfferForm, OfferFormAction, PreparedWrite, SendForm,
    TrustlineForm, TrustlineFormAction,
};

pub(super) struct App {
    pub(super) client: FresnicaClient,
    pub(super) wallets: Vec<WalletRecord>,
    pub(super) selected: usize,
    pub(super) balances: Vec<Value>,
    pub(super) operations: Vec<Value>,
    pub(super) offers: Vec<OpenOffer>,
    pub(super) status: String,
    pub(super) mode: Mode,
}

impl App {
    pub(super) fn new(
        client: FresnicaClient,
        requested_wallet: Option<&str>,
    ) -> Result<Self, String> {
        let wallets = client.wallets()?;
        if wallets.is_empty() {
            return Err(format!(
                "no {} wallets are available in {}",
                client.network(),
                client.storage().home().display()
            ));
        }

        let selected = match requested_wallet {
            Some(name) => wallets
                .iter()
                .position(|wallet| wallet.name == name)
                .ok_or_else(|| format!("wallet not found on {}: {name}", client.network()))?,
            None => {
                let resolved = client.resolve_wallet(None)?;
                wallets
                    .iter()
                    .position(|wallet| wallet.name == resolved.name)
                    .ok_or_else(|| {
                        format!(
                            "default wallet \"{}\" is not on {}",
                            resolved.name,
                            client.network()
                        )
                    })?
            }
        };

        let mut app = Self {
            client,
            wallets,
            selected,
            balances: Vec::new(),
            operations: Vec::new(),
            offers: Vec::new(),
            status: String::new(),
            mode: Mode::Browse,
        };
        app.refresh();
        Ok(app)
    }

    pub(super) fn selected_wallet(&self) -> &WalletRecord {
        &self.wallets[self.selected]
    }

    fn refresh(&mut self) {
        let name = self.selected_wallet().name.clone();
        let balance_result = self.client.balances(Some(&name));
        let history_result = self.client.history(Some(&name), 12);
        let offers_result = self.client.open_offers(Some(&name), 8);

        let mut failures = Vec::new();
        match balance_result {
            Ok(BalanceSnapshot { balances, .. }) => self.balances = balances,
            Err(error) => failures.push(format!("balances: {error}")),
        }
        match history_result {
            Ok(HistorySnapshot { operations, .. }) => self.operations = operations,
            Err(error) => failures.push(format!("activity: {error}")),
        }
        match offers_result {
            Ok(snapshot) => self.offers = snapshot.offers,
            Err(error) => failures.push(format!("offers: {error}")),
        }

        self.status = if failures.is_empty() {
            "Updated from Horizon".to_owned()
        } else {
            failures.join(" · ")
        };
    }

    fn select_previous(&mut self) {
        if self.wallets.len() > 1 {
            self.selected = if self.selected == 0 {
                self.wallets.len() - 1
            } else {
                self.selected - 1
            };
            self.refresh();
        }
    }

    fn select_next(&mut self) {
        if self.wallets.len() > 1 {
            self.selected = (self.selected + 1) % self.wallets.len();
            self.refresh();
        }
    }

    pub(super) fn handle_key(&mut self, code: KeyCode) -> bool {
        let wallet_name = self.selected_wallet().name.clone();
        let wallet_watch_only = self.selected_wallet().watch_only();
        let mut payment_request = None;
        let mut trustline_request = None;
        let mut offer_request = None;
        let mut market_request = None;
        let mut submit = false;

        match &mut self.mode {
            Mode::Browse => match code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Char('r') => self.refresh(),
                KeyCode::Char('[') | KeyCode::Left => self.select_previous(),
                KeyCode::Char(']') | KeyCode::Right => self.select_next(),
                KeyCode::Char('s') => {
                    if wallet_watch_only {
                        self.status =
                            "Selected wallet is watch-only; attach a signer before sending"
                                .to_owned();
                    } else {
                        self.mode = Mode::Send(SendForm::new());
                        self.status = "Preparing payment".to_owned();
                    }
                }
                KeyCode::Char('t') => {
                    if wallet_watch_only {
                        self.status =
                            "Selected wallet is watch-only; attach a signer before changing trustlines"
                                .to_owned();
                    } else {
                        self.mode = Mode::Trustline(TrustlineForm::new());
                        self.status = "Preparing trustline change".to_owned();
                    }
                }
                KeyCode::Char('d') => {
                    self.mode = Mode::Market(MarketForm::new());
                    self.status = "Choose a DEX market pair".to_owned();
                }
                _ => {}
            },
            Mode::Send(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Payment cancelled before review".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.active = (form.active + 1) % 4,
                KeyCode::BackTab | KeyCode::Up => form.active = (form.active + 3) % 4,
                KeyCode::Enter if form.active < 3 => form.active += 1,
                KeyCode::Enter => payment_request = Some(form.request(&wallet_name)),
                KeyCode::Backspace => {
                    form.current_mut().pop();
                }
                KeyCode::Char(character) => form.current_mut().push(character),
                _ => {}
            },
            Mode::Trustline(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Trustline change cancelled before review".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.active = (form.active + 1) % 3,
                KeyCode::BackTab | KeyCode::Up => form.active = (form.active + 2) % 3,
                KeyCode::Left if form.active == 0 => form.action = form.action.previous(),
                KeyCode::Right if form.active == 0 => form.action = form.action.next(),
                KeyCode::Char('a') if form.active == 0 => form.action = TrustlineFormAction::Add,
                KeyCode::Char('l') if form.active == 0 => {
                    form.action = TrustlineFormAction::SetLimit
                }
                KeyCode::Char('x') if form.active == 0 => form.action = TrustlineFormAction::Remove,
                KeyCode::Enter if form.active == 0 => form.active = 1,
                KeyCode::Enter
                    if form.active == 1 && form.action == TrustlineFormAction::Remove =>
                {
                    trustline_request = Some(form.request(&wallet_name));
                }
                KeyCode::Enter if form.active == 1 => form.active = 2,
                KeyCode::Enter => trustline_request = Some(form.request(&wallet_name)),
                KeyCode::Backspace => {
                    if let Some(value) = form.current_mut() {
                        value.pop();
                    }
                }
                KeyCode::Char(character) => {
                    if let Some(value) = form.current_mut() {
                        value.push(character);
                    }
                }
                _ => {}
            },
            Mode::Offer(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Offer change cancelled before review".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.next_field(),
                KeyCode::BackTab | KeyCode::Up => form.previous_field(),
                KeyCode::Left if form.active == 0 => {
                    form.action = form.action.previous();
                    form.normalize_active();
                }
                KeyCode::Right if form.active == 0 => {
                    form.action = form.action.next();
                    form.normalize_active();
                }
                KeyCode::Char('b') if form.active == 0 => form.action = OfferFormAction::Buy,
                KeyCode::Char('s') if form.active == 0 => form.action = OfferFormAction::Sell,
                KeyCode::Char('e') if form.active == 0 => {
                    form.action = OfferFormAction::Update;
                    form.normalize_active();
                }
                KeyCode::Char('x') if form.active == 0 => {
                    form.action = OfferFormAction::Cancel;
                    form.normalize_active();
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                    if form.active == 5
                        && matches!(form.action, OfferFormAction::Buy | OfferFormAction::Sell) =>
                {
                    form.allow_trustline = !form.allow_trustline;
                }
                KeyCode::Enter if form.active + 1 < form.field_count() => form.next_field(),
                KeyCode::Enter => match form.request(&wallet_name) {
                    Ok(request) => offer_request = Some(request),
                    Err(error) => self.status = error,
                },
                KeyCode::Backspace => {
                    if let Some(value) = form.current_mut() {
                        value.pop();
                    }
                }
                KeyCode::Char(character) => {
                    if let Some(value) = form.current_mut() {
                        value.push(character);
                    }
                }
                _ => {}
            },
            Mode::Market(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Market selection cancelled".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.active = (form.active + 1) % 2,
                KeyCode::BackTab | KeyCode::Up => form.active = (form.active + 1) % 2,
                KeyCode::Enter if form.active == 0 => form.active = 1,
                KeyCode::Enter => market_request = Some(form.pair()),
                KeyCode::Backspace => {
                    form.current_mut().pop();
                }
                KeyCode::Char(character) => form.current_mut().push(character),
                _ => {}
            },
            Mode::MarketView(snapshot) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Closed SDEX market".to_owned();
                }
                KeyCode::Char('r') => {
                    market_request = Some((
                        snapshot.order_book.base.clone(),
                        snapshot.order_book.counter.clone(),
                    ));
                }
                KeyCode::Char('w') => {
                    market_request = Some((
                        snapshot.order_book.counter.clone(),
                        snapshot.order_book.base.clone(),
                    ));
                }
                KeyCode::Char('b')
                | KeyCode::Char('s')
                | KeyCode::Char('e')
                | KeyCode::Char('x') => {
                    if wallet_watch_only {
                        self.status =
                            "Selected wallet is watch-only; attach a signer before managing offers"
                                .to_owned();
                    } else {
                        let action = match code {
                            KeyCode::Char('b') => OfferFormAction::Buy,
                            KeyCode::Char('s') => OfferFormAction::Sell,
                            KeyCode::Char('e') => OfferFormAction::Update,
                            KeyCode::Char('x') => OfferFormAction::Cancel,
                            _ => unreachable!(),
                        };
                        let base = snapshot.order_book.base.clone();
                        let counter = snapshot.order_book.counter.clone();
                        self.mode = Mode::Offer(OfferForm::for_market(action, base, counter));
                        self.status = match action {
                            OfferFormAction::Buy => "Preparing DEX buy offer",
                            OfferFormAction::Sell => "Preparing DEX sell offer",
                            OfferFormAction::Update => "Enter the offer ID to edit",
                            OfferFormAction::Cancel => "Enter the offer ID to cancel",
                        }
                        .to_owned();
                    }
                }
                _ => {}
            },
            Mode::PaymentReview(prepared) => match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.mode = Mode::Passcode {
                        prepared: PreparedWrite::Payment(prepared.clone()),
                        passcode: String::new(),
                    };
                    self.status = "Enter Fresnica passphrase; input is masked".to_owned();
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Payment cancelled after review".to_owned();
                }
                _ => {}
            },
            Mode::TrustlineReview(prepared) => match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.mode = Mode::Passcode {
                        prepared: PreparedWrite::Trustline(prepared.clone()),
                        passcode: String::new(),
                    };
                    self.status = "Enter Fresnica passphrase; input is masked".to_owned();
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Trustline change cancelled after review".to_owned();
                }
                _ => {}
            },
            Mode::OfferReview(prepared) => match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.mode = Mode::Passcode {
                        prepared: PreparedWrite::Offer(prepared.clone()),
                        passcode: String::new(),
                    };
                    self.status = "Enter Fresnica passphrase; input is masked".to_owned();
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Offer change cancelled after review".to_owned();
                }
                _ => {}
            },
            Mode::Passcode { passcode, .. } => match code {
                KeyCode::Esc => {
                    passcode.zeroize();
                    self.mode = Mode::Browse;
                    self.status = "Transaction cancelled before signing".to_owned();
                }
                KeyCode::Enter if passcode.is_empty() => {
                    self.status = "Fresnica passphrase cannot be empty".to_owned();
                }
                KeyCode::Enter => submit = true,
                KeyCode::Backspace => {
                    passcode.pop();
                }
                KeyCode::Char(character) => passcode.push(character),
                _ => {}
            },
        }

        if let Some(request) = payment_request {
            match self.client.prepare_payment(&request) {
                Ok(prepared) => {
                    self.mode = Mode::PaymentReview(prepared);
                    self.status = "Review the exact prepared payment before signing".to_owned();
                }
                Err(error) => self.status = error,
            }
        }

        if let Some(request) = trustline_request {
            match self.client.prepare_trustline(&request) {
                Ok(prepared) => {
                    self.mode = Mode::TrustlineReview(prepared);
                    self.status =
                        "Review the exact prepared trustline change before signing".to_owned();
                }
                Err(error) => self.status = error,
            }
        }

        if let Some(request) = offer_request {
            match self.client.prepare_offer(&request) {
                Ok(prepared) => {
                    self.mode = Mode::OfferReview(prepared);
                    self.status = "Review the exact prepared SDEX offer before signing".to_owned();
                }
                Err(error) => self.status = error,
            }
        }

        if let Some((base, counter)) = market_request {
            let snapshot = (|| {
                let order_book = self.client.order_book(&base, &counter)?;
                let trades = self.client.pair_trades(&base, &counter, 8)?;
                let end_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| "system clock is before Unix epoch".to_owned())?
                    .as_millis() as u64;
                let candles = self.client.candles(
                    &base,
                    &counter,
                    3_600_000,
                    Some(end_time.saturating_sub(24 * 3_600_000)),
                    Some(end_time),
                    None,
                    6,
                )?;
                Ok::<_, String>(MarketSnapshot {
                    order_book,
                    trades,
                    candles,
                })
            })();
            match snapshot {
                Ok(snapshot) => {
                    self.status = format!(
                        "Updated SDEX market {}/{}",
                        snapshot.order_book.base, snapshot.order_book.counter
                    );
                    self.mode = Mode::MarketView(snapshot);
                }
                Err(error) => self.status = error,
            }
        }

        if submit {
            let result = match &mut self.mode {
                Mode::Passcode { prepared, passcode } => {
                    let result = match prepared {
                        PreparedWrite::Payment(prepared) => {
                            self.client.submit_payment(prepared, passcode.as_str())
                        }
                        PreparedWrite::Trustline(prepared) => {
                            self.client.submit_trustline(prepared, passcode.as_str())
                        }
                        PreparedWrite::Offer(prepared) => {
                            self.client.submit_offer(prepared, passcode.as_str())
                        }
                    };
                    passcode.zeroize();
                    result
                }
                _ => return false,
            };
            match result {
                Ok(submission) => {
                    self.mode = Mode::Browse;
                    self.refresh();
                    self.status = match submission.ledger {
                        Some(ledger) => format!("Submitted {} in ledger {ledger}", submission.hash),
                        None => format!("Submitted {}", submission.hash),
                    };
                }
                Err(error) if error.contains("invalid Fresnica passcode") => {
                    self.status = error.replace("Fresnica passcode", "Fresnica passphrase");
                }
                Err(error) => {
                    self.mode = Mode::Browse;
                    self.status = error;
                }
            }
        }

        false
    }
}
