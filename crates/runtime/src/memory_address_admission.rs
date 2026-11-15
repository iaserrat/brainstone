use super::Addresses;
use crate::Prompt;

impl Addresses {
    pub(crate) fn admission_prompt(&self, prompt: &Prompt) -> Prompt {
        let mut prompt = prompt.clone();
        let Some(lane_id) = self.find_lane(&prompt.address) else {
            return prompt;
        };
        let lane = self
            .lanes
            .values()
            .find(|lane| lane.id == lane_id)
            .expect("resolved lane exists");
        prompt.system.clone_from(&lane.system);
        prompt
    }
}
