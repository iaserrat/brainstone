use super::Addresses;
use crate::{Error, LaneId, Prompt};

impl Addresses {
    pub(crate) fn configure(
        &mut self,
        lane_id: LaneId,
        prompt: &Prompt,
        reconfigure: bool,
    ) -> Result<agent::ExecutionProfile, Error> {
        let lane = self
            .lanes
            .values_mut()
            .find(|lane| lane.id == lane_id)
            .expect("resolved lane exists");
        if !reconfigure && !lane.profile.same_settings(&prompt.profile) {
            return Err(Error::LaneConfigurationConflict);
        }
        if reconfigure {
            lane.profile.clone_from(&prompt.profile);
        }
        Ok(lane.profile.clone())
    }
}
