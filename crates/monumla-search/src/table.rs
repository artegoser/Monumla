#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone)]
pub(crate) struct Entry<A> {
    pub key: u128,
    pub depth: u16,
    pub score: i32,
    pub bound: Bound,
    pub line: Vec<A>,
    pub generation: u16,
}

pub(crate) struct Table<A> {
    slots: Vec<Option<Entry<A>>>,
}
impl<A: Clone> Table<A> {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: std::iter::repeat_with(|| None).take(capacity).collect(),
        }
    }
    fn index(&self, key: u128) -> usize {
        ((key as u64 ^ (key >> 64) as u64) as usize) % self.slots.len()
    }
    pub fn get(&self, key: u128) -> Option<&Entry<A>> {
        if self.slots.is_empty() {
            return None;
        }
        self.slots[self.index(key)]
            .as_ref()
            .filter(|entry| entry.key == key)
    }
    pub fn store(&mut self, entry: Entry<A>) {
        if self.slots.is_empty() {
            return;
        }
        let index = self.index(entry.key);
        let replace = self.slots[index].as_ref().map_or(true, |old| {
            old.generation != entry.generation
                || entry.depth >= old.depth
                || (old.key == entry.key && entry.bound == Bound::Exact)
        });
        if replace {
            self.slots[index] = Some(entry);
        }
    }
}
