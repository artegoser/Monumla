//! Fleet geometry, validation and randomized constraint solving, shared by setup
//! and hidden-state completion. No search algorithm or opponent secrets here.
use super::*;
pub const FLEET: [usize; 10] = [4, 3, 3, 2, 2, 2, 1, 1, 1, 1];
pub(crate) type Fleet = Vec<(usize, Vec<usize>)>;
pub(crate) fn fleet(s: &GameState, p: PlayerId) -> Fleet {
    let mut ships = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for e in s.entities.values().filter(|e| e.owner == p) {
        if let Some(i) = e.state.get("ship").and_then(StateValue::as_u64) {
            ships
                .entry(i as usize)
                .or_default()
                .push(usize::from(e.position.y) * 10 + usize::from(e.position.x - offset(p)));
        }
    }
    ships.into_iter().collect()
}
pub(crate) fn cells(cell: usize, len: usize, vertical: bool) -> Option<Vec<usize>> {
    if cell >= 100 || (vertical && cell / 10 + len > 10) || (!vertical && cell % 10 + len > 10) {
        return None;
    }
    Some(
        (0..len)
            .map(|i| cell + i * if vertical { 10 } else { 1 })
            .collect(),
    )
}
fn adjacent(a: usize, b: usize) -> bool {
    (a % 10).abs_diff(b % 10) <= 1 && (a / 10).abs_diff(b / 10) <= 1
}
pub(crate) fn can_place(
    s: &GameState,
    p: PlayerId,
    index: usize,
    cell: usize,
    vertical: bool,
) -> bool {
    let Some(&len) = FLEET.get(index) else {
        return false;
    };
    let Some(c) = cells(cell, len, vertical) else {
        return false;
    };
    let ships = fleet(s, p);
    !ships
        .iter()
        .any(|(i, parts)| *i == index || parts.iter().any(|a| c.iter().any(|b| adjacent(*a, *b))))
}
pub fn place_ship(
    s: &mut GameState,
    p: PlayerId,
    index: usize,
    cell: usize,
    vertical: bool,
) -> Result<(), RuleError> {
    if phase(s) != "setup" || ready(s, p) || !can_place(s, p, index, cell, vertical) {
        return Err(rule_error(
            "Ships must fit and cannot touch, even diagonally",
        ));
    }
    install_ship(s, p, index, &cells(cell, FLEET[index], vertical).unwrap())
}
fn install_ship(
    s: &mut GameState,
    p: PlayerId,
    index: usize,
    parts: &[usize],
) -> Result<(), RuleError> {
    for (deck, &cell) in parts.iter().enumerate() {
        let mut e = EntityState::new(
            EntityId::new(p.get() * 1000 + index as u32 * 10 + deck as u32 + 1),
            DECK,
            p,
            Position::new(offset(p) + (cell % 10) as u16, (cell / 10) as u16),
        );
        e.state.insert("ship", index as u64);
        e.state
            .insert("hit", matches!(shot(s, p, cell), "hit" | "sunk"));
        s.add_entity(e)?;
    }
    Ok(())
}
pub(crate) fn install(s: &mut GameState, p: PlayerId, ships: &Fleet) -> Result<(), RuleError> {
    for id in s
        .entities
        .values()
        .filter(|e| e.owner == p)
        .map(|e| e.id)
        .collect::<Vec<_>>()
    {
        s.remove_entity(id)?;
    }
    for (i, c) in ships {
        install_ship(s, p, *i, c)?;
    }
    Ok(())
}
/// Rotate an existing ship around its top/left deck, retaining its identity.
/// Work on a branch so an invalid rotation never partially removes the ship.
pub fn rotate_ship(s: &mut GameState, p: PlayerId, index: usize) -> Result<(), RuleError> {
    let ships = fleet(s, p);
    let parts = &ships
        .iter()
        .find(|(i, _)| *i == index)
        .ok_or_else(|| rule_error("Unknown ship"))?
        .1;
    if parts.len() < 2 {
        return Err(rule_error("A one-deck ship has no orientation"));
    }
    let anchor = *parts.iter().min().unwrap();
    let vertical = parts.iter().all(|c| c / 10 == anchor / 10);
    let mut next = s.clone();
    for id in next
        .entities
        .values()
        .filter(|e| {
            e.owner == p && e.state.get("ship").and_then(StateValue::as_u64) == Some(index as u64)
        })
        .map(|e| e.id)
        .collect::<Vec<_>>()
    {
        next.remove_entity(id)?;
    }
    place_ship(&mut next, p, index, anchor, vertical)?;
    *s = next;
    Ok(())
}
pub fn validate_fleet(s: &GameState, p: PlayerId) -> Result<(), RuleError> {
    let ships = fleet(s, p);
    if ships.len() != 10 || s.entities.values().filter(|e| e.owner == p).count() != 20 {
        return Err(rule_error("Place all ten ships"));
    }
    for (i, c) in &ships {
        if *i >= 10 || c.len() != FLEET[*i] {
            return Err(rule_error("Incorrect fleet composition"));
        }
        let mut sorted = c.clone();
        sorted.sort_unstable();
        if cells(sorted[0], c.len(), false).as_ref() != Some(&sorted)
            && cells(sorted[0], c.len(), true).as_ref() != Some(&sorted)
        {
            return Err(rule_error("A ship must be straight and contiguous"));
        }
        if ships
            .iter()
            .any(|(j, d)| i != j && c.iter().any(|a| d.iter().any(|b| adjacent(*a, *b))))
        {
            return Err(rule_error("Ships cannot touch"));
        }
    }
    Ok(())
}
pub(crate) struct Random(pub u64);
impl Random {
    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}
// Backtracking preserves every public hit/miss/sunk fact. The distribution is a
// randomized feasible-layout sampler, not a claim of uniform posterior sampling.
pub(crate) fn generate(
    s: &GameState,
    p: PlayerId,
    seed: u64,
    knowledge: bool,
) -> Result<Fleet, RuleError> {
    let mut fixed = Vec::new();
    let mut visited = [false; 100];
    let mut unused = (0..10).collect::<Vec<_>>();
    let mut blocked = [false; 100];
    let mut hits = Vec::new();
    if knowledge {
        for i in 0..100 {
            blocked[i] = matches!(shot(s, p, i), "miss" | "water");
            if shot(s, p, i) == "hit" {
                hits.push(i);
            }
            if shot(s, p, i) != "sunk" || visited[i] {
                continue;
            }
            let mut group = vec![i];
            visited[i] = true;
            let mut cursor = 0;
            while cursor < group.len() {
                let a = group[cursor];
                cursor += 1;
                for (b, seen) in visited.iter_mut().enumerate() {
                    if !*seen
                        && shot(s, p, b) == "sunk"
                        && (a % 10).abs_diff(b % 10) + (a / 10).abs_diff(b / 10) == 1
                    {
                        *seen = true;
                        group.push(b);
                    }
                }
            }
            let index = unused
                .iter()
                .position(|i| FLEET[*i] == group.len())
                .ok_or_else(|| rule_error("Inconsistent sunk fleet"))?;
            fixed.push((unused.remove(index), group));
        }
    }
    let mut candidates = Vec::new();
    for &length in &FLEET {
        let mut options = Vec::new();
        for cell in 0..100 {
            for vertical in [false, true] {
                if length == 1 && vertical {
                    continue;
                }
                if let Some(parts) = cells(cell, length, vertical) {
                    if parts
                        .iter()
                        .any(|c| blocked[*c] || (knowledge && shot(s, p, *c) == "sunk"))
                    {
                        continue;
                    }
                    if knowledge && parts.iter().all(|c| shot(s, p, *c) == "hit") {
                        continue;
                    }
                    options.push(parts);
                }
            }
        }
        candidates.push(options);
    }
    struct Solver<'a> {
        candidates: &'a [Vec<Vec<usize>>],
        hits: &'a [usize],
        budget: usize,
        random: Random,
    }
    impl Solver<'_> {
        fn solve(&mut self, left: &[usize], ships: &mut Fleet) -> bool {
            if self.budget == 0 {
                return false;
            }
            self.budget -= 1;
            if left.is_empty() {
                return self
                    .hits
                    .iter()
                    .all(|h| ships.iter().any(|(_, c)| c.contains(h)));
            }
            let uncovered = self
                .hits
                .iter()
                .copied()
                .filter(|h| !ships.iter().any(|(_, c)| c.contains(h)))
                .collect::<Vec<_>>();
            let target = uncovered.first().copied();
            let indices = if let Some(h) = target {
                left.iter()
                    .copied()
                    .filter(|i| self.candidates[*i].iter().any(|c| c.contains(&h)))
                    .collect::<Vec<_>>()
            } else {
                vec![left[0]]
            };
            for index in indices {
                let mut options = self.candidates[index].clone();
                self.random.shuffle(&mut options);
                for parts in options {
                    if target.is_some_and(|h| !parts.contains(&h)) {
                        continue;
                    }
                    if ships
                        .iter()
                        .any(|(_, c)| c.iter().any(|a| parts.iter().any(|b| adjacent(*a, *b))))
                    {
                        continue;
                    }
                    if uncovered
                        .iter()
                        .any(|h| !parts.contains(h) && parts.iter().any(|b| adjacent(*h, *b)))
                    {
                        continue;
                    }
                    ships.push((index, parts));
                    let remaining = left
                        .iter()
                        .copied()
                        .filter(|i| *i != index)
                        .collect::<Vec<_>>();
                    if self.solve(&remaining, ships) {
                        return true;
                    }
                    ships.pop();
                }
            }
            false
        }
    }
    if (Solver {
        candidates: &candidates,
        hits: &hits,
        budget: 50_000,
        random: Random(seed),
    })
    .solve(&unused, &mut fixed)
    {
        Ok(fixed)
    } else {
        Err(rule_error("No fleet completion within the sampling budget"))
    }
}
