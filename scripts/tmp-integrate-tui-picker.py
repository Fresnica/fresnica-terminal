from pathlib import Path


# app.rs
p = Path("crates/tui/src/app.rs")
text = p.read_text()
old = "use fresnica_client::{BalanceSnapshot, FresnicaClient, HistorySnapshot, OpenOffer, WalletRecord};"
new = "use fresnica_client::{\n    AssetCatalogEntry, BalanceSnapshot, FresnicaClient, HistorySnapshot, OpenOffer, WalletRecord,\n    MAX_ASSET_CATALOG_LIMIT,\n};"
if text.count(old) != 1:
    raise SystemExit("app.rs import pattern mismatch")
text = text.replace(old, new)

old = '''pub(super) struct App {\n    pub(super) client: FresnicaClient,\n    pub(super) wallets: Vec<WalletRecord>,\n    pub(super) selected: usize,\n    pub(super) balances: Vec<Value>,\n    pub(super) operations: Vec<Value>,\n    pub(super) offers: Vec<OpenOffer>,\n    pub(super) status: String,\n    pub(super) mode: Mode,\n}\n'''
new = '''#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub(super) enum AssetPickerTarget {\n    Send,\n    Trustline,\n    MarketBase,\n    MarketCounter,\n    OfferBase,\n    OfferCounter,\n}\n\npub(super) struct AssetPickerState {\n    pub(super) entries: Vec<AssetCatalogEntry>,\n    pub(super) selected: usize,\n    target: AssetPickerTarget,\n}\n\nimpl AssetPickerState {\n    pub(super) fn new(\n        entries: Vec<AssetCatalogEntry>,\n        target: AssetPickerTarget,\n        current: &str,\n    ) -> Self {\n        let selected = entries\n            .iter()\n            .position(|entry| entry.identity == current)\n            .unwrap_or(0);\n        Self {\n            entries,\n            selected,\n            target,\n        }\n    }\n\n    fn previous(&mut self) {\n        if self.entries.is_empty() {\n            return;\n        }\n        self.selected = if self.selected == 0 {\n            self.entries.len() - 1\n        } else {\n            self.selected - 1\n        };\n    }\n\n    fn next(&mut self) {\n        if !self.entries.is_empty() {\n            self.selected = (self.selected + 1) % self.entries.len();\n        }\n    }\n}\n\npub(super) struct App {\n    pub(super) client: FresnicaClient,\n    pub(super) wallets: Vec<WalletRecord>,\n    pub(super) selected: usize,\n    pub(super) balances: Vec<Value>,\n    pub(super) operations: Vec<Value>,\n    pub(super) offers: Vec<OpenOffer>,\n    pub(super) status: String,\n    pub(super) mode: Mode,\n    pub(super) asset_picker: Option<AssetPickerState>,\n}\n'''
if text.count(old) != 1:
    raise SystemExit("app.rs struct pattern mismatch")
text = text.replace(old, new)

old = '''            offers: Vec::new(),\n            status: String::new(),\n            mode: Mode::Browse,\n        };'''
new = '''            offers: Vec::new(),\n            status: String::new(),\n            mode: Mode::Browse,\n            asset_picker: None,\n        };'''
if text.count(old) != 1:
    raise SystemExit("app.rs constructor pattern mismatch")
text = text.replace(old, new)

old = '''    pub(super) fn selected_wallet(&self) -> &WalletRecord {\n        &self.wallets[self.selected]\n    }\n\n    fn refresh(&mut self) {'''
new = '''    pub(super) fn selected_wallet(&self) -> &WalletRecord {\n        &self.wallets[self.selected]\n    }\n\n    fn handle_asset_picker(&mut self, code: KeyCode) {\n        let mut chosen = None;\n        let mut cancelled = false;\n        if let Some(picker) = self.asset_picker.as_mut() {\n            match code {\n                KeyCode::Esc => cancelled = true,\n                KeyCode::Up | KeyCode::Char('k') => picker.previous(),\n                KeyCode::Down | KeyCode::Char('j') => picker.next(),\n                KeyCode::Enter => {\n                    chosen = picker\n                        .entries\n                        .get(picker.selected)\n                        .map(|entry| (picker.target, entry.identity.clone()));\n                }\n                _ => {}\n            }\n        }\n        if cancelled {\n            self.asset_picker = None;\n            self.status = "Asset selection cancelled; manual value kept".to_owned();\n        } else if let Some((target, identity)) = chosen {\n            self.asset_picker = None;\n            self.apply_asset_identity(target, &identity);\n            self.status = format!("Selected exact asset identity {identity}");\n        }\n    }\n\n    fn apply_asset_identity(&mut self, target: AssetPickerTarget, identity: &str) {\n        match (&mut self.mode, target) {\n            (Mode::Send(form), AssetPickerTarget::Send) => form.asset = identity.to_owned(),\n            (Mode::Trustline(form), AssetPickerTarget::Trustline) => form.asset = identity.to_owned(),\n            (Mode::Market(form), AssetPickerTarget::MarketBase) => form.base = identity.to_owned(),\n            (Mode::Market(form), AssetPickerTarget::MarketCounter) => form.counter = identity.to_owned(),\n            (Mode::Offer(form), AssetPickerTarget::OfferBase) => form.base = identity.to_owned(),\n            (Mode::Offer(form), AssetPickerTarget::OfferCounter) => form.counter = identity.to_owned(),\n            _ => {}\n        }\n    }\n\n    fn refresh(&mut self) {'''
if text.count(old) != 1:
    raise SystemExit("app.rs method insertion pattern mismatch")
text = text.replace(old, new)

old = '''    pub(super) fn handle_key(&mut self, code: KeyCode) -> bool {\n        let wallet_name = self.selected_wallet().name.clone();'''
new = '''    pub(super) fn handle_key(&mut self, code: KeyCode) -> bool {\n        if self.asset_picker.is_some() {\n            self.handle_asset_picker(code);\n            return false;\n        }\n\n        let wallet_name = self.selected_wallet().name.clone();'''
if text.count(old) != 1:
    raise SystemExit("app.rs handle start pattern mismatch")
text = text.replace(old, new)

old = '''        let mut offer_request = None;\n        let mut market_request = None;\n        let mut submit = false;'''
new = '''        let mut offer_request = None;\n        let mut market_request = None;\n        let mut asset_picker_request = None;\n        let mut submit = false;'''
if text.count(old) != 1:
    raise SystemExit("app.rs request vars pattern mismatch")
text = text.replace(old, new)

old = '''                KeyCode::Enter => payment_request = Some(form.request(&wallet_name)),\n                KeyCode::Backspace => {'''
new = '''                KeyCode::Enter => payment_request = Some(form.request(&wallet_name)),\n                KeyCode::Char('/') if form.active == 1 => {\n                    asset_picker_request = Some((AssetPickerTarget::Send, form.asset.clone()))\n                }\n                KeyCode::Backspace => {'''
if text.count(old) != 1:
    raise SystemExit("send picker pattern mismatch")
text = text.replace(old, new)

old = '''                KeyCode::Enter => trustline_request = Some(form.request(&wallet_name)),\n                KeyCode::Backspace => {'''
new = '''                KeyCode::Enter => trustline_request = Some(form.request(&wallet_name)),\n                KeyCode::Char('/') if form.active == 1 => {\n                    asset_picker_request = Some((AssetPickerTarget::Trustline, form.asset.clone()))\n                }\n                KeyCode::Backspace => {'''
if text.count(old) != 1:
    raise SystemExit("trustline picker pattern mismatch")
text = text.replace(old, new)

old = '''                KeyCode::Enter => match form.request(&wallet_name) {\n                    Ok(request) => offer_request = Some(request),\n                    Err(error) => self.status = error,\n                },\n                KeyCode::Backspace => {'''
new = '''                KeyCode::Enter => match form.request(&wallet_name) {\n                    Ok(request) => offer_request = Some(request),\n                    Err(error) => self.status = error,\n                },\n                KeyCode::Char('/') => {\n                    let target = match (form.action, form.active) {\n                        (OfferFormAction::Buy | OfferFormAction::Sell, 1)\n                        | (OfferFormAction::Update, 2) => Some(AssetPickerTarget::OfferBase),\n                        (OfferFormAction::Buy | OfferFormAction::Sell, 2)\n                        | (OfferFormAction::Update, 3) => Some(AssetPickerTarget::OfferCounter),\n                        _ => None,\n                    };\n                    if let Some(target) = target {\n                        let current = match target {\n                            AssetPickerTarget::OfferBase => form.base.clone(),\n                            AssetPickerTarget::OfferCounter => form.counter.clone(),\n                            _ => unreachable!(),\n                        };\n                        asset_picker_request = Some((target, current));\n                    } else if let Some(value) = form.current_mut() {\n                        value.push('/');\n                    }\n                }\n                KeyCode::Backspace => {'''
if text.count(old) != 1:
    raise SystemExit("offer picker pattern mismatch")
text = text.replace(old, new)

old = '''                KeyCode::Enter if form.active == 0 => form.active = 1,\n                KeyCode::Enter => market_request = Some(form.pair()),\n                KeyCode::Backspace => {'''
new = '''                KeyCode::Enter if form.active == 0 => form.active = 1,\n                KeyCode::Enter => market_request = Some(form.pair()),\n                KeyCode::Char('/') => {\n                    let (target, current) = if form.active == 0 {\n                        (AssetPickerTarget::MarketBase, form.base.clone())\n                    } else {\n                        (AssetPickerTarget::MarketCounter, form.counter.clone())\n                    };\n                    asset_picker_request = Some((target, current));\n                }\n                KeyCode::Backspace => {'''
if text.count(old) != 1:
    raise SystemExit("market picker pattern mismatch")
text = text.replace(old, new)

old = '''        if let Some(request) = payment_request {\n            match self.client.prepare_payment(&request) {'''
new = '''        if let Some((target, current)) = asset_picker_request {\n            match self.client.asset_catalog(MAX_ASSET_CATALOG_LIMIT, true) {\n                Ok(snapshot) => {\n                    self.asset_picker = Some(AssetPickerState::new(snapshot.entries, target, &current));\n                    self.status = "Choose an exact asset identity; Esc keeps manual entry".to_owned();\n                }\n                Err(error) => self.status = error,\n            }\n        }\n\n        if let Some(request) = payment_request {\n            match self.client.prepare_payment(&request) {'''
if text.count(old) != 1:
    raise SystemExit("app.rs picker effect insertion mismatch")
text = text.replace(old, new)
p.write_text(text)

# render.rs
p = Path("crates/tui/src/render.rs")
text = p.read_text()
old = "use super::app::App;"
new = "use super::app::{App, AssetPickerState};"
if text.count(old) != 1:
    raise SystemExit("render import pattern mismatch")
text = text.replace(old, new)

old = '''            Mode::Passcode { passcode, .. } => self.render_passcode(frame, passcode),\n        }\n    }'''
new = '''            Mode::Passcode { passcode, .. } => self.render_passcode(frame, passcode),\n        }\n        if let Some(picker) = &self.asset_picker {\n            self.render_asset_picker(frame, picker);\n        }\n    }'''
if text.count(old) != 1:
    raise SystemExit("render overlay insertion mismatch")
text = text.replace(old, new)

old = '''    fn render_footer(&self, frame: &mut Frame, area: Rect) {\n        let help = match &self.mode {'''
new = '''    fn render_footer(&self, frame: &mut Frame, area: Rect) {\n        let help = if self.asset_picker.is_some() {\n            "Up/Down or j/k select   Enter choose exact identity   Esc keep manual value"\n        } else {\n            match &self.mode {'''
if text.count(old) != 1:
    raise SystemExit("render footer start mismatch")
text = text.replace(old, new)

old = '''            Mode::Passcode { .. } => "Enter submit   Backspace edit   Esc cancel",\n        };\n        let body = format!("{}\\n{help}", self.status);'''
new = '''            Mode::Passcode { .. } => "Enter submit   Backspace edit   Esc cancel",\n            }\n        };\n        let body = format!("{}\\n{help}", self.status);'''
if text.count(old) != 1:
    raise SystemExit("render footer end mismatch")
text = text.replace(old, new)

for old, new in [
    ('Mode::Send(_) => "type value   Tab/Up/Down field   Enter next/prepare   Esc cancel",', 'Mode::Send(_) => "type value   / choose asset   Tab/Up/Down field   Enter next/prepare   Esc cancel",'),
    ('"action: Left/Right or a/l/x   Tab field   Enter next/prepare   Esc cancel"', '"action: Left/Right or a/l/x   / choose asset   Tab field   Enter next/prepare   Esc cancel"'),
    ('"action: Left/Right or b/s/e/x   Tab field   Space toggles trustline   Enter next/prepare"', '"action: Left/Right or b/s/e/x   / choose asset   Tab field   Space toggles trustline   Enter next/prepare"'),
    ('Mode::Market(_) => "type asset   Tab/Up/Down field   Enter next/open   Esc cancel",', 'Mode::Market(_) => "type asset   / choose asset   Tab/Up/Down field   Enter next/open   Esc cancel",'),
    ('"Destination may be a G address or a saved contact name.",', '"Asset may be typed exactly or chosen with /. Destination may be a G address or contact.",'),
    ('"Asset format: CODE:GISSUER. Add may leave limit empty for the default.",', '"Asset: / chooses catalog; manual CODE:GISSUER remains available. Add may leave limit empty.",'),
    ('"Action: b buy, s sell, e edit, x cancel. Price is counter per base.",', '"Action: b buy, s sell, e edit, x cancel. / chooses base/counter asset. Price is counter per base.",'),
    ('"Assets are XLM or full CODE:GISSUER identities. Price is counter per base.",', '"Assets are exact XLM or CODE:GISSUER; / opens catalog. Price is counter per base.",'),
]:
    if text.count(old) != 1:
        raise SystemExit(f"render replacement mismatch: {old}")
    text = text.replace(old, new)

old = '''    fn render_passcode(&self, frame: &mut Frame, passcode: &str) {\n        let area = popup_area(frame.area());'''
new = '''    fn render_asset_picker(&self, frame: &mut Frame, picker: &AssetPickerState) {\n        let area = popup_area(frame.area());\n        let visible = usize::from(area.height.saturating_sub(4) / 2).max(1);\n        let max_start = picker.entries.len().saturating_sub(visible);\n        let start = picker.selected.saturating_sub(visible / 2).min(max_start);\n        let end = (start + visible).min(picker.entries.len());\n        let mut lines = Vec::new();\n        for (index, entry) in picker.entries[start..end].iter().enumerate() {\n            let absolute = start + index;\n            let marker = if absolute == picker.selected { ">" } else { " " };\n            let identity = Line::from(format!("{marker} {}", entry.identity));\n            lines.push(if absolute == picker.selected {\n                identity.style(Style::new().add_modifier(Modifier::BOLD))\n            } else {\n                identity\n            });\n            let mut metadata = Vec::new();\n            if let Some(domain) = &entry.domain { metadata.push(domain.as_str()); }\n            if let Some(name) = &entry.name { metadata.push(name.as_str()); }\n            if let Some(organization) = &entry.organization { metadata.push(organization.as_str()); }\n            metadata.push(entry.source.as_str());\n            lines.push(Line::from(format!("  {}", metadata.join(" · "))));\n        }\n        if picker.entries.is_empty() {\n            lines.push(Line::from("No catalog entries; Esc keeps manual entry."));\n        }\n        frame.render_widget(Clear, area);\n        frame.render_widget(\n            Paragraph::new(lines).block(Block::bordered().title("Select exact asset identity")),\n            area,\n        );\n    }\n\n    fn render_passcode(&self, frame: &mut Frame, passcode: &str) {\n        let area = popup_area(frame.area());'''
if text.count(old) != 1:
    raise SystemExit("render picker function insertion mismatch")
text = text.replace(old, new)
p.write_text(text)

# main.rs help/tests
p = Path("crates/tui/src/main.rs")
text = p.read_text()
old = '''  d           open the DEX market selector\n\nWrite flow:'''
new = '''  d           open the DEX market selector\n  /           choose an exact asset identity while an asset field is focused\n\nWrite flow:'''
if text.count(old) != 1:
    raise SystemExit("main help pattern mismatch")
text = text.replace(old, new)

old = '''    use fresnica_client::{OfferRequest, OfferSide, TrustlineAction, WalletRecord};'''
new = '''    use fresnica_client::{\n        AssetCatalogEntry, OfferRequest, OfferSide, TrustlineAction, WalletRecord,\n    };'''
if text.count(old) != 1:
    raise SystemExit("main test client import mismatch")
text = text.replace(old, new)

old = '''    use super::render::compact_asset;\n    use super::state::{'''
new = '''    use super::app::{AssetPickerState, AssetPickerTarget};\n    use super::render::compact_asset;\n    use super::state::{'''
if text.count(old) != 1:
    raise SystemExit("main test app import mismatch")
text = text.replace(old, new)

old = '''                status: String::new(),\n                mode: Mode::Browse,\n            },'''
new = '''                status: String::new(),\n                mode: Mode::Browse,\n                asset_picker: None,\n            },'''
if text.count(old) != 1:
    raise SystemExit("main local app pattern mismatch")
text = text.replace(old, new)

insert_before = '''    #[test]\n    fn browse_send_enters_form_without_horizon() {'''
if text.count(insert_before) != 1:
    raise SystemExit("main tests insertion anchor mismatch")
tests = r'''    fn catalog_entry(identity: &str) -> AssetCatalogEntry {
        AssetCatalogEntry {
            identity: identity.to_owned(),
            domain: Some("example.org".to_owned()),
            name: None,
            organization: None,
            source: "test".to_owned(),
        }
    }

    #[test]
    fn asset_picker_applies_full_issuer_identity() {
        let (mut app, home) = local_app(false);
        let mut form = TrustlineForm::new();
        form.active = 1;
        form.asset = "MANUAL:GOLD".to_owned();
        app.mode = Mode::Trustline(form);
        let identity = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        app.asset_picker = Some(AssetPickerState::new(
            vec![catalog_entry("XLM"), catalog_entry(identity)],
            AssetPickerTarget::Trustline,
            "",
        ));
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Enter);
        assert!(app.asset_picker.is_none());
        assert!(matches!(&app.mode, Mode::Trustline(form) if form.asset == identity));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn asset_picker_cancel_preserves_manual_identity() {
        let (mut app, home) = local_app(false);
        let mut form = MarketForm::new();
        form.base = "USD:GMANUAL".to_owned();
        app.mode = Mode::Market(form);
        app.asset_picker = Some(AssetPickerState::new(
            vec![catalog_entry("XLM")],
            AssetPickerTarget::MarketBase,
            "USD:GMANUAL",
        ));
        app.handle_key(KeyCode::Esc);
        assert!(app.asset_picker.is_none());
        assert!(matches!(&app.mode, Mode::Market(form) if form.base == "USD:GMANUAL"));
        assert!(app.status.contains("manual value kept"));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn send_asset_field_opens_cache_first_picker_without_horizon() {
        let (mut app, home) = local_app(false);
        app.handle_key(KeyCode::Char('s'));
        app.handle_key(KeyCode::Enter);
        assert!(matches!(&app.mode, Mode::Send(form) if form.active == 1));
        app.handle_key(KeyCode::Char('/'));
        assert!(app.asset_picker.is_some());
        assert!(app.status.contains("exact asset identity"));
        let _ = std::fs::remove_dir_all(home);
    }

'''
text = text.replace(insert_before, tests + insert_before)
p.write_text(text)
