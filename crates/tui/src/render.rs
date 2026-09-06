use fresnica_client::{
    balance_asset_label, operation_summary, OfferReviewDetails, PreparedOffer, PreparedPayment,
    PreparedTrustline,
};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, List, ListItem, Paragraph, Row, Table};
use ratatui::Frame;
use serde_json::Value;

use super::app::App;
use super::state::{
    MarketForm, MarketSnapshot, Mode, OfferForm, OfferFormAction, SendForm, TrustlineForm,
    TrustlineFormAction,
};

impl App {
    pub(super) fn render(&self, frame: &mut Frame) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(4),
        ])
        .areas(frame.area());

        self.render_header(frame, header_area);
        if main_area.width >= 100 {
            let [portfolio_area, activity_area] =
                Layout::horizontal([Constraint::Percentage(68), Constraint::Percentage(32)])
                    .areas(main_area);
            let [assets_area, offers_area] =
                Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .areas(portfolio_area);
            self.render_assets(frame, assets_area);
            self.render_offers(frame, offers_area);
            self.render_activity(frame, activity_area);
        } else {
            let [assets_area, offers_area, activity_area] = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Percentage(25),
                Constraint::Percentage(30),
            ])
            .areas(main_area);
            self.render_assets(frame, assets_area);
            self.render_offers(frame, offers_area);
            self.render_activity(frame, activity_area);
        }
        self.render_footer(frame, footer_area);

        match &self.mode {
            Mode::Browse => {}
            Mode::Send(form) => self.render_send_form(frame, form),
            Mode::Trustline(form) => self.render_trustline_form(frame, form),
            Mode::Offer(form) => self.render_offer_form(frame, form),
            Mode::Market(form) => self.render_market_form(frame, form),
            Mode::MarketView(snapshot) => self.render_market_view(frame, snapshot),
            Mode::PaymentReview(prepared) => self.render_payment_review(frame, prepared),
            Mode::TrustlineReview(prepared) => self.render_trustline_review(frame, prepared),
            Mode::OfferReview(prepared) => self.render_offer_review(frame, prepared),
            Mode::Passcode { passcode, .. } => self.render_passcode(frame, passcode),
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let wallet = self.selected_wallet();
        let capability = if wallet.watch_only() {
            "Watch-only"
        } else {
            "Local signer"
        };
        let title = format!(
            "{}  [{}]  {}  ({}/{})",
            wallet.name,
            wallet.network,
            capability,
            self.selected + 1,
            self.wallets.len()
        );
        let body = format!(
            "{}\n{}",
            wallet.address, "Fresnica Rust TUI · shared client/service layer"
        );
        frame.render_widget(
            Paragraph::new(body).block(Block::bordered().title(title)),
            area,
        );
    }

    fn render_assets(&self, frame: &mut Frame, area: Rect) {
        let header = Row::new(["Asset", "Balance", "Selling", "Buying"])
            .style(Style::new().add_modifier(Modifier::BOLD));
        let rows = self.balances.iter().map(|balance| {
            Row::new([
                balance_asset_label(balance),
                text(balance, "balance").unwrap_or("0").to_owned(),
                text(balance, "selling_liabilities")
                    .unwrap_or("0")
                    .to_owned(),
                text(balance, "buying_liabilities")
                    .unwrap_or("0")
                    .to_owned(),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Min(24),
                Constraint::Length(16),
                Constraint::Length(14),
                Constraint::Length(14),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title("Assets"));
        frame.render_widget(table, area);
    }

    fn render_offers(&self, frame: &mut Frame, area: Rect) {
        let header = Row::new(["ID", "Selling", "Buying", "Amount", "Price"])
            .style(Style::new().add_modifier(Modifier::BOLD));
        let rows = self.offers.iter().map(|offer| {
            Row::new([
                offer.offer_id.to_string(),
                compact_asset(&offer.selling),
                compact_asset(&offer.buying),
                offer.amount.clone(),
                offer.price.clone(),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Min(14),
                Constraint::Min(14),
                Constraint::Length(14),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title("Open offers"));
        frame.render_widget(table, area);
    }

    fn render_activity(&self, frame: &mut Frame, area: Rect) {
        let address = &self.selected_wallet().address;
        let items = if self.operations.is_empty() {
            vec![ListItem::new("No recent activity")]
        } else {
            self.operations
                .iter()
                .map(|operation| {
                    let created_at = text(operation, "created_at").unwrap_or("?");
                    let operation_type = text(operation, "type").unwrap_or("unknown");
                    ListItem::new(Line::from(format!(
                        "{created_at}  {operation_type}  {}",
                        operation_summary(operation, address)
                    )))
                })
                .collect()
        };
        frame.render_widget(
            List::new(items).block(Block::bordered().title("Recent activity")),
            area,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let help = match &self.mode {
            Mode::Browse => {
                "q quit   r refresh   [ / ] switch wallet   s send   t manage assets   d DEX"
            }
            Mode::Send(_) => "type value   Tab/Up/Down field   Enter next/prepare   Esc cancel",
            Mode::Trustline(_) => {
                "action: Left/Right or a/l/x   Tab field   Enter next/prepare   Esc cancel"
            }
            Mode::Offer(_) => {
                "action: Left/Right or b/s/e/x   Tab field   Space toggles trustline   Enter next/prepare"
            }
            Mode::Market(_) => "type asset   Tab/Up/Down field   Enter next/open   Esc cancel",
            Mode::MarketView(_) => {
                "r refresh   w swap pair   b buy   s sell   e edit   x cancel   Esc back"
            }
            Mode::PaymentReview(_) | Mode::TrustlineReview(_) | Mode::OfferReview(_) => {
                "y/Enter sign   n/Esc cancel"
            }
            Mode::Passcode { .. } => "Enter submit   Backspace edit   Esc cancel",
        };
        let body = format!("{}\n{help}", self.status);
        frame.render_widget(Paragraph::new(body).block(Block::bordered()), area);
    }

    fn render_send_form(&self, frame: &mut Frame, form: &SendForm) {
        let area = popup_area(frame.area());
        let fields = [
            ("Amount", &form.amount),
            ("Asset", &form.asset),
            ("Destination", &form.destination),
            ("Memo (optional)", &form.memo),
        ];
        let lines = fields
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                let line = Line::from(format!("{label:<18} {value}"));
                if index == form.active {
                    line.style(Style::new().add_modifier(Modifier::BOLD))
                } else {
                    line
                }
            })
            .chain(std::iter::once(Line::from("")))
            .chain(std::iter::once(Line::from(
                "Destination may be a G address or a saved contact name.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Prepare payment")),
            area,
        );
    }

    fn render_trustline_form(&self, frame: &mut Frame, form: &TrustlineForm) {
        let area = popup_area(frame.area());
        let limit_value = if form.action == TrustlineFormAction::Remove {
            "(not used)"
        } else if form.limit.is_empty() && form.action == TrustlineFormAction::Add {
            "(default Fresnica limit)"
        } else {
            form.limit.as_str()
        };
        let fields = [
            ("Action", form.action.label()),
            ("Asset", form.asset.as_str()),
            ("Limit", limit_value),
        ];
        let lines = fields
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                let line = Line::from(format!("{label:<18} {value}"));
                if index == form.active {
                    line.style(Style::new().add_modifier(Modifier::BOLD))
                } else {
                    line
                }
            })
            .chain(std::iter::once(Line::from("")))
            .chain(std::iter::once(Line::from(
                "Asset format: CODE:GISSUER. Add may leave limit empty for the default.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Prepare trustline change")),
            area,
        );
    }

    fn render_offer_form(&self, frame: &mut Frame, form: &OfferForm) {
        let area = popup_area(frame.area());
        let fields = if form.action == OfferFormAction::Cancel {
            vec![
                ("Action", form.action.label().to_owned()),
                ("Offer ID", form.offer_id.clone()),
            ]
        } else if form.action == OfferFormAction::Update {
            vec![
                ("Action", form.action.label().to_owned()),
                ("Offer ID", form.offer_id.clone()),
                ("Base", form.base.clone()),
                ("Counter", form.counter.clone()),
                ("Amount", form.amount.clone()),
                ("Price", form.price.clone()),
            ]
        } else {
            vec![
                ("Action", form.action.label().to_owned()),
                ("Base", form.base.clone()),
                ("Counter", form.counter.clone()),
                ("Amount", form.amount.clone()),
                ("Price", form.price.clone()),
                (
                    "Add trustline",
                    if form.allow_trustline { "yes" } else { "no" }.to_owned(),
                ),
            ]
        };
        let lines = fields
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                let line = Line::from(format!("{label:<18} {value}"));
                if index == form.active {
                    line.style(Style::new().add_modifier(Modifier::BOLD))
                } else {
                    line
                }
            })
            .chain(std::iter::once(Line::from("")))
            .chain(std::iter::once(Line::from(
                "Action: b buy, s sell, e edit, x cancel. Price is counter per base.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Prepare SDEX offer")),
            area,
        );
    }

    fn render_market_form(&self, frame: &mut Frame, form: &MarketForm) {
        let area = popup_area(frame.area());
        let fields = [
            ("Base", form.base.clone()),
            ("Counter", form.counter.clone()),
        ];
        let lines = fields
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                let line = Line::from(format!("{label:<12} {value}"));
                if index == form.active {
                    line.style(Style::new().add_modifier(Modifier::BOLD))
                } else {
                    line
                }
            })
            .chain(std::iter::once(Line::from("")))
            .chain(std::iter::once(Line::from(
                "Assets are XLM or full CODE:GISSUER identities. Price is counter per base.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Open SDEX market")),
            area,
        );
    }

    fn render_market_view(&self, frame: &mut Frame, snapshot: &MarketSnapshot) {
        let area = popup_area(frame.area());
        let [book_area, trades_area, candles_area] = Layout::vertical([
            Constraint::Percentage(54),
            Constraint::Percentage(23),
            Constraint::Percentage(23),
        ])
        .areas(area);
        let book = &snapshot.order_book;
        let header = Row::new(["Amount", "Price", "Price", "Amount"])
            .style(Style::new().add_modifier(Modifier::BOLD));
        let count = book.bids.len().max(book.asks.len());
        let rows = (0..count).map(|index| {
            let bid = book.bids.get(index);
            let ask = book.asks.get(index);
            Row::new([
                bid.map(|level| level.amount.clone()).unwrap_or_default(),
                bid.map(|level| level.price.clone()).unwrap_or_default(),
                ask.map(|level| level.price.clone()).unwrap_or_default(),
                ask.map(|level| level.amount.clone()).unwrap_or_default(),
            ])
        });
        let title = format!(
            "BID · BUY   {} / {}   ASK · SELL",
            compact_asset(&book.base),
            compact_asset(&book.counter)
        );
        let book_table = Table::new(
            rows,
            [
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title(title));

        let trade_rows = snapshot.trades.trades.iter().map(|trade| {
            Row::new([
                trade.ledger_close_time.clone().unwrap_or_default(),
                trade.base_side.label().to_owned(),
                trade.base_amount.clone(),
                trade.price.clone(),
            ])
        });
        let trades_table = Table::new(
            trade_rows,
            [
                Constraint::Percentage(40),
                Constraint::Percentage(12),
                Constraint::Percentage(24),
                Constraint::Percentage(24),
            ],
        )
        .header(
            Row::new(["Time", "Side", "Base amount", "Price"])
                .style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(1)
        .block(Block::bordered().title("Recent trades"));

        let candle_rows = snapshot.candles.candles.iter().map(|candle| {
            Row::new([
                candle.timestamp.to_string(),
                candle.open.clone(),
                candle.high.clone(),
                candle.low.clone(),
                candle.close.clone(),
                candle.base_volume.clone(),
                candle.trade_count.to_string(),
            ])
        });
        let candles_table = Table::new(
            candle_rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(13),
                Constraint::Percentage(13),
                Constraint::Percentage(13),
                Constraint::Percentage(13),
                Constraint::Percentage(18),
                Constraint::Percentage(10),
            ],
        )
        .header(
            Row::new([
                "Time(ms)", "Open", "High", "Low", "Close", "Volume", "Trades",
            ])
            .style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(1)
        .block(Block::bordered().title("1h candles · last 24h"));

        frame.render_widget(Clear, area);
        frame.render_widget(book_table, book_area);
        frame.render_widget(trades_table, trades_area);
        frame.render_widget(candles_table, candles_area);
    }

    fn render_payment_review(&self, frame: &mut Frame, prepared: &PreparedPayment) {
        let review = &prepared.review;
        let mut lines = vec![
            Line::from(format!("Operation: {}", review.operation.label())),
            Line::from(format!(
                "From:      {} ({})",
                review.wallet_name, review.source
            )),
            Line::from(match &review.contact_name {
                Some(name) => format!("To:        {name} ({})", review.destination),
                None => format!("To:        {}", review.destination),
            }),
            Line::from(format!("Amount:    {} {}", review.amount, review.asset)),
            Line::from(format!("Fee:       {} XLM", review.fee_xlm)),
            Line::from(format!("Network:   {}", review.network)),
        ];
        if let Some(memo) = &review.memo {
            lines.push(Line::from(format!(
                "Memo:      {} ({})",
                memo.value, memo.memo_type
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Press y/Enter to continue to signing."));
        let area = popup_area(frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Review transaction")),
            area,
        );
    }

    fn render_trustline_review(&self, frame: &mut Frame, prepared: &PreparedTrustline) {
        let review = &prepared.review;
        let mut lines = vec![
            Line::from(format!(
                "Operation: ChangeTrust ({})",
                review.operation.label()
            )),
            Line::from(format!(
                "Wallet:    {} ({})",
                review.wallet_name, review.source
            )),
            Line::from(format!("Asset:     {}", review.asset)),
            Line::from(format!("Fee:       {} XLM", review.fee_xlm)),
            Line::from(format!("Network:   {}", review.network)),
        ];
        if let Some(limit) = &review.limit {
            lines.insert(3, Line::from(format!("Limit:     {limit}")));
        }
        if let Some(authorization) = review.authorization {
            lines.insert(
                lines.len().saturating_sub(2),
                Line::from(format!("Auth:      {}", authorization.label())),
            );
        }
        if let Some(clawback_enabled) = review.clawback_enabled {
            lines.insert(
                lines.len().saturating_sub(2),
                Line::from(format!(
                    "Clawback:  {}",
                    if clawback_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                )),
            );
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Press y/Enter to continue to signing."));
        let area = popup_area(frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Review trustline change")),
            area,
        );
    }

    fn render_offer_review(&self, frame: &mut Frame, prepared: &PreparedOffer) {
        let review = &prepared.review;
        let mut lines = vec![
            Line::from(format!(
                "Operation: {} ({})",
                review.operation.label(),
                review.action.label()
            )),
            Line::from(format!(
                "Wallet:    {} ({})",
                review.wallet_name, review.source
            )),
        ];
        if let Some(offer_id) = review.offer_id {
            lines.push(Line::from(format!("Offer:     #{offer_id}")));
        }
        match &review.details {
            OfferReviewDetails::Trade {
                side,
                base,
                counter,
                amount,
                price,
                requested_price,
                price_n,
                price_d,
                total,
                trustline_asset,
                trustline_limit,
            } => {
                lines.push(Line::from(format!("Side:      {}", side.label())));
                lines.push(Line::from(format!("Pair:      {base} / {counter}")));
                lines.push(Line::from(format!("Amount:    {amount} {base}")));
                lines.push(Line::from(format!("Price:     {price} {counter}/{base}")));
                lines.push(Line::from(format!("Encoded:   {price_n}/{price_d}")));
                if let Some(requested) = requested_price {
                    lines.push(Line::from(format!(
                        "Requested: {requested} {counter}/{base}"
                    )));
                }
                lines.push(Line::from(format!("Total:     {total} {counter}")));
                if let Some(asset) = trustline_asset {
                    let limit = trustline_limit
                        .as_deref()
                        .map(|value| format!("; limit {value}"))
                        .unwrap_or_default();
                    lines.push(Line::from(format!(
                        "Trustline: + {asset}{limit} (explicitly approved)"
                    )));
                }
            }
            OfferReviewDetails::Cancel { selling, buying } => {
                lines.push(Line::from(format!("Selling:   {selling}")));
                lines.push(Line::from(format!("Buying:    {buying}")));
            }
        }
        lines.push(Line::from(format!("Fee:       {} XLM", review.fee_xlm)));
        lines.push(Line::from(format!("Network:   {}", review.network)));
        lines.push(Line::from(""));
        lines.push(Line::from("Press y/Enter to continue to signing."));
        let area = popup_area(frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Review SDEX offer")),
            area,
        );
    }

    fn render_passcode(&self, frame: &mut Frame, passcode: &str) {
        let area = popup_area(frame.area());
        let masked = "*".repeat(passcode.chars().count());
        let lines = vec![
            Line::from(
                "The Fresnica passphrase is passed to the shared client service only after review.",
            ),
            Line::from(""),
            Line::from(format!("Fresnica passphrase: {masked}")),
            Line::from(""),
            Line::from("Enter submits the prepared transaction."),
        ];
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Sign and submit")),
            area,
        );
    }
}

pub(super) fn compact_asset(value: &str) -> String {
    let Some((code, issuer)) = value.split_once(':') else {
        return value.to_owned();
    };
    if issuer.len() <= 14 {
        return value.to_owned();
    }
    format!("{code}:{}...{}", &issuer[..6], &issuer[issuer.len() - 4..])
}

fn popup_area(area: Rect) -> Rect {
    let [_, vertical, _] = Layout::vertical([
        Constraint::Percentage(14),
        Constraint::Percentage(72),
        Constraint::Percentage(14),
    ])
    .areas(area);
    let [_, popup, _] = Layout::horizontal([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ])
    .areas(vertical);
    popup
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
