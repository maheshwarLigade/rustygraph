// Package rustygraph provides high-performance in-memory property graph database bindings
// and native Cypher query execution for Go applications, backed by the rusty_graph Rust core.
package rustygraph

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"syscall"
	"unsafe"
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

var (
	dll                   *syscall.LazyDLL
	procNew               *syscall.LazyProc
	procFree              *syscall.LazyProc
	procStringFree        *syscall.LazyProc
	procNodeCount         *syscall.LazyProc
	procEdgeCount         *syscall.LazyProc
	procCreateNode        *syscall.LazyProc
	procDeleteNode        *syscall.LazyProc
	procCreateEdge        *syscall.LazyProc
	procDeleteEdge        *syscall.LazyProc
	procCypher            *syscall.LazyProc
	procCreateVectorIndex *syscall.LazyProc
	procQueryVectorIndex  *syscall.LazyProc
	procToJSON            *syscall.LazyProc
	procFromJSON          *syscall.LazyProc
)

func init() {
	dllPath := findDLL()
	dll = syscall.NewLazyDLL(dllPath)

	procNew = dll.NewProc("rusty_graph_new")
	procFree = dll.NewProc("rusty_graph_free")
	procStringFree = dll.NewProc("rusty_graph_string_free")
	procNodeCount = dll.NewProc("rusty_graph_node_count")
	procEdgeCount = dll.NewProc("rusty_graph_edge_count")
	procCreateNode = dll.NewProc("rusty_graph_create_node")
	procDeleteNode = dll.NewProc("rusty_graph_delete_node")
	procCreateEdge = dll.NewProc("rusty_graph_create_edge")
	procDeleteEdge = dll.NewProc("rusty_graph_delete_edge")
	procCypher = dll.NewProc("rusty_graph_cypher")
	procCreateVectorIndex = dll.NewProc("rusty_graph_create_vector_index")
	procQueryVectorIndex = dll.NewProc("rusty_graph_query_vector_index")
	procToJSON = dll.NewProc("rusty_graph_to_json")
	procFromJSON = dll.NewProc("rusty_graph_from_json")
}

func findDLL() string {
	var libName string
	if runtime.GOOS == "windows" {
		libName = "rusty_graph.dll"
	} else if runtime.GOOS == "darwin" {
		libName = "librusty_graph.dylib"
	} else {
		libName = "librusty_graph.so"
	}

	if envPath := os.Getenv("RUSTY_GRAPH_LIB"); envPath != "" {
		if _, err := os.Stat(envPath); err == nil {
			return envPath
		}
	}

	// Check relative target directory paths
	candidates := []string{
		libName,
		filepath.Join(".", libName),
		filepath.Join("..", "..", "..", "target", "debug", libName),
		filepath.Join("..", "..", "..", "target", "release", libName),
		filepath.Join("..", "..", "target", "debug", libName),
		filepath.Join("..", "..", "target", "release", libName),
		filepath.Join("..", "target", "debug", libName),
		filepath.Join("target", "debug", libName),
		filepath.Join("target", "release", libName),
	}

	for _, p := range candidates {
		if _, err := os.Stat(p); err == nil {
			abs, err := filepath.Abs(p)
			if err == nil {
				return abs
			}
			return p
		}
	}

	return libName
}

// Graph represents an in-memory property graph database instance in Go.
type Graph struct {
	ptr uintptr
}

// New creates a new in-memory Graph instance.
func New() (*Graph, error) {
	r, _, err := procNew.Call()
	if r == 0 {
		return nil, fmt.Errorf("failed to allocate rusty_graph instance: %v", err)
	}
	g := &Graph{ptr: r}
	runtime.SetFinalizer(g, (*Graph).Close)
	return g, nil
}

// Close explicitly frees the native Graph memory in Rust.
func (g *Graph) Close() {
	if g.ptr != 0 {
		procFree.Call(g.ptr)
		g.ptr = 0
	}
}

// NodeCount returns the number of active nodes in the graph.
func (g *Graph) NodeCount() uint64 {
	if g.ptr == 0 {
		return 0
	}
	r, _, _ := procNodeCount.Call(g.ptr)
	return uint64(r)
}

// EdgeCount returns the number of active relationships in the graph.
func (g *Graph) EdgeCount() uint64 {
	if g.ptr == 0 {
		return 0
	}
	r, _, _ := procEdgeCount.Call(g.ptr)
	return uint64(r)
}

// CreateNode creates a new node with optional labels and key-value properties.
func (g *Graph) CreateNode(labels []string, properties map[string]interface{}) (uint64, error) {
	if g.ptr == 0 {
		return 0, errors.New("graph is closed")
	}

	labelsJSON, err := json.Marshal(labels)
	if err != nil {
		return 0, err
	}
	propsJSON, err := json.Marshal(properties)
	if err != nil {
		return 0, err
	}

	cLabels, err := syscall.BytePtrFromString(string(labelsJSON))
	if err != nil {
		return 0, err
	}
	cProps, err := syscall.BytePtrFromString(string(propsJSON))
	if err != nil {
		return 0, err
	}

	r, _, _ := procCreateNode.Call(g.ptr, uintptr(unsafe.Pointer(cLabels)), uintptr(unsafe.Pointer(cProps)))
	nodeID := uint64(r)
	if nodeID == 0 {
		return 0, errors.New("failed to create node in graph")
	}
	return nodeID, nil
}

// DeleteNode removes a node and all connected relationships from the graph.
func (g *Graph) DeleteNode(nodeID uint64) error {
	if g.ptr == 0 {
		return errors.New("graph is closed")
	}
	r, _, _ := procDeleteNode.Call(g.ptr, uintptr(nodeID))
	if int32(r) != 0 {
		return fmt.Errorf("node %d not found or failed to delete", nodeID)
	}
	return nil
}

// CreateEdge creates a directed relationship between source and target nodes.
func (g *Graph) CreateEdge(source, target uint64, relType string, properties map[string]interface{}) (uint64, error) {
	if g.ptr == 0 {
		return 0, errors.New("graph is closed")
	}

	propsJSON, err := json.Marshal(properties)
	if err != nil {
		return 0, err
	}

	cType, err := syscall.BytePtrFromString(relType)
	if err != nil {
		return 0, err
	}
	cProps, err := syscall.BytePtrFromString(string(propsJSON))
	if err != nil {
		return 0, err
	}

	r, _, _ := procCreateEdge.Call(
		g.ptr,
		uintptr(source),
		uintptr(target),
		uintptr(unsafe.Pointer(cType)),
		uintptr(unsafe.Pointer(cProps)),
	)
	edgeID := uint64(r)
	if edgeID == 0 {
		return 0, fmt.Errorf("failed to create edge from %d to %d", source, target)
	}
	return edgeID, nil
}

// DeleteEdge removes a relationship from the graph by ID.
func (g *Graph) DeleteEdge(edgeID uint64) error {
	if g.ptr == 0 {
		return errors.New("graph is closed")
	}
	r, _, _ := procDeleteEdge.Call(g.ptr, uintptr(edgeID))
	if int32(r) != 0 {
		return fmt.Errorf("edge %d not found or failed to delete", edgeID)
	}
	return nil
}

// Cypher executes a Cypher query string against the graph and returns the tabular result.
func (g *Graph) Cypher(query string) (*QueryResult, error) {
	if g.ptr == 0 {
		return nil, errors.New("graph is closed")
	}

	cQuery, err := syscall.BytePtrFromString(query)
	if err != nil {
		return nil, err
	}

	r, _, _ := procCypher.Call(g.ptr, uintptr(unsafe.Pointer(cQuery)))
	if r == 0 {
		return nil, errors.New("null response pointer returned from cypher engine")
	}
	defer procStringFree.Call(r)

	jsonBytes := copyCString(r)
	var res QueryResult
	if err := json.Unmarshal(jsonBytes, &res); err != nil {
		return nil, fmt.Errorf("failed to parse cypher response json: %w", err)
	}

	if !res.Success {
		errMsg := "unknown cypher error"
		if res.Error != nil {
			errMsg = *res.Error
		}
		return nil, fmt.Errorf("cypher error: %s", errMsg)
	}

	return &res, nil
}

// CreateVectorIndex initializes a vector index for semantic nearest-neighbor search.
func (g *Graph) CreateVectorIndex(name, label, propKey string, dimension int, metric VectorSimilarity) error {
	if g.ptr == 0 {
		return errors.New("graph is closed")
	}

	cName, err := syscall.BytePtrFromString(name)
	if err != nil {
		return err
	}
	cLabel, err := syscall.BytePtrFromString(label)
	if err != nil {
		return err
	}
	cProp, err := syscall.BytePtrFromString(propKey)
	if err != nil {
		return err
	}

	r, _, _ := procCreateVectorIndex.Call(
		g.ptr,
		uintptr(unsafe.Pointer(cName)),
		uintptr(unsafe.Pointer(cLabel)),
		uintptr(unsafe.Pointer(cProp)),
		uintptr(dimension),
		uintptr(metric),
	)

	if int32(r) != 0 {
		return fmt.Errorf("failed to create vector index '%s'", name)
	}
	return nil
}

// QueryVectorIndex searches for the top-k nearest neighbors matching the query vector.
func (g *Graph) QueryVectorIndex(name string, k int, queryVector []float32) ([]VectorHit, error) {
	if g.ptr == 0 {
		return nil, errors.New("graph is closed")
	}
	if len(queryVector) == 0 {
		return nil, errors.New("query vector cannot be empty")
	}

	cName, err := syscall.BytePtrFromString(name)
	if err != nil {
		return nil, err
	}

	vecPtr := unsafe.Pointer(&queryVector[0])

	r, _, _ := procQueryVectorIndex.Call(
		g.ptr,
		uintptr(unsafe.Pointer(cName)),
		uintptr(k),
		uintptr(vecPtr),
		uintptr(len(queryVector)),
	)

	if r == 0 {
		return nil, errors.New("null response pointer returned from vector search")
	}
	defer procStringFree.Call(r)

	jsonBytes := copyCString(r)
	var hits []VectorHit
	if err := json.Unmarshal(jsonBytes, &hits); err != nil {
		return nil, fmt.Errorf("failed to parse vector search response json: %w", err)
	}

	return hits, nil
}

// ToJSON serializes the entire graph topology, labels, and properties into a JSON dump string.
func (g *Graph) ToJSON() (string, error) {
	if g.ptr == 0 {
		return "", errors.New("graph is closed")
	}
	r, _, _ := procToJSON.Call(g.ptr)
	if r == 0 {
		return "", errors.New("failed to serialize graph to json")
	}
	defer procStringFree.Call(r)

	return string(copyCString(r)), nil
}

// FromJSON deserializes a graph from a JSON dump string.
func FromJSON(jsonStr string) (*Graph, error) {
	cStr, err := syscall.BytePtrFromString(jsonStr)
	if err != nil {
		return nil, err
	}
	r, _, _ := procFromJSON.Call(uintptr(unsafe.Pointer(cStr)))
	if r == 0 {
		return nil, errors.New("failed to deserialize graph from json")
	}
	g := &Graph{ptr: r}
	runtime.SetFinalizer(g, (*Graph).Close)
	return g, nil
}

func copyCString(ptr uintptr) []byte {
	var bytes []byte
	p := (*byte)(unsafe.Pointer(ptr))
	for *p != 0 {
		bytes = append(bytes, *p)
		ptr++
		p = (*byte)(unsafe.Pointer(ptr))
	}
	return bytes
}
