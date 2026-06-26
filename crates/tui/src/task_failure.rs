#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskFailure {
    pub source: TaskFailureSource,
    pub scope: TaskFailureScope,
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

    pub fn has_source(&self, source: TaskFailureSource) -> bool {
        self.entries.iter().any(|failure| failure.source == source)
    }

    pub fn set(&mut self, failure: TaskFailure) {
        self.entries.retain(|existing| {
            existing.source != failure.source || existing.scope != failure.scope
        });
        self.entries.push(failure);
    }

    pub fn clear_global(&mut self, source: TaskFailureSource) {
        self.entries.retain(|failure| {
            failure.source != source || failure.scope != TaskFailureScope::Global
        });
    }

    pub fn clear_profile(&mut self, source: TaskFailureSource, profile: &str) {
        self.entries.retain(|failure| {
            failure.source != source
                || failure.scope != TaskFailureScope::Profile(profile.to_string())
        });
    }

    pub fn clear_symbol(
        &mut self,
        source: TaskFailureSource,
        requested_symbol: &str,
        symbol: &str,
    ) {
        self.entries.retain(|failure| {
            failure.source != source
                || !failure
                    .scope
                    .matches_either_symbol(requested_symbol, symbol)
        });
    }
}

impl TaskFailure {
    pub fn market(error: String) -> Self {
        Self {
            source: TaskFailureSource::Quotes,
            scope: TaskFailureScope::Global,
            error,
        }
    }

    pub fn history(symbol: String, error: String) -> Self {
        Self {
            source: TaskFailureSource::History,
            scope: TaskFailureScope::Symbol(symbol),
            error,
        }
    }

    pub fn evidence(symbol: String, error: String) -> Self {
        Self {
            source: TaskFailureSource::CryptoEvidence,
            scope: TaskFailureScope::Symbol(symbol),
            error,
        }
    }

    pub fn scheduler(error: String) -> Self {
        Self {
            source: TaskFailureSource::Scheduler,
            scope: TaskFailureScope::Global,
            error,
        }
    }

    pub fn account(profile: String, error: String) -> Self {
        Self {
            source: TaskFailureSource::Account,
            scope: TaskFailureScope::Profile(profile),
            error,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TaskFailureScope {
    Global,
    Symbol(String),
    Profile(String),
}

impl TaskFailureScope {
    fn matches_either_symbol(&self, requested_symbol: &str, symbol: &str) -> bool {
        matches!(self, Self::Symbol(value) if value == requested_symbol || value == symbol)
    }

    pub fn selected_symbol_matches(&self, selected_symbol: Option<&str>) -> bool {
        match self {
            Self::Symbol(symbol) => selected_symbol == Some(symbol.as_str()),
            Self::Global | Self::Profile(_) => true,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskFailureSource {
    Quotes,
    History,
    CryptoEvidence,
    Account,
    Scheduler,
}
