package main

import (
	"fmt"
	"log"

	"github.com/mahesh/rusty-graph/bindings/go/rustygraph"
)

func main() {
	fmt.Println("=====================================================================")
	fmt.Println("             rusty_graph - Go SDK Demonstration                     ")
	fmt.Println("=====================================================================")

	// 1. Initialize an in-memory Graph instance
	graph, err := rustygraph.New()
	if err != nil {
		log.Fatalf("Failed to initialize rusty_graph: %v", err)
	}
	defer graph.Close()

	fmt.Printf("[Step 1] Initialized native RustyGraph instance in Go\n")
	fmt.Printf("  Initial node count: %d\n", graph.NodeCount())

	// 2. Create property graph nodes
	aliceID, err := graph.CreateNode([]string{"Developer", "Engineer"}, map[string]interface{}{
		"name": "Alice",
		"lang": "Rust",
		"age":  30,
	})
	if err != nil {
		log.Fatalf("Failed to create Alice: %v", err)
	}
	fmt.Printf("  Created Node Alice (ID: %d)\n", aliceID)

	bobID, err := graph.CreateNode([]string{"Developer"}, map[string]interface{}{
		"name": "Bob",
		"lang": "Go",
		"age":  28,
	})
	if err != nil {
		log.Fatalf("Failed to create Bob: %v", err)
	}
	fmt.Printf("  Created Node Bob (ID: %d)\n", bobID)

	// 3. Create relationship between nodes
	edgeID, err := graph.CreateEdge(aliceID, bobID, "COLLABORATES_WITH", map[string]interface{}{
		"project": "rusty-graph",
		"since":   2024,
	})
	if err != nil {
		log.Fatalf("Failed to create relationship: %v", err)
	}
	fmt.Printf("  Created Edge COLLABORATES_WITH (ID: %d)\n", edgeID)
	fmt.Printf("  Current Graph topology: %d nodes, %d edges\n\n", graph.NodeCount(), graph.EdgeCount())

	// 4. Execute openCypher Query
	fmt.Println("[Step 2] Executing openCypher Query from Go...")
	query := "MATCH (p:Developer)-[:COLLABORATES_WITH]->(c:Developer) WHERE c.age < 30 RETURN p.name, c.name"
	fmt.Printf("  Query: %s\n", query)

	result, err := graph.Cypher(query)
	if err != nil {
		log.Fatalf("Cypher query failed: %v", err)
	}
	fmt.Printf("  Success: %v\n", result.Success)
	fmt.Printf("  Columns: %v\n", result.Columns)
	fmt.Printf("  Matched rows: %v\n\n", result.Rows)

	// 5. Create Vector Index & Semantic Search
	fmt.Println("[Step 3] Vector Indexing & Semantic Search in Go...")
	err = graph.CreateVectorIndex("skills_idx", "Engineer", "skill_vec", 3, rustygraph.SimilarityCosine)
	if err != nil {
		log.Fatalf("Failed to create vector index: %v", err)
	}

	charlesID, err := graph.CreateNode([]string{"Engineer"}, map[string]interface{}{
		"name":      "Charles",
		"skill_vec": []float32{1.0, 0.0, 0.0},
	})
	if err != nil {
		log.Fatalf("Failed to create Charles: %v", err)
	}
	fmt.Printf("  Indexed Node Charles (ID: %d) with vector [1.0, 0.0, 0.0]\n", charlesID)

	hits, err := graph.QueryVectorIndex("skills_idx", 1, []float32{0.95, 0.05, 0.0})
	if err != nil {
		log.Fatalf("Vector search failed: %v", err)
	}
	if len(hits) > 0 {
		fmt.Printf("  Top nearest neighbor: Node %d (Cosine similarity score: %.4f)\n\n", hits[0].NodeID, hits[0].Score)
	}

	// 6. JSON Export & Round-Trip
	fmt.Println("[Step 4] Testing JSON Serialization Round-Trip in Go...")
	jsonDump, err := graph.ToJSON()
	if err != nil {
		log.Fatalf("Failed to serialize graph to JSON: %v", err)
	}
	fmt.Printf("  Exported JSON payload length: %d bytes\n", len(jsonDump))

	restoredGraph, err := rustygraph.FromJSON(jsonDump)
	if err != nil {
		log.Fatalf("Failed to deserialize graph from JSON: %v", err)
	}
	defer restoredGraph.Close()
	fmt.Printf("  Restored Graph node count: %d\n", restoredGraph.NodeCount())

	fmt.Println("\nGo SDK demonstration completed successfully!")
}
