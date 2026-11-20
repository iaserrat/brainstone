use std::collections::HashMap;

use crate::{Error, LaneId, Prompt, SessionId};

#[path = "memory_address_admission.rs"]
mod admission;
#[path = "memory_address_bind.rs"]
mod bind;
#[path = "memory_address_profile.rs"]
mod profile;

type SessionKey = (String, String);
type LaneKey = (SessionId, String);

#[derive(Debug)]
struct Lane {
    id: LaneId,
    system: String,
    profile: agent::ExecutionProfile,
    identity: Option<agent::ExecutionIdentity>,
}

#[derive(Debug, Default)]
pub(crate) struct Addresses {
    next_session_id: i64,
    next_lane_id: i64,
    sessions: HashMap<SessionKey, SessionId>,
    lanes: HashMap<LaneKey, Lane>,
}

impl Addresses {
    pub(crate) fn resolve(&mut self, prompt: &Prompt) -> Result<(SessionId, LaneId), Error> {
        let session_id = self.session_id(prompt);
        let lane_id = self.lane_id(session_id, prompt)?;
        Ok((session_id, lane_id))
    }

    pub(crate) fn find_lane(&self, address: &crate::Address) -> Option<LaneId> {
        let session = (address.surface.clone(), address.conversation.clone());
        let session_id = self.sessions.get(&session)?;
        self.lanes
            .get(&(*session_id, address.lane.clone()))
            .map(|lane| lane.id)
    }

    fn session_id(&mut self, prompt: &Prompt) -> SessionId {
        let key = (
            prompt.address.surface.clone(),
            prompt.address.conversation.clone(),
        );
        if let Some(id) = self.sessions.get(&key) {
            return *id;
        }
        self.next_session_id += 1;
        let id = SessionId(self.next_session_id);
        self.sessions.insert(key, id);
        id
    }

    fn lane_id(&mut self, session_id: SessionId, prompt: &Prompt) -> Result<LaneId, Error> {
        let key = (session_id, prompt.address.lane.clone());
        if let Some(lane) = self.lanes.get(&key) {
            crate::identity_boundary::check(lane.identity.as_ref(), prompt.identity.as_ref())?;
            if lane.system != prompt.system {
                return Err(Error::LaneConfigurationConflict);
            }
            return Ok(lane.id);
        }
        self.next_lane_id += 1;
        let id = LaneId(self.next_lane_id);
        let lane = Lane {
            id,
            system: prompt.system.clone(),
            profile: prompt.profile.clone(),
            identity: prompt.identity.clone(),
        };
        self.lanes.insert(key, lane);
        Ok(id)
    }
}
