//go:build windows

package rustygraph

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"syscall"
	"unsafe"
)

var (
	nativeMu              sync.RWMutex
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
	loadLibrary(dllPath)
}

func loadLibrary(libPath string) {
	nativeMu.Lock()
	defer nativeMu.Unlock()

	dll = syscall.NewLazyDLL(libPath)
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

// setLibraryPathPlatform sets a custom DLL path at runtime on Windows.
func setLibraryPathPlatform(path string) error {
	if _, err := os.Stat(path); err != nil {
		return fmt.Errorf("rusty_graph library not found at: %s", path)
	}
	loadLibrary(path)
	return nil
}

func findDLL() string {
	libName := "rusty_graph.dll"

	if envPath := os.Getenv("RUSTY_GRAPH_LIB"); envPath != "" {
		if _, err := os.Stat(envPath); err == nil {
			return envPath
		}
	}

	// 1. Check directory of the running executable
	if exePath, err := os.Executable(); err == nil {
		exeDir := filepath.Dir(exePath)
		candidates := []string{
			filepath.Join(exeDir, libName),
			filepath.Join(exeDir, "target", "debug", libName),
			filepath.Join(exeDir, "target", "release", libName),
			filepath.Join(exeDir, "..", "target", "debug", libName),
			filepath.Join(exeDir, "..", "target", "release", libName),
			filepath.Join(exeDir, "..", "..", "target", "debug", libName),
		}
		for _, c := range candidates {
			if _, err := os.Stat(c); err == nil {
				return c
			}
		}
	}

	// 2. Search climbing up from the current working directory
	if cwd, err := os.Getwd(); err == nil {
		dir := cwd
		for i := 0; i < 6; i++ {
			debugPath := filepath.Join(dir, "target", "debug", libName)
			if _, err := os.Stat(debugPath); err == nil {
				return debugPath
			}
			releasePath := filepath.Join(dir, "target", "release", libName)
			if _, err := os.Stat(releasePath); err == nil {
				return releasePath
			}
			parent := filepath.Dir(dir)
			if parent == dir {
				break
			}
			dir = parent
		}
	}

	return libName
}

func callNew() (uintptr, error) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	r, _, err := procNew.Call()
	if r == 0 {
		return 0, fmt.Errorf("failed to allocate native Graph instance: %v", err)
	}
	return r, nil
}

func callFree(ptr uintptr) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	procFree.Call(ptr)
}

func callNodeCount(ptr uintptr) uint64 {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	r, _, _ := procNodeCount.Call(ptr)
	return uint64(r)
}

func callEdgeCount(ptr uintptr) uint64 {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	r, _, _ := procEdgeCount.Call(ptr)
	return uint64(r)
}

func callCreateNode(ptr uintptr, labels []string, properties map[string]interface{}) (uint64, error) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	if labels == nil {
		labels = []string{}
	}
	if properties == nil {
		properties = map[string]interface{}{}
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

	r, _, _ := procCreateNode.Call(ptr, uintptr(unsafe.Pointer(cLabels)), uintptr(unsafe.Pointer(cProps)))
	nodeID := uint64(r)
	if nodeID == 0 {
		return 0, errors.New("failed to create node in graph")
	}
	return nodeID, nil
}

func callDeleteNode(ptr uintptr, nodeID uint64) error {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	r, _, _ := procDeleteNode.Call(ptr, uintptr(nodeID))
	if int32(r) != 0 {
		return fmt.Errorf("node %d not found or failed to delete", nodeID)
	}
	return nil
}

func callCreateEdge(ptr uintptr, source, target uint64, relType string, properties map[string]interface{}) (uint64, error) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	if properties == nil {
		properties = map[string]interface{}{}
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
		ptr,
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

func callDeleteEdge(ptr uintptr, edgeID uint64) error {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	r, _, _ := procDeleteEdge.Call(ptr, uintptr(edgeID))
	if int32(r) != 0 {
		return fmt.Errorf("edge %d not found or failed to delete", edgeID)
	}
	return nil
}

func callCypher(ptr uintptr, query string) (*QueryResult, error) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	cQuery, err := syscall.BytePtrFromString(query)
	if err != nil {
		return nil, err
	}

	var outPtr *byte
	r, _, _ := procCypher.Call(
		ptr,
		uintptr(unsafe.Pointer(cQuery)),
		uintptr(unsafe.Pointer(&outPtr)),
	)
	if int32(r) != 0 || outPtr == nil {
		return nil, errors.New("null response pointer returned from cypher engine")
	}
	defer procStringFree.Call(uintptr(unsafe.Pointer(outPtr)))

	jsonBytes := copyCString(outPtr)
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

func callCreateVectorIndex(ptr uintptr, name, label, propKey string, dimension int, metric VectorSimilarity) error {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

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
		ptr,
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

func callQueryVectorIndex(ptr uintptr, name string, k int, queryVector []float32) ([]VectorHit, error) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	cName, err := syscall.BytePtrFromString(name)
	if err != nil {
		return nil, err
	}

	var outPtr *byte
	r, _, _ := procQueryVectorIndex.Call(
		ptr,
		uintptr(unsafe.Pointer(cName)),
		uintptr(k),
		uintptr(unsafe.Pointer(&queryVector[0])),
		uintptr(len(queryVector)),
		uintptr(unsafe.Pointer(&outPtr)),
	)
	runtime.KeepAlive(queryVector)

	if int32(r) != 0 || outPtr == nil {
		return nil, errors.New("null response pointer returned from vector search")
	}
	defer procStringFree.Call(uintptr(unsafe.Pointer(outPtr)))

	jsonBytes := copyCString(outPtr)
	var hits []VectorHit
	if err := json.Unmarshal(jsonBytes, &hits); err != nil {
		return nil, fmt.Errorf("failed to parse vector search response json: %w", err)
	}

	return hits, nil
}

func callToJSON(ptr uintptr) (string, error) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	var outPtr *byte
	r, _, _ := procToJSON.Call(ptr, uintptr(unsafe.Pointer(&outPtr)))
	if int32(r) != 0 || outPtr == nil {
		return "", errors.New("failed to serialize graph to json")
	}
	defer procStringFree.Call(uintptr(unsafe.Pointer(outPtr)))

	return string(copyCString(outPtr)), nil
}

func callFromJSON(jsonStr string) (uintptr, error) {
	nativeMu.RLock()
	defer nativeMu.RUnlock()

	cStr, err := syscall.BytePtrFromString(jsonStr)
	if err != nil {
		return 0, err
	}
	r, _, _ := procFromJSON.Call(uintptr(unsafe.Pointer(cStr)))
	if r == 0 {
		return 0, errors.New("failed to deserialize graph from json")
	}
	return r, nil
}

func copyCString(p *byte) []byte {
	if p == nil {
		return nil
	}
	s := (*[1 << 30]byte)(unsafe.Pointer(p))
	var length int
	for s[length] != 0 {
		length++
	}
	if length == 0 {
		return []byte{}
	}
	dst := make([]byte, length)
	copy(dst, s[:length])
	return dst
}
