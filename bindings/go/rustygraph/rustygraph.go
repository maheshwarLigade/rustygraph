// Package rustygraph provides high-performance in-memory property graph database bindings
// and native Cypher query execution for Go applications, backed by the rusty_graph Rust core.
package rustygraph

import (
	"errors"
	"runtime"
)

// VectorSimilarity defines the distance metric used for vector index nearest neighbor search.
type VectorSimilarity int

const (
	// SimilarityCosine computes normalized cosine similarity [-1, 1].
	SimilarityCosine VectorSimilarity = 0
	// SimilarityEuclidean computes inverted Euclidean distance (0, 1].
	SimilarityEuclidean VectorSimilarity = 1
	// SimilarityDotProduct computes unnormalized dot product.
	SimilarityDotProduct VectorSimilarity = 2
)

// QueryResult represents the tabular output of an executed Cypher query.
type QueryResult struct {
	Success bool            `json:"success"`
	Columns []string        `json:"columns"`
	Rows    [][]interface{} `json:"rows"`
	Error   *string         `json:"error"`
}

// VectorHit represents a single matched node and its similarity score from a vector query.
type VectorHit struct {
	NodeID uint64  `json:"node_id"`
	Score  float32 `json:"score"`
}

// SetLibraryPath explicitly sets a custom path to the rusty_graph dynamic library.
// Useful when deploying binaries where the shared library is located in a custom directory.
func SetLibraryPath(path string) error {
	return setLibraryPathPlatform(path)
}

// Graph represents an in-memory property graph database instance in Go.
type Graph struct {
	ptr uintptr
}

// New creates a new in-memory Graph instance backed by the Rust engine.
func New() (*Graph, error) {
	ptr, err := callNew()
	if err != nil {
		return nil, err
	}
	g := &Graph{ptr: ptr}
	runtime.SetFinalizer(g, (*Graph).Close)
	return g, nil
}

// Close explicitly frees the native Graph memory in Rust.
func (g *Graph) Close() {
	if g.ptr != 0 {
		callFree(g.ptr)
		g.ptr = 0
		runtime.SetFinalizer(g, nil)
	}
}

// NodeCount returns the total number of active nodes in the graph.
func (g *Graph) NodeCount() uint64 {
	if g.ptr == 0 {
		return 0
	}
	return callNodeCount(g.ptr)
}

// EdgeCount returns the total number of active relationships in the graph.
func (g *Graph) EdgeCount() uint64 {
	if g.ptr == 0 {
		return 0
	}
	return callEdgeCount(g.ptr)
}

// CreateNode creates a new node with optional labels and key-value properties.
func (g *Graph) CreateNode(labels []string, properties map[string]interface{}) (uint64, error) {
	if g.ptr == 0 {
		return 0, errors.New("graph is closed")
	}
	return callCreateNode(g.ptr, labels, properties)
}

// DeleteNode removes a node and all connected relationships from the graph.
func (g *Graph) DeleteNode(nodeID uint64) error {
	if g.ptr == 0 {
		return errors.New("graph is closed")
	}
	return callDeleteNode(g.ptr, nodeID)
}

// CreateEdge creates a directed relationship between source and target nodes.
func (g *Graph) CreateEdge(source, target uint64, relType string, properties map[string]interface{}) (uint64, error) {
	if g.ptr == 0 {
		return 0, errors.New("graph is closed")
	}
	return callCreateEdge(g.ptr, source, target, relType, properties)
}

// DeleteEdge removes a relationship from the graph by ID.
func (g *Graph) DeleteEdge(edgeID uint64) error {
	if g.ptr == 0 {
		return errors.New("graph is closed")
	}
	return callDeleteEdge(g.ptr, edgeID)
}

// Cypher executes an openCypher query string against the graph and returns tabular results.
func (g *Graph) Cypher(query string) (*QueryResult, error) {
	if g.ptr == 0 {
		return nil, errors.New("graph is closed")
	}
	return callCypher(g.ptr, query)
}

// CreateVectorIndex initializes an in-memory vector index for semantic nearest-neighbor search.
func (g *Graph) CreateVectorIndex(name, label, propKey string, dimension int, metric VectorSimilarity) error {
	if g.ptr == 0 {
		return errors.New("graph is closed")
	}
	return callCreateVectorIndex(g.ptr, name, label, propKey, dimension, metric)
}

// QueryVectorIndex searches for the top-k nearest neighbors matching the query vector.
func (g *Graph) QueryVectorIndex(name string, k int, queryVector []float32) ([]VectorHit, error) {
	if g.ptr == 0 {
		return nil, errors.New("graph is closed")
	}
	if len(queryVector) == 0 {
		return nil, errors.New("query vector cannot be empty")
	}
	return callQueryVectorIndex(g.ptr, name, k, queryVector)
}

// ToJSON serializes the entire graph topology, labels, and properties into a JSON dump string.
func (g *Graph) ToJSON() (string, error) {
	if g.ptr == 0 {
		return "", errors.New("graph is closed")
	}
	return callToJSON(g.ptr)
}

// FromJSON deserializes a graph from a JSON dump string.
func FromJSON(jsonStr string) (*Graph, error) {
	ptr, err := callFromJSON(jsonStr)
	if err != nil {
		return nil, err
	}
	g := &Graph{ptr: ptr}
	runtime.SetFinalizer(g, (*Graph).Close)
	return g, nil
}
