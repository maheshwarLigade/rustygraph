//go:build !windows

package rustygraph

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
)

var errUnsupported = errors.New("rustygraph: direct Windows syscall dynamic loading is active; on non-Windows platforms (Linux/Darwin), please link via CGO or configure RUSTY_GRAPH_LIB")

func init() {
	// Discover shared object path for diagnostic info
	_ = findDLL()
}

func setLibraryPathPlatform(path string) error {
	if _, err := os.Stat(path); err != nil {
		return fmt.Errorf("rusty_graph library not found at: %s", path)
	}
	return nil
}

func findDLL() string {
	var libName string
	if runtime.GOOS == "darwin" {
		libName = "librusty_graph.dylib"
	} else {
		libName = "librusty_graph.so"
	}

	if envPath := os.Getenv("RUSTY_GRAPH_LIB"); envPath != "" {
		if _, err := os.Stat(envPath); err == nil {
			return envPath
		}
	}

	if exePath, err := os.Executable(); err == nil {
		exeDir := filepath.Dir(exePath)
		for _, c := range []string{
			filepath.Join(exeDir, libName),
			filepath.Join(exeDir, "target", "debug", libName),
			filepath.Join(exeDir, "target", "release", libName),
			filepath.Join(exeDir, "..", "target", "debug", libName),
			filepath.Join(exeDir, "..", "target", "release", libName),
		} {
			if _, err := os.Stat(c); err == nil {
				return c
			}
		}
	}

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
	return 0, errUnsupported
}

func callFree(ptr uintptr) {}

func callNodeCount(ptr uintptr) uint64 {
	return 0
}

func callEdgeCount(ptr uintptr) uint64 {
	return 0
}

func callCreateNode(ptr uintptr, labels []string, properties map[string]interface{}) (uint64, error) {
	return 0, errUnsupported
}

func callDeleteNode(ptr uintptr, nodeID uint64) error {
	return errUnsupported
}

func callCreateEdge(ptr uintptr, source, target uint64, relType string, properties map[string]interface{}) (uint64, error) {
	return 0, errUnsupported
}

func callDeleteEdge(ptr uintptr, edgeID uint64) error {
	return errUnsupported
}

func callCypher(ptr uintptr, query string) (*QueryResult, error) {
	return nil, errUnsupported
}

func callCreateVectorIndex(ptr uintptr, name, label, propKey string, dimension int, metric VectorSimilarity) error {
	return errUnsupported
}

func callQueryVectorIndex(ptr uintptr, name string, k int, queryVector []float32) ([]VectorHit, error) {
	return nil, errUnsupported
}

func callToJSON(ptr uintptr) (string, error) {
	return "", errUnsupported
}

func callFromJSON(jsonStr string) (uintptr, error) {
	return 0, errUnsupported
}
