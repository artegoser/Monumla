//! Conditional fleet sampling. Pairwise Gibbs updates choose uniformly among
//! all legal replacements for two ships, including changing which ship length
//! explains a hit. The stationary distribution is uniform over legal labeled
//! fleets; finite chains can remain correlated and are not exact draws.
use super::*;
use crate::fleet::{cells, generate, Fleet, Random};
use std::cell::RefCell;
#[derive(Clone)]
struct Placement {
    cells: Vec<usize>,
    mask: u128,
    halo: u128,
}
impl Placement {
    fn new(cells: Vec<usize>) -> Self {
        let mask = cells.iter().fold(0, |m, c| m | (1_u128 << c));
        let mut halo = 0;
        for &c in &cells {
            for y in (c / 10).saturating_sub(1)..=(c / 10 + 1).min(9) {
                for x in (c % 10).saturating_sub(1)..=(c % 10 + 1).min(9) {
                    halo |= 1_u128 << (y * 10 + x);
                }
            }
        }
        Self { cells, mask, halo }
    }
}
struct Chain {
    key: u128,
    seed: u64,
    candidates: Vec<Vec<Placement>>,
    current: Vec<Placement>,
    movable: Vec<usize>,
    hits: u128,
}
#[derive(Default)]
pub(crate) struct FleetSampler(RefCell<Option<Chain>>);
impl FleetSampler {
    pub fn sample(&self, s: &GameState, p: PlayerId, seed: u64) -> Result<Fleet, RuleError> {
        let key = s.semantic_digest();
        let mut cache = self.0.borrow_mut();
        if cache
            .as_ref()
            .map_or(true, |c| c.key != key || c.seed.wrapping_add(1) != seed)
        {
            let initial = generate(s, p, seed, true)?;
            let mut current = (0..10)
                .map(|i| Placement::new(initial.iter().find(|(j, _)| *j == i).unwrap().1.clone()))
                .collect::<Vec<_>>();
            let mut forbidden = 0;
            let mut hits = 0;
            for c in 0..100 {
                match shot(s, p, c) {
                    "miss" | "water" | "sunk" => forbidden |= 1_u128 << c,
                    "hit" => hits |= 1_u128 << c,
                    _ => {}
                }
            }
            let movable = current
                .iter()
                .enumerate()
                .filter(|(_, v)| v.cells.iter().any(|c| shot(s, p, *c) != "sunk"))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let mut candidates = Vec::new();
            for (i, &length) in FLEET.iter().enumerate() {
                let mut list = Vec::new();
                if movable.contains(&i) {
                    for cell in 0..100 {
                        for vertical in [false, true] {
                            if length == 1 && vertical {
                                continue;
                            }
                            if let Some(cells) = cells(cell, length, vertical) {
                                let v = Placement::new(cells);
                                if v.mask & forbidden == 0 && v.mask & !hits != 0 {
                                    list.push(v);
                                }
                            }
                        }
                    }
                } else {
                    list.push(current[i].clone());
                }
                current[i].cells.sort_unstable();
                candidates.push(list);
            }
            let mut chain = Chain {
                key,
                seed,
                candidates,
                current,
                movable,
                hits,
            };
            chain.advance(128, &mut Random(seed.wrapping_mul(0x9e3779b97f4a7c15)));
            *cache = Some(chain);
        } else {
            let chain = cache.as_mut().unwrap();
            chain.advance(16, &mut Random(seed.wrapping_mul(0x9e3779b97f4a7c15)));
            chain.seed = seed;
        }
        Ok(cache
            .as_ref()
            .unwrap()
            .current
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.cells.clone()))
            .collect())
    }
}
impl Chain {
    fn advance(&mut self, steps: usize, random: &mut Random) {
        if self.movable.is_empty() {
            return;
        }
        for _ in 0..steps {
            let a = self.movable[random.next() as usize % self.movable.len()];
            let b = if self.movable.len() > 1 {
                let mut b = a;
                while b == a {
                    b = self.movable[random.next() as usize % self.movable.len()];
                }
                Some(b)
            } else {
                None
            };
            let mut occupied = 0;
            let mut halo = 0;
            for (i, p) in self.current.iter().enumerate() {
                if i != a && Some(i) != b {
                    occupied |= p.mask;
                    halo |= p.halo;
                }
            }
            let needed = self.hits & !occupied;
            let left = self.candidates[a]
                .iter()
                .enumerate()
                .filter(|(_, p)| p.mask & halo == 0)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let right = b
                .map(|b| {
                    self.candidates[b]
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p.mask & halo == 0)
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![0]);
            let valid = |x: usize, y: usize| {
                let p = &self.candidates[a][x];
                if let Some(b) = b {
                    let q = &self.candidates[b][y];
                    p.halo & q.mask == 0 && needed & !(p.mask | q.mask) == 0
                } else {
                    needed & !p.mask == 0
                }
            };
            let count = left
                .iter()
                .map(|x| right.iter().filter(|y| valid(*x, **y)).count())
                .sum::<usize>();
            if count == 0 {
                continue;
            } // Current legal pair should always witness one replacement.
            let mut pick = random.next() as usize % count;
            'chosen: for &x in &left {
                for &y in &right {
                    if valid(x, y) {
                        if pick == 0 {
                            let p = self.candidates[a][x].clone();
                            let q = b.map(|b| self.candidates[b][y].clone());
                            self.current[a] = p;
                            if let (Some(b), Some(q)) = (b, q) {
                                self.current[b] = q;
                            }
                            break 'chosen;
                        }
                        pick -= 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_ship_conditional_draws_match_exhaustive_legal_pairs() {
        let a = vec![Placement::new(vec![0, 1]), Placement::new(vec![40, 41])];
        let b = vec![Placement::new(vec![10, 11]), Placement::new(vec![70, 71])];
        let mut c = Chain {
            key: 0,
            seed: 0,
            current: vec![a[0].clone(), b[1].clone()],
            candidates: vec![a, b],
            movable: vec![0, 1],
            hits: 0,
        };
        let mut counts = std::collections::BTreeMap::new();
        let mut random = Random(99);
        for _ in 0..900 {
            c.advance(1, &mut random);
            *counts
                .entry((c.current[0].mask, c.current[1].mask))
                .or_insert(0) += 1;
        }
        assert_eq!(counts.len(), 3);
        for count in counts.values() {
            assert!((240..360).contains(count));
        }
        c.hits = 1;
        for _ in 0..50 {
            c.advance(1, &mut random);
            assert_eq!(c.current[0].cells, vec![0, 1]);
            assert_eq!(c.current[1].cells, vec![70, 71]);
        }
    }
}
