use std::collections::VecDeque;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct TaskLog {
    entries: VecDeque<TaskLogEntry>,
}

impl TaskLog {
    const MAX_ENTRIES: usize = 200;

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &TaskLogEntry> {
        self.entries.iter()
    }

    pub fn running(&mut self, key: TaskKey, message: impl Into<String>) {
        self.upsert(key, TaskStatus::Running, message);
    }

    pub fn succeeded(&mut self, key: TaskKey, message: impl Into<String>) {
        self.upsert(key, TaskStatus::Succeeded, message);
    }

    pub fn warning(&mut self, key: TaskKey, message: impl Into<String>) {
        self.upsert(key, TaskStatus::Warning, message);
    }

    pub fn failed(&mut self, key: TaskKey, message: impl Into<String>) {
        self.upsert(key, TaskStatus::Failed, message);
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.push(TaskLogEntry {
            key: None,
            status: TaskStatus::Info,
            message: message.into(),
        });
    }

    pub fn warning_event(&mut self, message: impl Into<String>) {
        self.push(TaskLogEntry {
            key: None,
            status: TaskStatus::Warning,
            message: message.into(),
        });
    }

    fn upsert(&mut self, key: TaskKey, status: TaskStatus, message: impl Into<String>) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.key.as_ref() == Some(&key))
        {
            entry.status = status;
            entry.message = message.into();
            return;
        }

        self.push(TaskLogEntry {
            key: Some(key),
            status,
            message: message.into(),
        });
    }

    fn push(&mut self, entry: TaskLogEntry) {
        self.entries.push_back(entry);
        while self.entries.len() > Self::MAX_ENTRIES {
            self.entries.pop_front();
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskLogEntry {
    pub key: Option<TaskKey>,
    pub status: TaskStatus,
    pub message: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TaskKey {
    Refresh { generation: u64 },
    History { generation: u64, symbol: String },
    Evidence { generation: u64, symbol: String },
    Research { generation: u64, symbol: String },
    Account { generation: u64, profile: String },
    Scheduler,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskStatus {
    Info,
    Running,
    Succeeded,
    Warning,
    Failed,
}

impl TaskStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Warning => "warning",
            Self::Failed => "failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_status_replaces_running_entry_for_same_task() {
        let mut log = TaskLog::default();
        let key = TaskKey::History {
            generation: 7,
            symbol: "CRDO".to_string(),
        };

        log.running(key.clone(), "CRDO history loading");
        log.succeeded(key, "CRDO history loaded");

        let entries = log.iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, TaskStatus::Succeeded);
        assert_eq!(entries[0].message, "CRDO history loaded");
    }
}
