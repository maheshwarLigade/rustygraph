/**
 * rusty_graph.h - C/C++ Header for the in-memory rusty_graph database engine
 */

#ifndef RUSTY_GRAPH_H
#define RUSTY_GRAPH_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque Graph handle
typedef struct Graph Graph;

// Lifecycle
Graph* rusty_graph_new(void);
void rusty_graph_free(Graph* ptr);
void rusty_graph_string_free(char* ptr);

// Statistics
uint64_t rusty_graph_node_count(const Graph* ptr);
uint64_t rusty_graph_edge_count(const Graph* ptr);

// Node & Edge CRUD
uint64_t rusty_graph_create_node(Graph* ptr, const char* labels_json, const char* props_json);
int32_t  rusty_graph_delete_node(Graph* ptr, uint64_t node_id);
uint64_t rusty_graph_create_edge(Graph* ptr, uint64_t source, uint64_t target, const char* rel_type, const char* props_json);
int32_t  rusty_graph_delete_edge(Graph* ptr, uint64_t rel_id);

// Cypher Queries
// Returns JSON string: {"success": bool, "columns": [...], "rows": [...], "error": ...}
char* rusty_graph_cypher(Graph* ptr, const char* query);

// Vector Database & Nearest Neighbor Search
// metric: 0 = Cosine, 1 = Euclidean, 2 = DotProduct
int32_t rusty_graph_create_vector_index(Graph* ptr, const char* name, const char* label, const char* prop_key, size_t dimension, uint32_t metric);
// Returns JSON string array: [{"node_id": u64, "score": f32}, ...]
char*   rusty_graph_query_vector_index(const Graph* ptr, const char* name, size_t k, const float* vec_ptr, size_t vec_len);

// Persistence
char*  rusty_graph_to_json(const Graph* ptr);
Graph* rusty_graph_from_json(const char* json_str);

#ifdef __cplusplus
}
#endif

#endif // RUSTY_GRAPH_H
