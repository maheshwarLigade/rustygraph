use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Slot<T> {
    generation: u32,
    data: Option<T>,
}

/// Generational arena providing cache-friendly O(1) insertions, lookups, and deletions
/// with protection against ABA/stale handle access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotArena<T> {
    slots: Vec<Slot<T>>,
    free_list: Vec<u32>,
    active_count: usize,
}

impl<T> Default for SlotArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SlotArena<T> {
    /// Create a new, empty SlotArena.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            active_count: 0,
        }
    }

    /// Create a new SlotArena with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free_list: Vec::new(),
            active_count: 0,
        }
    }

    /// Insert an item into the arena, returning its unique 64-bit ID.
    pub fn insert_with<F>(&mut self, f: F) -> (u64, &T)
    where
        F: FnOnce(u64) -> T,
    {
        let (id, idx) = if let Some(free_idx) = self.free_list.pop() {
            let gen_val = self.slots[free_idx as usize].generation;
            let id = ((gen_val as u64) << 32) | (free_idx as u64);
            let item = f(id);
            self.slots[free_idx as usize].data = Some(item);
            (id, free_idx as usize)
        } else {
            let idx = self.slots.len() as u32;
            let gen_val = 1u32;
            let id = ((gen_val as u64) << 32) | (idx as u64);
            let item = f(id);
            self.slots.push(Slot {
                generation: gen_val,
                data: Some(item),
            });
            (id, idx as usize)
        };

        self.active_count += 1;
        (id, self.slots[idx].data.as_ref().unwrap())
    }

    /// Insert a pre-existing item into the arena with a newly allocated ID.
    pub fn insert(&mut self, item: T) -> u64 {
        let (id, _) = self.insert_with(|_| item);
        id
    }

    /// Insert an item with an exact required ID (useful for distributed replication and state recovery).
    pub fn insert_exact(&mut self, id: u64, item: T) {
        let idx = (id & 0xFFFF_FFFF) as usize;
        let gen_val = (id >> 32) as u32;

        if idx >= self.slots.len() {
            self.slots.resize_with(idx + 1, || Slot {
                generation: 1,
                data: None,
            });
        }

        let slot = &mut self.slots[idx];
        if slot.data.is_none() {
            self.active_count += 1;
            self.free_list.retain(|&f| f as usize != idx);
        }
        slot.generation = gen_val;
        slot.data = Some(item);
    }

    /// Look up an item by its 64-bit generational ID.
    #[inline]
    pub fn get(&self, id: u64) -> Option<&T> {
        let idx = (id & 0xFFFF_FFFF) as usize;
        let gen_val = (id >> 32) as u32;

        self.slots.get(idx).and_then(|slot| {
            if slot.generation == gen_val {
                slot.data.as_ref()
            } else {
                None
            }
        })
    }

    /// Mutably look up an item by its 64-bit generational ID.
    #[inline]
    pub fn get_mut(&mut self, id: u64) -> Option<&mut T> {
        let idx = (id & 0xFFFF_FFFF) as usize;
        let gen_val = (id >> 32) as u32;

        if let Some(slot) = self.slots.get_mut(idx) {
            if slot.generation == gen_val {
                return slot.data.as_mut();
            }
        }
        None
    }

    /// Remove an item by its 64-bit generational ID.
    pub fn remove(&mut self, id: u64) -> Option<T> {
        let idx = (id & 0xFFFF_FFFF) as usize;
        let gen_val = (id >> 32) as u32;

        if let Some(slot) = self.slots.get_mut(idx) {
            if slot.generation == gen_val && slot.data.is_some() {
                let item = slot.data.take();
                // Increment generation so stale IDs become invalid
                slot.generation = slot.generation.wrapping_add(1);
                if slot.generation == 0 {
                    slot.generation = 1;
                }
                self.free_list.push(idx as u32);
                self.active_count -= 1;
                return item;
            }
        }
        None
    }

    /// Returns true if the given ID corresponds to an active element in the arena.
    #[inline]
    pub fn contains(&self, id: u64) -> bool {
        self.get(id).is_some()
    }

    /// Returns the number of active items in the arena.
    #[inline]
    pub fn len(&self) -> usize {
        self.active_count
    }

    /// Returns true if the arena has no active items.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.active_count == 0
    }

    /// Clear all items from the arena.
    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            if slot.data.is_some() {
                slot.data = None;
                slot.generation = slot.generation.wrapping_add(1);
            }
        }
        self.free_list.clear();
        self.active_count = 0;
    }

    /// Iterate over (ID, &Item) for all active items.
    pub fn iter(&self) -> impl Iterator<Item = (u64, &T)> {
        self.slots.iter().enumerate().filter_map(|(idx, slot)| {
            let id = ((slot.generation as u64) << 32) | (idx as u64);
            slot.data.as_ref().map(|data| (id, data))
        })
    }

    /// Iterate over (ID, &mut Item) for all active items.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u64, &mut T)> {
        self.slots.iter_mut().enumerate().filter_map(|(idx, slot)| {
            let id = ((slot.generation as u64) << 32) | (idx as u64);
            slot.data.as_mut().map(|data| (id, data))
        })
    }
}
