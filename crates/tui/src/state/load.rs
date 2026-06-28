use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LoadSlot<K> {
    generation: u64,
    loading: bool,
    key: Option<K>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ActiveLoad<K> {
    pub generation: u64,
    pub key: K,
}

impl<K> LoadSlot<K> {
    pub(super) fn new() -> Self {
        Self {
            generation: 0,
            loading: false,
            key: None,
        }
    }

    pub(super) fn start(&mut self, generation: u64, key: K) {
        self.generation = generation;
        self.loading = true;
        self.key = Some(key);
    }

    pub fn loading(&self) -> bool {
        self.loading
    }

    pub(super) fn finish(&mut self, generation: u64) -> Option<ActiveLoad<K>> {
        if !self.loading || generation != self.generation {
            return None;
        }
        let active = ActiveLoad {
            generation: self.generation,
            key: self.key.take()?,
        };
        self.loading = false;
        Some(active)
    }

    pub(super) fn cancel(&mut self) -> Option<ActiveLoad<K>> {
        if !self.loading {
            return None;
        }
        let active = ActiveLoad {
            generation: self.generation,
            key: self.key.take()?,
        };
        self.loading = false;
        Some(active)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedSymbolLoad<T> {
    snapshot: Option<T>,
    request: LoadSlot<String>,
}

impl<T> SelectedSymbolLoad<T> {
    pub(super) fn new() -> Self {
        Self {
            snapshot: None,
            request: LoadSlot::new(),
        }
    }

    pub fn loading(&self) -> bool {
        self.request.loading()
    }

    pub(super) fn start(&mut self, generation: u64, symbol: String) {
        self.request.start(generation, symbol);
    }

    pub(super) fn finish(&mut self, generation: u64) -> Option<ActiveLoad<String>> {
        self.request.finish(generation)
    }

    pub(super) fn set_snapshot(&mut self, snapshot: T) {
        self.snapshot = Some(snapshot);
    }

    pub(super) fn cancel(&mut self) -> Option<ActiveLoad<String>> {
        self.request.cancel()
    }

    pub(super) fn reset(&mut self) -> Option<ActiveLoad<String>> {
        self.snapshot = None;
        self.request.cancel()
    }
}

pub trait SymbolSnapshot {
    fn requested_symbol(&self) -> &str;
    fn symbol(&self) -> &str;
}

impl SymbolSnapshot for HistorySnapshot {
    fn requested_symbol(&self) -> &str {
        &self.requested_symbol
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

impl SymbolSnapshot for CryptoQuoteEvidenceSnapshot {
    fn requested_symbol(&self) -> &str {
        &self.requested_symbol
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

impl SymbolSnapshot for ResearchContextSnapshot {
    fn requested_symbol(&self) -> &str {
        &self.requested_symbol
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SelectedDataState {
    Fresh,
    Stale,
    Empty,
}

impl<T: SymbolSnapshot> SelectedSymbolLoad<T> {
    pub fn has_selected_snapshot(&self, selected: &str) -> bool {
        self.selected_snapshot(selected).is_some()
    }

    pub fn selected_data_state(&self, selected: &str) -> SelectedDataState {
        if self.selected_snapshot(selected).is_some() {
            SelectedDataState::Fresh
        } else if self.snapshot.is_some() {
            SelectedDataState::Stale
        } else {
            SelectedDataState::Empty
        }
    }

    pub fn selected_snapshot(&self, selected: &str) -> Option<&T> {
        self.snapshot.as_ref().filter(|snapshot| {
            snapshot.requested_symbol() == selected || snapshot.symbol() == selected
        })
    }
}
