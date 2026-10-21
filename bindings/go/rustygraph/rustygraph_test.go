package rustygraph

import (
	"fmt"
	"testing"
)

func TestGoGraphLifecycleAndCypher(t *testing.T) {
	g, err := New()
	if err != nil {
		t.Fatalf("Failed to create Go graph: %v", err)
	}
	defer g.Close()

	if count := g.NodeCount(); count != 0 {
		t.Errorf("Expected 0 nodes, got %d", count)
	}

	// 1. Create Nodes
	aliceID, err := g.CreateNode([]string{"Person", "Developer"}, map[string]interface{}{
		"name": "Alice",
		"age":  30,
	})
	if err != nil {
		t.Fatalf("Failed to create Alice: %v", err)
	}

	bobID, err := g.CreateNode([]string{"Person"}, map[string]interface{}{
		"name": "Bob",
		"age":  28,
	})
	if err != nil {
		t.Fatalf("Failed to create Bob: %v", err)
	}

	if g.NodeCount() != 2 {
		t.Errorf("Expected 2 nodes, got %d", g.NodeCount())
	}

	// 2. Create Directed Relationship
	edgeID, err := g.CreateEdge(aliceID, bobID, "KNOWS", map[string]interface{}{
		"since": 2021,
	})
	if err != nil {
		t.Fatalf("Failed to create edge: %v", err)
	}

	if edgeID == 0 || g.EdgeCount() != 1 {
		t.Errorf("Expected 1 edge, got %d", g.EdgeCount())
	}

	// 3. Execute Cypher Query
	res, err := g.Cypher("MATCH (p:Person)-[:KNOWS]->(f:Person) WHERE f.age < 30 RETURN p.name, f.name")
	if err != nil {
		t.Fatalf("Cypher query failed: %v", err)
	}

	if len(res.Columns) != 2 {
		t.Errorf("Expected 2 columns, got %d", len(res.Columns))
	}
	if len(res.Rows) != 1 {
		t.Errorf("Expected 1 row, got %d", len(res.Rows))
	}

	fmt.Printf("Go Cypher Query Matched: %v\n", res.Rows)

	// 4. Test Vector Index in Go
	err = g.CreateVectorIndex("doc_vecs", "Document", "embedding", 3, SimilarityCosine)
	if err != nil {
		t.Fatalf("Failed to create vector index: %v", err)
	}

	_, err = g.CreateNode([]string{"Document"}, map[string]interface{}{
		"title":     "Go Rust Graph Engine",
		"embedding": []float32{1.0, 0.0, 0.0},
	})
	if err != nil {
		t.Fatalf("Failed to create vector node: %v", err)
	}

	hits, err := g.QueryVectorIndex("doc_vecs", 1, []float32{0.9, 0.1, 0.0})
	if err != nil {
		t.Fatalf("Failed vector search: %v", err)
	}

	if len(hits) != 1 {
		t.Fatalf("Expected 1 vector hit, got %d", len(hits))
	}
	if hits[0].Score < 0.95 {
		t.Errorf("Expected high cosine similarity score, got %f", hits[0].Score)
	}
	fmt.Printf("Go Vector Search Hit: Node %d, Score: %f\n", hits[0].NodeID, hits[0].Score)

	// 5. Test JSON Export and Import in Go
	jsonStr, err := g.ToJSON()
	if err != nil {
		t.Fatalf("Failed ToJSON: %v", err)
	}
	if len(jsonStr) == 0 {
		t.Fatalf("Empty JSON exported")
	}

	importedGraph, err := FromJSON(jsonStr)
	if err != nil {
		t.Fatalf("Failed FromJSON: %v", err)
	}
	defer importedGraph.Close()

	if importedGraph.NodeCount() != g.NodeCount() {
		t.Errorf("Imported graph node count mismatch: %d vs %d", importedGraph.NodeCount(), g.NodeCount())
	}
}
