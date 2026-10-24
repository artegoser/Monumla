//! Reuse group/liberty facts while generating legal placements. This is the
//! rules' legality check, shared by every caller, without search heuristics.
use super::*;
struct GroupFacts {
    positions: Vec<Position>,
    liberties: BTreeSet<Position>,
}
pub(super) struct PlacementFacts {
    key: BoardKey,
    groups: Vec<GroupFacts>,
    membership: std::collections::BTreeMap<EntityId, usize>,
}
impl PlacementFacts {
    pub fn new(s: &GameState) -> Result<Self, InteractionError> {
        let key = board_key(s)?;
        let mut groups = Vec::new();
        let mut membership = std::collections::BTreeMap::new();
        for e in s.entities.values() {
            if membership.contains_key(&e.id) {
                continue;
            }
            let members = group(s, e.id)?;
            let index = groups.len();
            let mut positions = Vec::new();
            let mut liberties = BTreeSet::new();
            for id in members {
                membership.insert(id, index);
                let p = s.entity(id)?.position;
                positions.push(p);
                for n in neighbors(s, p) {
                    if s.entity_at(n)?.is_none() {
                        liberties.insert(n);
                    }
                }
            }
            groups.push(GroupFacts {
                positions,
                liberties,
            });
        }
        Ok(Self {
            key,
            groups,
            membership,
        })
    }
    pub fn legal(
        &self,
        s: &GameState,
        seen: &BTreeSet<(PlayerId, BoardKey)>,
        player: PlayerId,
        p: Position,
    ) -> Result<bool, InteractionError> {
        if phase(s) != PHASE_PLAY || s.entity_at(p)?.is_some() {
            return Ok(false);
        }
        let enemy = opponent(player)?;
        let mut liberty = false;
        let mut connected = false;
        let mut captured = Vec::new();
        for n in neighbors(s, p) {
            match s.entity_at(n)? {
                None => liberty = true,
                Some(e) => {
                    let facts = &self.groups[self.membership[&e.id]];
                    if e.owner == player && facts.liberties.len() > 1 {
                        connected = true;
                    }
                    if e.owner == enemy
                        && facts.liberties.len() == 1
                        && facts.liberties.contains(&p)
                    {
                        captured.push(facts);
                    }
                }
            }
        }
        if !liberty && !connected && captured.is_empty() {
            return Ok(false);
        }
        let mut after = self.key.clone();
        let width = usize::from(s.board.width());
        after.0[usize::from(p.y) * width + usize::from(p.x)] = if player == BLACK { 1 } else { 2 };
        for facts in captured {
            for p in &facts.positions {
                after.0[usize::from(p.y) * width + usize::from(p.x)] = 0;
            }
        }
        Ok(!seen.contains(&(enemy, after)))
    }
}
