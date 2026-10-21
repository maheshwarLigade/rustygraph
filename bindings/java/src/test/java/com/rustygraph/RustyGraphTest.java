package com.rustygraph;

import java.util.List;
import java.util.Map;

public class RustyGraphTest {
    public static void main(String[] args) {
        System.out.println("╔═══════════════════════════════════════════════════════════════════╗");
        System.out.println("║            rusty_graph — Java 21 SDK Demonstration                ║");
        System.out.println("╚═══════════════════════════════════════════════════════════════════╝\n");

        try (RustyGraph graph = RustyGraph.create()) {
            System.out.println("[Step 1] Initialized native RustyGraph instance in Java");
            System.out.println("  Initial node count: " + graph.nodeCount());

            // 1. Create Nodes
            long aliceId = graph.createNode(
                List.of("Person", "Engineer"),
                Map.of("name", "Alice", "age", 30)
            );
            System.out.println("  Created Node Alice (ID: " + aliceId + ")");

            long bobId = graph.createNode(
                List.of("Person"),
                Map.of("name", "Bob", "age", 28)
            );
            System.out.println("  Created Node Bob (ID: " + bobId + ")");

            // 2. Create Relationship
            long relId = graph.createEdge(aliceId, bobId, "KNOWS", Map.of("since", 2021));
            System.out.println("  Created Relationship KNOWS (ID: " + relId + ")");
            System.out.println("  Graph summary: " + graph.nodeCount() + " nodes, " + graph.edgeCount() + " edges\n");

            // 3. Execute Cypher Query from Java
            System.out.println("[Step 2] Executing Cypher Query from Java...");
            CypherResult result = graph.cypher(
                "MATCH (p:Person)-[:KNOWS]->(f:Person) WHERE f.age < 30 RETURN p.name, f.name"
            );
            System.out.println("  Cypher success: " + result.success());
            System.out.println("  Columns: " + result.columns());
            System.out.println("  Rows returned: " + result.rows());

            if (!result.hasRows()) {
                throw new AssertionError("Expected matched rows in Cypher result");
            }
            System.out.println("  ✔ Java Cypher query matched successfully!\n");

            // 4. Vector Database Search from Java
            System.out.println("[Step 3] Vector Indexing & Semantic Search in Java...");
            graph.createVectorIndex("paper_index", "Paper", "vec", 3, VectorSimilarity.COSINE);

            graph.createNode(
                List.of("Paper"),
                Map.of("title", "Java FFM Rust Graph Engine", "vec", new float[]{1.0f, 0.0f, 0.0f})
            );
            graph.createNode(
                List.of("Paper"),
                Map.of("title", "Unrelated Machine Learning", "vec", new float[]{0.0f, 1.0f, 0.0f})
            );

            List<VectorHit> hits = graph.queryVectorIndex("paper_index", 1, new float[]{0.95f, 0.05f, 0.0f});
            System.out.println("  Nearest neighbor hits: " + hits);
            if (hits.isEmpty() || hits.get(0).score() < 0.90f) {
                throw new AssertionError("Vector search score lower than expected");
            }
            System.out.println("  ✔ Top match Node ID " + hits.get(0).nodeId() + " with score: " + hits.get(0).score() + "\n");

            // 5. JSON Serialization
            System.out.println("[Step 4] Testing JSON Export/Import in Java...");
            String json = graph.toJson();
            System.out.println("  Exported graph JSON (" + json.length() + " bytes)");

            try (RustyGraph imported = RustyGraph.fromJson(json)) {
                System.out.println("  Imported graph node count: " + imported.nodeCount());
                if (imported.nodeCount() != graph.nodeCount()) {
                    throw new AssertionError("Imported node count mismatch");
                }
            }
            System.out.println("  ✔ JSON round-trip verified!\n");

            System.out.println("Java SDK test run completed successfully!");
        }
    }
}
