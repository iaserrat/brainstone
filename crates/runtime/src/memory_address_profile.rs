use super::Addresses;
use crate::{Error, LaneId, Prompt};

impl Addresses {
    pub(crate) fn configure(
        &mut self,
        lane_id: LaneId,
        prompt: &Prompt,
        reconfigure: bool,
    ) -> Result<(), Error> {
        let lane = self
            .lanes
            .values_mut()
            .find(|lane| lane.id == lane_id)
            .expect("resolved lane exists");
        if !reconfigure && lane.profile != prompt.profile {
            return Err(Error::LaneConfigurationConflict);
        }
        lane.profile.clone_from(&prompt.profile);
        Ok(())
    }
}
