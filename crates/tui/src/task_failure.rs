#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskFailure {
    pub source: TaskFailureSource,
    pub symbol: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone, Default)]
pub struct TaskFailures {
    entries: Vec<TaskFailure>,
}

impl TaskFailures {
    pub fn iter(&self) -> impl Iterator<Item = &TaskFailure> {
        self.entries.iter()
    }

    pub fn set(&mut self, failure: TaskFailure) {
        self.clear(failure.source, failure.symbol.as_deref());
        self.entries.push(failure);
    }

    pub fn clear(&mut self, source: TaskFailureSource, symbol: Option<&str>) {
        self.entries
            .retain(|failure| failure.source != source || failure.symbol.as_deref() != symbol);
    }

    pub fn clear_symbol(
        &mut self,
        source: TaskFailureSource,
        requested_symbol: &str,
        symbol: &str,
    ) {
        self.entries.retain(|failure| {
            failure.source != source
                || !matches!(
                    failure.symbol.as_deref(),
                    Some(value) if value == requested_symbol || value == symbol
                )
        });
    }
}

impl TaskFailure {
    pub fn market(error: String) -> Self {
        Self {
            source: TaskFailureSource::Quotes,
            symbol: None,
            error,
        }
    }

    pub fn history(symbol: String, error: String) -> Self {
        Self {
            source: TaskFailureSource::History,
            symbol: Some(symbol),
            error,
        }
    }

    pub fn evidence(symbol: String, error: String) -> Self {
        Self {
            source: TaskFailureSource::CryptoEvidence,
            symbol: Some(symbol),
            error,
        }
    }

    pub fn scheduler(error: String) -> Self {
        Self {
            source: TaskFailureSource::Scheduler,
            symbol: None,
            error,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskFailureSource {
    Quotes,
    History,
    CryptoEvidence,
    Scheduler,
}
