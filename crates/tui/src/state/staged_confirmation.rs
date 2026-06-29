use crate::state::{StagedExecutionRequest, TypedConfirmation};

#[derive(Debug, Clone)]
pub(super) struct PendingStagedConfirmation {
    request: StagedExecutionRequest,
    gate: ConfirmationGate,
}

#[derive(Debug, Clone)]
enum ConfirmationGate {
    Open,
    Typed {
        policy: TypedConfirmation,
        input: String,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TypedConfirmationGateView<'a> {
    pub phrase: &'static str,
    pub reason: &'static str,
    pub input: &'a str,
    pub matched: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingStagedConfirmationView<'a> {
    pub request: &'a StagedExecutionRequest,
    pub typed_gate: Option<TypedConfirmationGateView<'a>>,
    pub can_confirm: bool,
}

impl PendingStagedConfirmation {
    pub(super) fn new(request: StagedExecutionRequest) -> Self {
        let gate = request
            .typed_confirmation()
            .map(|policy| ConfirmationGate::Typed {
                policy,
                input: String::new(),
            })
            .unwrap_or(ConfirmationGate::Open);
        Self { request, gate }
    }

    pub(super) fn request(&self) -> &StagedExecutionRequest {
        &self.request
    }

    pub(super) fn view(&self) -> PendingStagedConfirmationView<'_> {
        PendingStagedConfirmationView {
            request: &self.request,
            typed_gate: self.typed_gate(),
            can_confirm: self.can_confirm(),
        }
    }

    pub(super) fn into_request(self) -> StagedExecutionRequest {
        self.request
    }

    pub(super) fn accepts_text_input(&self) -> bool {
        matches!(self.gate, ConfirmationGate::Typed { .. })
    }

    pub(super) fn can_confirm(&self) -> bool {
        match &self.gate {
            ConfirmationGate::Open => true,
            ConfirmationGate::Typed { policy, input } => policy.satisfied_by(input),
        }
    }

    pub(super) fn typed_gate(&self) -> Option<TypedConfirmationGateView<'_>> {
        match &self.gate {
            ConfirmationGate::Open => None,
            ConfirmationGate::Typed { policy, input } => Some(TypedConfirmationGateView {
                phrase: policy.phrase,
                reason: policy.reason,
                input,
                matched: policy.satisfied_by(input),
            }),
        }
    }

    pub(super) fn edit(&mut self, request: tui_input::InputRequest) {
        let ConfirmationGate::Typed { input, .. } = &mut self.gate else {
            return;
        };
        let mut editor = tui_input::Input::new(input.clone());
        editor.handle(request);
        *input = editor.to_string();
    }

    pub(super) fn missing_confirmation_message(&self) -> Option<String> {
        let gate = self.typed_gate()?;
        (!gate.matched).then(|| {
            format!(
                "type {} exactly to confirm {}",
                gate.phrase,
                self.request.kind_label()
            )
        })
    }
}
