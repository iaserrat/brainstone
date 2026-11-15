use super::Addresses;
use crate::{Error, LaneId};

impl Addresses {
    /// Rename lane `id`, named `provisional` in its session, to `external_id`.
    /// `false` when another lane of the session already holds that name.
    pub(crate) fn rename(
        &mut self,
        id: LaneId,
        provisional: &str,
        external_id: &str,
    ) -> Result<bool, Error> {
        let (key, _) = self
            .lanes
            .iter()
            .find(|(_, lane)| lane.id == id)
            .ok_or(Error::LeaseLost)?;
        let session = key.0;
        if key.1 == external_id {
            return Ok(true);
        }
        if key.1 != provisional {
            return Err(Error::InvalidInput { field: "lane" });
        }
        if self.lanes.contains_key(&(session, external_id.to_owned())) {
            return Ok(false);
        }
        let lane = self
            .lanes
            .remove(&(session, provisional.to_owned()))
            .expect("lane found above");
        self.lanes.insert((session, external_id.to_owned()), lane);
        Ok(true)
    }
}
