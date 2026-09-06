use fresnica_client::{
    CandleSnapshot, OfferRequest, OfferSide, OrderBookSnapshot, PairTradesSnapshot, PaymentRequest,
    PreparedOffer, PreparedPayment, PreparedTrustline, TrustlineAction, TrustlineRequest,
};

pub(super) enum Mode {
    Browse,
    Send(SendForm),
    Trustline(TrustlineForm),
    Offer(OfferForm),
    Market(MarketForm),
    MarketView(MarketSnapshot),
    PaymentReview(PreparedPayment),
    TrustlineReview(PreparedTrustline),
    OfferReview(PreparedOffer),
    Passcode {
        prepared: PreparedWrite,
        passcode: String,
    },
}

pub(super) struct MarketSnapshot {
    pub(super) order_book: OrderBookSnapshot,
    pub(super) trades: PairTradesSnapshot,
    pub(super) candles: CandleSnapshot,
}

pub(super) struct MarketForm {
    pub(super) base: String,
    pub(super) counter: String,
    pub(super) active: usize,
}

impl MarketForm {
    pub(super) fn new() -> Self {
        Self {
            base: String::new(),
            counter: "XLM".to_owned(),
            active: 0,
        }
    }

    pub(super) fn current_mut(&mut self) -> &mut String {
        if self.active == 0 {
            &mut self.base
        } else {
            &mut self.counter
        }
    }

    pub(super) fn pair(&self) -> (String, String) {
        (self.base.clone(), self.counter.clone())
    }
}

#[derive(Clone)]
pub(super) enum PreparedWrite {
    Payment(PreparedPayment),
    Trustline(PreparedTrustline),
    Offer(PreparedOffer),
}

pub(super) struct SendForm {
    pub(super) amount: String,
    pub(super) asset: String,
    pub(super) destination: String,
    pub(super) memo: String,
    pub(super) active: usize,
}

impl SendForm {
    pub(super) fn new() -> Self {
        Self {
            amount: String::new(),
            asset: "XLM".to_owned(),
            destination: String::new(),
            memo: String::new(),
            active: 0,
        }
    }

    pub(super) fn current_mut(&mut self) -> &mut String {
        match self.active {
            0 => &mut self.amount,
            1 => &mut self.asset,
            2 => &mut self.destination,
            _ => &mut self.memo,
        }
    }

    pub(super) fn request(&self, wallet: &str) -> PaymentRequest {
        PaymentRequest {
            wallet: Some(wallet.to_owned()),
            amount: self.amount.clone(),
            asset: self.asset.clone(),
            destination: self.destination.clone(),
            memo: (!self.memo.is_empty()).then(|| self.memo.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrustlineFormAction {
    Add,
    SetLimit,
    Remove,
}

impl TrustlineFormAction {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::SetLimit => "limit",
            Self::Remove => "remove",
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::Add => Self::Remove,
            Self::SetLimit => Self::Add,
            Self::Remove => Self::SetLimit,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::Add => Self::SetLimit,
            Self::SetLimit => Self::Remove,
            Self::Remove => Self::Add,
        }
    }
}

pub(super) struct TrustlineForm {
    pub(super) action: TrustlineFormAction,
    pub(super) asset: String,
    pub(super) limit: String,
    pub(super) active: usize,
}

impl TrustlineForm {
    pub(super) fn new() -> Self {
        Self {
            action: TrustlineFormAction::Add,
            asset: String::new(),
            limit: String::new(),
            active: 0,
        }
    }

    pub(super) fn current_mut(&mut self) -> Option<&mut String> {
        match self.active {
            1 => Some(&mut self.asset),
            2 => Some(&mut self.limit),
            _ => None,
        }
    }

    pub(super) fn request(&self, wallet: &str) -> TrustlineRequest {
        let action = match self.action {
            TrustlineFormAction::Add => TrustlineAction::Add {
                limit: (!self.limit.is_empty()).then(|| self.limit.clone()),
            },
            TrustlineFormAction::SetLimit => TrustlineAction::SetLimit {
                limit: self.limit.clone(),
            },
            TrustlineFormAction::Remove => TrustlineAction::Remove,
        };
        TrustlineRequest {
            wallet: Some(wallet.to_owned()),
            asset: self.asset.clone(),
            action,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfferFormAction {
    Buy,
    Sell,
    Update,
    Cancel,
}

impl OfferFormAction {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Update => "update",
            Self::Cancel => "cancel",
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::Buy => Self::Cancel,
            Self::Sell => Self::Buy,
            Self::Update => Self::Sell,
            Self::Cancel => Self::Update,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Update,
            Self::Update => Self::Cancel,
            Self::Cancel => Self::Buy,
        }
    }
}

pub(super) struct OfferForm {
    pub(super) action: OfferFormAction,
    pub(super) offer_id: String,
    pub(super) base: String,
    pub(super) counter: String,
    pub(super) amount: String,
    pub(super) price: String,
    pub(super) allow_trustline: bool,
    pub(super) active: usize,
}

impl OfferForm {
    pub(super) fn new() -> Self {
        Self {
            action: OfferFormAction::Buy,
            offer_id: String::new(),
            base: String::new(),
            counter: "XLM".to_owned(),
            amount: String::new(),
            price: String::new(),
            allow_trustline: false,
            active: 0,
        }
    }

    pub(super) fn for_market(action: OfferFormAction, base: String, counter: String) -> Self {
        let mut form = Self::new();
        form.action = action;
        form.base = base;
        form.counter = counter;
        form.active = match action {
            OfferFormAction::Buy | OfferFormAction::Sell => 3,
            OfferFormAction::Update | OfferFormAction::Cancel => 1,
        };
        form
    }

    pub(super) fn field_count(&self) -> usize {
        if self.action == OfferFormAction::Cancel {
            2
        } else {
            6
        }
    }

    pub(super) fn next_field(&mut self) {
        self.active = (self.active + 1) % self.field_count();
    }

    pub(super) fn previous_field(&mut self) {
        self.active = (self.active + self.field_count() - 1) % self.field_count();
    }

    pub(super) fn normalize_active(&mut self) {
        if self.active >= self.field_count() {
            self.active = self.field_count() - 1;
        }
    }

    pub(super) fn current_mut(&mut self) -> Option<&mut String> {
        match (self.action, self.active) {
            (OfferFormAction::Cancel, 1) => Some(&mut self.offer_id),
            (OfferFormAction::Update, 1) => Some(&mut self.offer_id),
            (OfferFormAction::Update, 2) => Some(&mut self.base),
            (OfferFormAction::Update, 3) => Some(&mut self.counter),
            (OfferFormAction::Update, 4) => Some(&mut self.amount),
            (OfferFormAction::Update, 5) => Some(&mut self.price),
            (OfferFormAction::Buy | OfferFormAction::Sell, 1) => Some(&mut self.base),
            (OfferFormAction::Buy | OfferFormAction::Sell, 2) => Some(&mut self.counter),
            (OfferFormAction::Buy | OfferFormAction::Sell, 3) => Some(&mut self.amount),
            (OfferFormAction::Buy | OfferFormAction::Sell, 4) => Some(&mut self.price),
            _ => None,
        }
    }

    pub(super) fn request(&self, wallet: &str) -> Result<OfferRequest, String> {
        let wallet = Some(wallet.to_owned());
        match self.action {
            OfferFormAction::Buy | OfferFormAction::Sell => Ok(OfferRequest::Create {
                wallet,
                side: if self.action == OfferFormAction::Buy {
                    OfferSide::Buy
                } else {
                    OfferSide::Sell
                },
                base: self.base.clone(),
                counter: self.counter.clone(),
                amount: self.amount.clone(),
                price: self.price.clone(),
                allow_trustline: self.allow_trustline,
            }),
            OfferFormAction::Update => Ok(OfferRequest::Update {
                wallet,
                offer_id: self.offer_id()?,
                base: self.base.clone(),
                counter: self.counter.clone(),
                amount: self.amount.clone(),
                price: self.price.clone(),
            }),
            OfferFormAction::Cancel => Ok(OfferRequest::Cancel {
                wallet,
                offer_id: self.offer_id()?,
            }),
        }
    }

    pub(super) fn offer_id(&self) -> Result<i64, String> {
        self.offer_id
            .parse::<i64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| "offer id must be a positive integer".to_owned())
    }
}
