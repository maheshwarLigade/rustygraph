use rusty_graph::prelude::*;
use std::fs;
use std::thread;
use std::time::Duration;

#[test]
fn test_cypher_mutations_create_set_delete() -> Result<()> {
    let mut graph = Graph::new();

    // 1. CREATE nodes and relationships
    let create_res = graph.cypher(
        "CREATE (a:Person {name: 'Alice', age: 25})-[:KNOWS]->(b:Person {name: 'Bob', age: 30})",
    )?;
    assert!(create_res.success);
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 1);

    // 2. Query created entities
    let read_res = graph.cypher("MATCH (p:Person {name: 'Alice'}) RETURN p.name, p.age")?;
    assert_eq!(read_res.rows.len(), 1);
    assert_eq!(
        read_res.rows[0][0],
        PropertyValue::String("Alice".to_string())
    );
    assert_eq!(read_res.rows[0][1], PropertyValue::Int(25));

    // 3. SET clause mutation
    let set_res = graph.cypher("MATCH (p:Person {name: 'Alice'}) SET p.age = 26")?;
    assert!(set_res.success);

    let verify_set = graph.cypher("MATCH (p:Person {name: 'Alice'}) RETURN p.age")?;
    assert_eq!(verify_set.rows[0][0], PropertyValue::Int(26));

    // 4. Strict DELETE should fail when node has connected relationships
    let delete_err = graph.cypher("MATCH (p:Person {name: 'Alice'}) DELETE p");
    assert!(
        delete_err.is_err(),
        "Strict DELETE without DETACH must fail when relationships exist"
    );

    // 5. DETACH DELETE should succeed and clean up connected relationships
    let detach_res = graph.cypher("MATCH (p:Person {name: 'Alice'}) DETACH DELETE p")?;
    assert!(detach_res.success);
    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.edge_count(), 0);

    let check_alice = graph.cypher("MATCH (p:Person {name: 'Alice'}) RETURN p.name")?;
    assert_eq!(check_alice.rows.len(), 0);

    Ok(())
}

#[test]
fn test_cypher_aggregations_and_ordering() -> Result<()> {
    let mut graph = Graph::new();

    graph.cypher("CREATE (p:Product {name: 'Laptop', price: 1200})")?;
    graph.cypher("CREATE (p:Product {name: 'Mouse', price: 50})")?;
    graph.cypher("CREATE (p:Product {name: 'Keyboard', price: 150})")?;
    graph.cypher("CREATE (p:Product {name: 'Monitor', price: 400})")?;

    // 1. Aggregations: count(*), sum, avg, min, max
    let agg_res = graph.cypher(
        "MATCH (p:Product) RETURN count(*), sum(p.price), avg(p.price), min(p.price), max(p.price)",
    )?;
    assert_eq!(agg_res.rows.len(), 1);
    let row = &agg_res.rows[0];
    assert_eq!(row[0], PropertyValue::Int(4)); // count(*)
    assert_eq!(row[1], PropertyValue::Float(1800.0)); // sum
    assert_eq!(row[2], PropertyValue::Float(450.0)); // avg (1800 / 4)
    assert_eq!(row[3], PropertyValue::Int(50)); // min
    assert_eq!(row[4], PropertyValue::Int(1200)); // max

    // 2. ORDER BY DESC with LIMIT
    let order_desc =
        graph.cypher("MATCH (p:Product) RETURN p.name, p.price ORDER BY p.price DESC LIMIT 2")?;
    assert_eq!(order_desc.rows.len(), 2);
    assert_eq!(
        order_desc.rows[0][0],
        PropertyValue::String("Laptop".to_string())
    );
    assert_eq!(order_desc.rows[0][1], PropertyValue::Int(1200));
    assert_eq!(
        order_desc.rows[1][0],
        PropertyValue::String("Monitor".to_string())
    );
    assert_eq!(order_desc.rows[1][1], PropertyValue::Int(400));

    // 3. ORDER BY ASC
    let order_asc =
        graph.cypher("MATCH (p:Product) RETURN p.name, p.price ORDER BY p.price ASC LIMIT 2")?;
    assert_eq!(order_asc.rows.len(), 2);
    assert_eq!(
        order_asc.rows[0][0],
        PropertyValue::String("Mouse".to_string())
    );
    assert_eq!(order_asc.rows[0][1], PropertyValue::Int(50));
    assert_eq!(
        order_asc.rows[1][0],
        PropertyValue::String("Keyboard".to_string())
    );
    assert_eq!(order_asc.rows[1][1], PropertyValue::Int(150));

    Ok(())
}

#[test]
fn test_acid_transaction_commit_and_isolation() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!(
        "rusty_tx_test_{}",
        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
    ));
    fs::create_dir_all(&temp_dir)?;

    let config = DatabaseConfig {
        data_dir: temp_dir.clone(),
        fsync_policy: "always".to_string(),
        keep_snapshots: 2,
        auto_checkpoint_ops: 1000,
    };

    let db = StandaloneDatabase::open(config)?;

    // Populate baseline node
    db.cypher("CREATE (u:User {name: 'Root', role: 'system'})")?;

    // 1. Begin Transaction
    let tx_id = db.begin_transaction(Some(Duration::from_secs(10)))?;
    assert_eq!(db.tx_manager().active_count(), 1);

    // 2. Execute statements inside transaction
    db.execute_tx(tx_id, "CREATE (u:User {name: 'Alice', role: 'admin'})")?;
    db.execute_tx(tx_id, "CREATE (u:User {name: 'Bob', role: 'member'})")?;
    db.execute_tx(
        tx_id,
        "MATCH (u:User {name: 'Alice'}) SET u.status = 'active'",
    )?;

    // 3. ISOLATION CHECK: Live database must NOT see staged uncommitted writes
    let live_res = db.cypher("MATCH (u:User) RETURN u.name")?;
    assert_eq!(
        live_res.rows.len(),
        1,
        "Live database must remain isolated before commit"
    );
    assert_eq!(
        live_res.rows[0][0],
        PropertyValue::String("Root".to_string())
    );

    // 4. TRANSACTION VIEW: Transaction sees its own staged writes
    let tx_res = db.execute_tx(tx_id, "MATCH (u:User) RETURN u.name")?;
    assert_eq!(
        tx_res.rows.len(),
        3,
        "Transaction must see its own uncommitted modifications"
    );

    // 5. COMMIT: Persist staged mutations to live database and WAL
    db.commit_transaction(tx_id)?;
    assert_eq!(db.tx_manager().active_count(), 0);

    // 6. POST-COMMIT: Live database now reflects all committed changes
    let committed_res = db.cypher("MATCH (u:User) RETURN u.name, u.status")?;
    assert_eq!(committed_res.rows.len(), 3);

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_acid_transaction_rollback() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!(
        "rusty_tx_rollback_{}",
        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
    ));
    fs::create_dir_all(&temp_dir)?;

    let config = DatabaseConfig {
        data_dir: temp_dir.clone(),
        fsync_policy: "always".to_string(),
        keep_snapshots: 2,
        auto_checkpoint_ops: 1000,
    };

    let db = StandaloneDatabase::open(config)?;
    db.cypher("CREATE (u:User {name: 'Original'})")?;

    // Begin and execute mutations
    let tx_id = db.begin_transaction(Some(Duration::from_secs(10)))?;
    db.execute_tx(tx_id, "CREATE (u:User {name: 'Ghost'})")?;
    db.execute_tx(
        tx_id,
        "MATCH (u:User {name: 'Original'}) SET u.name = 'Mutated'",
    )?;

    // Rollback transaction
    db.rollback_transaction(tx_id)?;
    assert_eq!(db.tx_manager().active_count(), 0);

    // Verify live DB was completely untouched
    let check_ghost = db.cypher("MATCH (u:User {name: 'Ghost'}) RETURN u.name")?;
    assert_eq!(check_ghost.rows.len(), 0);

    let check_orig = db.cypher("MATCH (u:User {name: 'Original'}) RETURN u.name")?;
    assert_eq!(check_orig.rows.len(), 1);

    // Attempting to execute in rolled back tx must fail
    let closed_err = db.execute_tx(tx_id, "MATCH (n) RETURN n");
    assert!(closed_err.is_err());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_acid_transaction_timeout_expiration() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!(
        "rusty_tx_timeout_{}",
        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
    ));
    fs::create_dir_all(&temp_dir)?;

    let config = DatabaseConfig {
        data_dir: temp_dir.clone(),
        fsync_policy: "always".to_string(),
        keep_snapshots: 2,
        auto_checkpoint_ops: 1000,
    };

    let db = StandaloneDatabase::open(config)?;

    // Start tx with short 50ms timeout
    let tx_id = db.begin_transaction(Some(Duration::from_millis(50)))?;

    // Allow transaction to expire
    thread::sleep(Duration::from_millis(100));

    // Attempting to execute in expired transaction must return an error
    let exec_res = db.execute_tx(tx_id, "CREATE (n:Test)");
    assert!(
        exec_res.is_err(),
        "Execution in expired transaction must fail"
    );

    // Attempting to commit expired transaction must fail
    let commit_res = db.commit_transaction(tx_id);
    assert!(
        commit_res.is_err(),
        "Commit on expired transaction must fail"
    );

    // Reaper removes it
    let reaped = db.tx_manager().reap_expired();
    assert!(reaped.contains(&tx_id) || db.tx_manager().active_count() == 0);

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_acid_transaction_crash_recovery_from_wal() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!(
        "rusty_tx_recovery_{}",
        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
    ));
    fs::create_dir_all(&temp_dir)?;

    let config = DatabaseConfig {
        data_dir: temp_dir.clone(),
        fsync_policy: "always".to_string(),
        keep_snapshots: 2,
        auto_checkpoint_ops: 1000,
    };

    // Session 1: Perform committed transaction
    {
        let db = StandaloneDatabase::open(config.clone())?;
        let tx_id = db.begin_transaction(None)?;
        db.execute_tx(tx_id, "CREATE (p:Product {name: 'GPU', price: 800})")?;
        db.execute_tx(tx_id, "CREATE (p:Product {name: 'CPU', price: 400})")?;
        db.commit_transaction(tx_id)?;
        // Database dropped here (simulating shutdown / crash)
    }

    // Session 2: Reopen database and verify WAL replay of committed transaction
    {
        let db2 = StandaloneDatabase::open(config)?;
        let res = db2.cypher("MATCH (p:Product) RETURN p.name, p.price ORDER BY p.price DESC")?;
        assert_eq!(
            res.rows.len(),
            2,
            "Committed transaction operations must be restored from WAL"
        );
        assert_eq!(res.rows[0][0], PropertyValue::String("GPU".to_string()));
        assert_eq!(res.rows[0][1], PropertyValue::Int(800));
        assert_eq!(res.rows[1][0], PropertyValue::String("CPU".to_string()));
        assert_eq!(res.rows[1][1], PropertyValue::Int(400));
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
