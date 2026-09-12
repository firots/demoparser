//! Opt-in world actor identities. Steam IDs remain account IDs; bots use a
//! demo-local controller handle (including its serial), independent of takeover.
use super::collect_data::PropCollectionError;
use super::entities::PlayerMetaData;
use super::parser_settings::SecondPassParser;
use super::variants::Variant;

impl<'a> SecondPassParser<'a> {
    pub fn world_actors_enabled(&self) -> bool {
        self.prop_controller.world_actors
    }

    pub fn controller_value(&self, player: &PlayerMetaData, name: &str) -> Option<Variant> {
        let id = self.prop_controller.name_to_id.get(name)?;
        self.get_controller_prop(id, player).ok()
    }

    pub fn actor_id(&self, player: &PlayerMetaData) -> Result<String, PropCollectionError> {
        if let Some(sid) = player.steamid.filter(|sid| *sid != 0) {
            return Ok(sid.to_string());
        }
        let controller = player.controller_entid
            .and_then(|id| self.entities.get(id as usize))
            .and_then(|ent| ent.as_ref())
            .ok_or(PropCollectionError::ControllerEntityIdNotSet)?;
        Ok(format!("bot:{}", controller.serial << 14 | controller.entity_id as u32))
    }

    /// Resolve the full handle, never a recycled entity slot alone.
    pub fn actor_pawn_from_handle(&self, handle: u32) -> Option<i32> {
        let id = (handle & 0x3fff) as i32;
        let entity = self.entities.get(id as usize)?.as_ref()?;
        (id != 2047 && entity.serial == handle >> 14 && self.players.contains_key(&id))
            .then_some(id)
    }

    /// Game-event pawn references use a different upper-bit encoding from
    /// network handles. Validate the slot against the controller's full handle.
    pub fn actor_from_event_pawn(&self, reference: u32, userid: Option<i32>) -> Option<i32> {
        let slot = (reference & 0x7ff) as i32;
        if let Some(resolved) = userid.and_then(|id| self.entity_id_from_userid(id)) {
            if resolved == slot { return Some(resolved); }
            // Lingering inferno damage may have no usable pawn reference after
            // the attacker dies. Preserve the ordinary own-account identity;
            // a takeover still requires the event to identify that bot pawn.
            return self.players.get(&resolved).filter(|p| self.actor_is_own_pawn(p))
                .map(|_| resolved);
        }
        let player = self.players.get(&slot)?;
        // Unknown account userid must never fall back to another human.
        if userid.is_some() && player.steamid != Some(0) { return None; }
        match self.controller_value(player, "CCSPlayerController.m_hPlayerPawn") {
            Some(Variant::U32(handle)) => self.actor_pawn_from_handle(handle).filter(|id| *id == slot),
            _ => None,
        }
    }

    pub fn current_actor_pawn(&self, player: &PlayerMetaData) -> Option<i32> {
        let current = match self.controller_value(player, "CCSPlayerController.m_hPawn") {
            Some(Variant::U32(handle)) => self.actor_pawn_from_handle(handle),
            _ => None,
        };
        if matches!(self.controller_value(player, "CCSPlayerController.m_bControllingBot"), Some(Variant::Bool(true))) {
            return current;
        }
        if current.and_then(|id| self.players.get(&id)).is_some_and(|p| p.steamid == Some(0)) {
            return current;
        }
        // Dead humans can possess a spectator pawn while their original pawn
        // still owns prior combat (notably assists). Spectator state must not
        // remove that account's identity or replace its playing pawn.
        player.player_entity_id
    }

    pub fn actor_is_own_pawn(&self, player: &PlayerMetaData) -> bool {
        if player.steamid.unwrap_or(0) == 0
            || matches!(self.controller_value(player, "CCSPlayerController.m_bControllingBot"), Some(Variant::Bool(true))) {
            return false;
        }
        if let Some(Variant::U32(handle)) = self.controller_value(player, "CCSPlayerController.m_hPlayerPawn") {
            if let Some(Some(entity)) = self.entities.get((handle & 0x3fff) as usize) {
                if entity.serial != handle >> 14 { return false; }
            }
        }
        // Also reject a confirmed different actor if a demo lacks the takeover flag.
        !self.current_actor_pawn(player).and_then(|id| self.players.get(&id))
            .is_some_and(|current| current.steamid == Some(0))
    }

    pub fn actor_controller_steamid(&self, entity_id: i32) -> Option<u64> {
        self.players.values().find_map(|player| {
            let sid = player.steamid.filter(|sid| *sid != 0)?;
            (self.current_actor_pawn(player) == Some(entity_id)).then_some(sid)
        })
    }

    pub fn actor_spotted_by(&self, entity_id: &i32) -> Result<Variant, PropCollectionError> {
        let prop = self.prop_controller.name_to_id.get("CCSPlayerPawn.m_bSpottedByMask")
            .ok_or(PropCollectionError::GetPropFromEntPropNotFound)?;
        let Variant::U32(mask) = self.get_prop_from_ent(prop, entity_id)? else {
            return Err(PropCollectionError::SpottedIncorrectVariant);
        };
        let mut actors = Vec::new();
        for bit in 0..32 {
            if mask & (1 << bit) == 0 { continue; }
            let Some(controller) = self.find_user_by_controller_id(bit + 1) else { continue; };
            let pawn = if matches!(self.controller_value(controller, "CCSPlayerController.m_bControllingBot"), Some(Variant::Bool(true))) {
                self.current_actor_pawn(controller)
            } else { controller.player_entity_id };
            let Some(pawn) = pawn else { continue; };
            if let Some(actor) = self.players.get(&pawn).and_then(|p| self.actor_id(p).ok()) {
                actors.push(actor);
            }
        }
        Ok(Variant::StringVec(actors))
    }
}
