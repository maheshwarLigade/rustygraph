use rusty_graph::storage::SlotArena;

#[test]
fn test_slot_arena_basic_crud() {
    let mut arena = SlotArena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);

    let id1 = arena.insert("item1");
    let id2 = arena.insert("item2");
    assert_eq!(arena.len(), 2);
    assert!(!arena.is_empty());

    assert_eq!(arena.get(id1), Some(&"item1"));
    assert_eq!(arena.get(id2), Some(&"item2"));
    assert!(arena.contains(id1));
    assert!(arena.contains(id2));

    // Remove item1
    let removed = arena.remove(id1);
    assert_eq!(removed, Some("item1"));
    assert_eq!(arena.len(), 1);
    assert!(!arena.contains(id1));
    assert_eq!(arena.get(id1), None);

    // item2 should remain intact
    assert_eq!(arena.get(id2), Some(&"item2"));
}

#[test]
fn test_slot_arena_generational_id_safety() {
    let mut arena = SlotArena::new();

    let id_a = arena.insert("Alpha");
    arena.remove(id_a);

    // Inserting next item should reuse the slot with an incremented generation
    let id_b = arena.insert("Beta");

    // The two IDs must not collide even though they share the same slot index
    assert_ne!(id_a, id_b);
    assert_eq!(arena.get(id_a), None); // Stale ID cannot access new element
    assert_eq!(arena.get(id_b), Some(&"Beta"));
}

#[test]
fn test_slot_arena_iterators() {
    let mut arena = SlotArena::new();
    let id1 = arena.insert(10);
    let id2 = arena.insert(20);
    let id3 = arena.insert(30);

    arena.remove(id2);

    let items: Vec<_> = arena.iter().collect();
    assert_eq!(items.len(), 2);
    assert!(items.contains(&(id1, &10)));
    assert!(items.contains(&(id3, &30)));

    // Test mutable iterator
    for (_, val) in arena.iter_mut() {
        *val *= 2;
    }
    assert_eq!(arena.get(id1), Some(&20));
    assert_eq!(arena.get(id3), Some(&60));
}

#[test]
fn test_slot_arena_clear() {
    let mut arena = SlotArena::new();
    let id1 = arena.insert("x");
    let id2 = arena.insert("y");

    arena.clear();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
    assert_eq!(arena.get(id1), None);
    assert_eq!(arena.get(id2), None);
}

#[test]
fn test_slot_arena_high_volume_stress() {
    let mut arena = SlotArena::with_capacity(5000);
    let mut ids = Vec::new();

    // Insert 5,000 items
    for i in 0..5000 {
        ids.push(arena.insert(i));
    }
    assert_eq!(arena.len(), 5000);

    // Remove every even index (2,500 items)
    for (idx, &id) in ids.iter().enumerate() {
        if idx % 2 == 0 {
            assert_eq!(arena.remove(id), Some(idx));
        }
    }
    assert_eq!(arena.len(), 2500);

    // Re-verify that all odd items are present and correct
    for (idx, &id) in ids.iter().enumerate() {
        if idx % 2 != 0 {
            assert_eq!(arena.get(id), Some(&idx));
        } else {
            assert_eq!(arena.get(id), None);
        }
    }

    // Insert 2,500 new items (should reuse freed slots)
    let mut new_ids = Vec::new();
    for i in 5000..7500 {
        new_ids.push(arena.insert(i));
    }
    assert_eq!(arena.len(), 5000);

    // Verify all newly inserted items
    for (i, &new_id) in new_ids.iter().enumerate() {
        assert_eq!(arena.get(new_id), Some(&(5000 + i)));
    }
}
