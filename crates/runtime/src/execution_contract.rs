//! Accepted restrictions intersect with current executor authority.
use super::Execution;
use crate::{Error, Store};
use agent::{Model, Outcome};

const CODE_CONTRACT_DENIED: &str = "tool_contract_denied";
const CODE_CONTRACT_UNAVAILABLE: &str = "tool_contract_unavailable";

impl<S: Store, M: Model> Execution<'_, S, M> {
    pub(super) fn call_denied(&self, name: &str) -> Option<&'static str> {
        if self
            .claimed
            .thread
            .profile()
            .tool_contract()
            .is_some_and(|contract| !contract.allows(name))
        {
            return Some(CODE_CONTRACT_DENIED);
        }
        self.policy
            .filter(|policy| policy.denied(name))
            .map(|_| "tool_policy_denied")
    }

    pub(super) fn approval_binding(&self, name: &str) -> Result<Option<String>, Error> {
        let contract = self.claimed.thread.profile().tool_contract();
        if contract.is_some_and(|contract| contract.approval_required(name)) {
            return serde_json::to_string(&(contract, self.policy.map(|policy| &policy.binding)))
                .map(Some)
                .map_err(Error::corrupt);
        }
        Ok(self
            .policy
            .filter(|policy| policy.required(name))
            .map(|policy| policy.binding.clone()))
    }

    pub(super) fn unavailable_tools(&self) -> Option<Outcome> {
        let contract = self.claimed.thread.profile().tool_contract()?;
        let names = contract
            .allowed_tools()
            .into_iter()
            .flatten()
            .chain(contract.required_tools())
            .chain(contract.approval_tools());
        let missing: Vec<&str> = names
            .filter(|name| self.tools.lookup(name).is_none())
            .map(String::as_str)
            .collect();
        if missing.is_empty() {
            return None;
        }
        Some(Outcome::Failed {
            code: CODE_CONTRACT_UNAVAILABLE.into(),
            message: format!("accepted tools unavailable: {}", missing.join(", ")),
        })
    }

    pub(super) async fn settle_contract(&mut self, outcome: &Outcome) -> Result<(), Error> {
        // Steering cannot make a missing capability executable in this claim.
        while !self.settle(outcome).await? {}
        Ok(())
    }
}
