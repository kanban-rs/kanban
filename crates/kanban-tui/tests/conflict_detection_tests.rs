use kanban_domain::{Board, Card, Column, Snapshot};
use kanban_persistence::PersistenceError;
use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};
use kanban_persistence_json::JsonFileStore;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_conflict_detection_on_concurrent_modification() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    // Create initial data and save via first instance
    let mut board = Board::new("Test Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(&mut board, column.id, "Test Task", 0);

    let snapshot1 = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board.clone()],
        columns: vec![column.clone()],
        cards: vec![card.clone()],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        archived_cards: vec![],
    };

    let store_id = uuid::Uuid::new_v4();
    let store = JsonFileStore::with_instance_id(&file_path, store_id);
    let data1 = kanban_persistence::snapshot_to_json_bytes(&snapshot1).unwrap();
    let persist_snapshot1 = StoreSnapshot {
        data: data1,
        metadata: PersistenceMetadata::new(store_id),
    };
    store.save(persist_snapshot1).await.unwrap();

    // External modification: change the file directly
    let modified_data = serde_json::json!({
        "version": 2,
        "metadata": {
            "instance_id": uuid::Uuid::new_v4().to_string(),
            "saved_at": chrono::Utc::now().to_rfc3339()
        },
        "data": {}
    });

    fs::write(
        &file_path,
        serde_json::to_string_pretty(&modified_data).unwrap(),
    )
    .unwrap();

    // Small delay to ensure file metadata changes
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Same instance tries to save and should detect conflict
    let data2 = kanban_persistence::snapshot_to_json_bytes(&snapshot1).unwrap();
    let persist_snapshot2 = StoreSnapshot {
        data: data2,
        metadata: PersistenceMetadata::new(store_id),
    };

    let result = store.save(persist_snapshot2).await;
    assert!(result.is_err(), "Should detect conflict on second save");

    match result {
        Err(PersistenceError::ConflictDetected { path, .. }) => {
            assert!(
                path.contains("kanban.json"),
                "Error should contain file path"
            );
        }
        _ => panic!("Expected ConflictDetected error"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_no_conflict_when_file_unchanged() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    // Create and save initial data
    let mut board = Board::new("Test Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(&mut board, column.id, "Test Task", 0);

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board.clone()],
        columns: vec![column.clone()],
        cards: vec![card.clone()],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        archived_cards: vec![],
    };

    let store = JsonFileStore::new(&file_path);
    let data = kanban_persistence::snapshot_to_json_bytes(&snapshot).unwrap();
    let persist_snapshot = StoreSnapshot {
        data: data.clone(),
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(persist_snapshot.clone()).await.unwrap();

    // Save again with same data - should not detect conflict
    let result = store.save(persist_snapshot).await;
    assert!(
        result.is_ok(),
        "Should not detect conflict when file unchanged"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_conflict_detection_tracks_file_metadata() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    // Create and save initial data
    let mut board = Board::new("Test Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let _card = Card::new(&mut board, column.id, "Test Task", 0);

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board.clone()],
        columns: vec![column.clone()],
        cards: vec![],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        archived_cards: vec![],
    };

    let store = JsonFileStore::new(&file_path);
    let data = kanban_persistence::snapshot_to_json_bytes(&snapshot).unwrap();
    let persist_snapshot = StoreSnapshot {
        data,
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(persist_snapshot.clone()).await.unwrap();

    // Verify save was successful - file exists
    assert!(
        file_path.exists(),
        "File should exist after successful save"
    );

    // Modify file externally
    let modified_data = serde_json::json!({
        "version": 2,
        "metadata": {
            "instance_id": uuid::Uuid::new_v4().to_string(),
            "saved_at": chrono::Utc::now().to_rfc3339()
        },
        "data": {}
    });
    fs::write(
        &file_path,
        serde_json::to_string_pretty(&modified_data).unwrap(),
    )
    .unwrap();

    // Small delay to ensure file metadata changes
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Try to save and should detect conflict
    let result = store.save(persist_snapshot).await;
    assert!(
        result.is_err(),
        "Should detect conflict after external modification"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_instances_with_different_ids() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    let mut board = Board::new("Test Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(&mut board, column.id, "Test Task", 0);

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board.clone()],
        columns: vec![column.clone()],
        cards: vec![card],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        archived_cards: vec![],
    };

    // First instance saves
    let store1_id = uuid::Uuid::new_v4();
    let store1 = JsonFileStore::with_instance_id(&file_path, store1_id);
    let data = kanban_persistence::snapshot_to_json_bytes(&snapshot).unwrap();
    let persist_snapshot1 = StoreSnapshot {
        data: data.clone(),
        metadata: PersistenceMetadata::new(store1_id),
    };
    store1.save(persist_snapshot1).await.unwrap();

    // Second instance with different ID loads and saves
    let store2_id = uuid::Uuid::new_v4();
    let store2 = JsonFileStore::with_instance_id(&file_path, store2_id);
    let persist_snapshot2 = StoreSnapshot {
        data: data.clone(),
        metadata: PersistenceMetadata::new(store2_id),
    };

    // Should succeed because it's a different instance
    let result = store2.save(persist_snapshot2).await;
    assert!(
        result.is_ok(),
        "Different instance should be able to save without conflict"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_conflict_resolution_with_force_overwrite() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    let mut board = Board::new("Test Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(&mut board, column.id, "Test Task", 0);

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board.clone()],
        columns: vec![column.clone()],
        cards: vec![card],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        archived_cards: vec![],
    };

    let store = JsonFileStore::new(&file_path);

    // Save initial state
    let data = kanban_persistence::snapshot_to_json_bytes(&snapshot).unwrap();
    let persist_snapshot = StoreSnapshot {
        data: data.clone(),
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(persist_snapshot.clone()).await.unwrap();

    // Externally modify file
    let modified_data = serde_json::json!({"version": 2, "metadata": {}, "data": {}});
    fs::write(
        &file_path,
        serde_json::to_string_pretty(&modified_data).unwrap(),
    )
    .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Try to save and get conflict
    let result = store.save(persist_snapshot.clone()).await;
    assert!(result.is_err(), "Should detect conflict");

    // Now save with force overwrite (simulating user choosing to keep their changes)
    let result = store.save(persist_snapshot).await;
    assert!(
        result.is_err(),
        "Conflict should still be detected until metadata is cleared"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_multi_instance_concurrent_editing_3_instances() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("shared_board.json");

    // Instance 1: Create initial board with data
    let instance1_id = uuid::Uuid::new_v4();
    let store1 = JsonFileStore::with_instance_id(&file_path, instance1_id);

    let mut board1 = Board::new("Shared Project", None::<String>);
    let column1 = Column::new(board1.id, "Todo", 0);
    let column2 = Column::new(board1.id, "In Progress", 1);

    let card1 = Card::new(&mut board1, column1.id, "Task A", 0);
    let card2 = Card::new(&mut board1, column2.id, "Task B", 0);

    let snapshot1 = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board1.clone()],
        columns: vec![column1.clone(), column2.clone()],
        cards: vec![card1.clone(), card2.clone()],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        archived_cards: vec![],
    };

    let data = kanban_persistence::snapshot_to_json_bytes(&snapshot1).unwrap();
    let persist_snapshot = StoreSnapshot {
        data: data.clone(),
        metadata: PersistenceMetadata::new(instance1_id),
    };
    store1.save(persist_snapshot).await.unwrap();

    // Instance 2: Load and modify (add a new card in Todo column)
    let instance2_id = uuid::Uuid::new_v4();
    let store2 = JsonFileStore::with_instance_id(&file_path, instance2_id);

    let (loaded_snap, _) = store2.load().await.unwrap();
    let mut snapshot2: Snapshot = serde_json::from_slice(&loaded_snap.data).unwrap();

    let new_card = Card::new(
        &mut snapshot2.boards[0],
        snapshot2.columns[0].id,
        "Task C (from Instance 2)",
        0,
    );
    snapshot2.cards.push(new_card);

    let data2 = kanban_persistence::snapshot_to_json_bytes(&snapshot2).unwrap();
    let persist_snapshot2 = StoreSnapshot {
        data: data2,
        metadata: PersistenceMetadata::new(instance2_id),
    };

    // Ensure file modification time changes
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result2 = store2.save(persist_snapshot2.clone()).await;
    assert!(
        result2.is_ok(),
        "Instance 2 should save successfully (instance 1 hasn't tried yet)"
    );

    // Instance 3: Load and also modify (rename a card)
    let instance3_id = uuid::Uuid::new_v4();
    let store3 = JsonFileStore::with_instance_id(&file_path, instance3_id);

    let (loaded_snap3, _) = store3.load().await.unwrap();
    let mut snapshot3: Snapshot = serde_json::from_slice(&loaded_snap3.data).unwrap();

    // Rename the first card
    snapshot3.cards[0].title = "Task A - Updated by Instance 3".to_string();

    let data3 = kanban_persistence::snapshot_to_json_bytes(&snapshot3).unwrap();
    let persist_snapshot3 = StoreSnapshot {
        data: data3,
        metadata: PersistenceMetadata::new(instance3_id),
    };

    tokio::time::sleep(Duration::from_millis(50)).await;

    let result3 = store3.save(persist_snapshot3.clone()).await;
    assert!(
        result3.is_ok(),
        "Instance 3 should save successfully (instance 2 saved, but store tracks different instance)"
    );

    // Now Instance 1 tries to save (has outdated metadata)
    // This should fail because file was modified by instances 2 and 3
    let persist_snapshot_retry = StoreSnapshot {
        data: data.clone(),
        metadata: PersistenceMetadata::new(instance1_id),
    };
    let result1_retry = store1.save(persist_snapshot_retry).await;
    assert!(
        result1_retry.is_err(),
        "Instance 1 should detect conflict after other instances modified"
    );

    match result1_retry {
        Err(PersistenceError::ConflictDetected { path, .. }) => {
            assert!(path.contains("shared_board.json"));
        }
        _ => panic!("Expected ConflictDetected error"),
    }

    // Verify final state: Instance 3's save wins (last write)
    let (final_snap, _) = store3.load().await.unwrap();
    let final_data: Snapshot = serde_json::from_slice(&final_snap.data).unwrap();

    // Should have 2 cards (Instance 2 added one, Instance 3 renamed another)
    assert_eq!(final_data.cards.len(), 3, "Should have 3 cards total");

    // Verify Instance 3's change is present (renamed task A)
    let task_a = final_data
        .cards
        .iter()
        .find(|c| c.title.contains("Updated by Instance 3"))
        .expect("Instance 3's change should be in final state");
    assert_eq!(task_a.title, "Task A - Updated by Instance 3");

    // Verify Instance 2's change is present (added task C)
    let task_c = final_data
        .cards
        .iter()
        .find(|c| c.title.contains("Task C (from Instance 2)"))
        .expect("Instance 2's addition should be in final state");
    assert!(task_c.title.contains("Task C (from Instance 2)"));

    // All instances should be able to detect and load the final state
    let (store1_final, _) = store1.load().await.unwrap();
    let store1_final_data: Snapshot = serde_json::from_slice(&store1_final.data).unwrap();
    assert_eq!(
        store1_final_data.cards.len(),
        3,
        "Instance 1 can see final state with all 3 cards"
    );
}
