package com.rustygraph;

import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.*;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * High-performance Java SDK for the in-memory rusty_graph database engine.
 * Powered by Java 21's Foreign Function & Memory (FFM) API.
 */
public class RustyGraph implements AutoCloseable {
    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP;

    private static final MethodHandle RUSTY_GRAPH_NEW;
    private static final MethodHandle RUSTY_GRAPH_FREE;
    private static final MethodHandle RUSTY_GRAPH_STRING_FREE;
    private static final MethodHandle RUSTY_GRAPH_NODE_COUNT;
    private static final MethodHandle RUSTY_GRAPH_EDGE_COUNT;
    private static final MethodHandle RUSTY_GRAPH_CREATE_NODE;
    private static final MethodHandle RUSTY_GRAPH_DELETE_NODE;
    private static final MethodHandle RUSTY_GRAPH_CREATE_EDGE;
    private static final MethodHandle RUSTY_GRAPH_DELETE_EDGE;
    private static final MethodHandle RUSTY_GRAPH_CYPHER;
    private static final MethodHandle RUSTY_GRAPH_CREATE_VECTOR_INDEX;
    private static final MethodHandle RUSTY_GRAPH_QUERY_VECTOR_INDEX;
    private static final MethodHandle RUSTY_GRAPH_TO_JSON;
    private static final MethodHandle RUSTY_GRAPH_FROM_JSON;

    static {
        Path libPath = findLibrary();
        LOOKUP = SymbolLookup.libraryLookup(libPath, Arena.global());

        RUSTY_GRAPH_NEW = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_new").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_FREE = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_free").orElseThrow(),
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_STRING_FREE = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_string_free").orElseThrow(),
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_NODE_COUNT = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_node_count").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_EDGE_COUNT = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_edge_count").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_CREATE_NODE = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_create_node").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_DELETE_NODE = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_delete_node").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG)
        );
        RUSTY_GRAPH_CREATE_EDGE = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_create_edge").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_DELETE_EDGE = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_delete_edge").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG)
        );
        RUSTY_GRAPH_CYPHER = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_cypher").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_CREATE_VECTOR_INDEX = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_create_vector_index").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT)
        );
        RUSTY_GRAPH_QUERY_VECTOR_INDEX = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_query_vector_index").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_TO_JSON = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_to_json").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS)
        );
        RUSTY_GRAPH_FROM_JSON = LINKER.downcallHandle(
            LOOKUP.find("rusty_graph_from_json").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)
        );
    }

    private MemorySegment graphPtr;

    private RustyGraph(MemorySegment graphPtr) {
        this.graphPtr = graphPtr;
    }

    /**
     * Create a new in-memory Graph instance.
     */
    public static RustyGraph create() {
        try {
            MemorySegment ptr = (MemorySegment) RUSTY_GRAPH_NEW.invokeExact();
            if (ptr.equals(MemorySegment.NULL)) {
                throw new IllegalStateException("Failed to allocate native Graph");
            }
            return new RustyGraph(ptr);
        } catch (Throwable t) {
            throw new RuntimeException("Error initializing native Graph", t);
        }
    }

    @Override
    public void close() {
        if (graphPtr != null && !graphPtr.equals(MemorySegment.NULL)) {
            try {
                RUSTY_GRAPH_FREE.invokeExact(graphPtr);
            } catch (Throwable ignored) {
            } finally {
                graphPtr = MemorySegment.NULL;
            }
        }
    }

    private void checkOpen() {
        if (graphPtr == null || graphPtr.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("RustyGraph is closed");
        }
    }

    /**
     * Returns total number of active nodes in the graph.
     */
    public long nodeCount() {
        checkOpen();
        try {
            return (long) RUSTY_GRAPH_NODE_COUNT.invokeExact(graphPtr);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns total number of active edges in the graph.
     */
    public long edgeCount() {
        checkOpen();
        try {
            return (long) RUSTY_GRAPH_EDGE_COUNT.invokeExact(graphPtr);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Create a new node with optional labels and key-value properties.
     */
    public long createNode(List<String> labels, Map<String, Object> properties) {
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            String labelsJson = toJsonArray(labels == null ? List.of() : labels);
            String propsJson = toJsonObject(properties == null ? Map.of() : properties);

            MemorySegment cLabels = arena.allocateUtf8String(labelsJson);
            MemorySegment cProps = arena.allocateUtf8String(propsJson);

            long nodeId = (long) RUSTY_GRAPH_CREATE_NODE.invokeExact(graphPtr, cLabels, cProps);
            if (nodeId == 0) {
                throw new RuntimeException("Failed to create node in graph");
            }
            return nodeId;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Delete a node by ID.
     */
    public boolean deleteNode(long nodeId) {
        checkOpen();
        try {
            int res = (int) RUSTY_GRAPH_DELETE_NODE.invokeExact(graphPtr, nodeId);
            return res == 0;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Create a directed relationship between source and target nodes.
     */
    public long createEdge(long source, long target, String relType, Map<String, Object> properties) {
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            String propsJson = toJsonObject(properties == null ? Map.of() : properties);

            MemorySegment cType = arena.allocateUtf8String(relType);
            MemorySegment cProps = arena.allocateUtf8String(propsJson);

            long relId = (long) RUSTY_GRAPH_CREATE_EDGE.invokeExact(graphPtr, source, target, cType, cProps);
            if (relId == 0) {
                throw new RuntimeException("Failed to create edge in graph");
            }
            return relId;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Delete a relationship by ID.
     */
    public boolean deleteEdge(long relId) {
        checkOpen();
        try {
            int res = (int) RUSTY_GRAPH_DELETE_EDGE.invokeExact(graphPtr, relId);
            return res == 0;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Execute a Cypher query string against the graph.
     */
    public CypherResult cypher(String query) {
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cQuery = arena.allocateUtf8String(query);
            MemorySegment outPtr = arena.allocate(ValueLayout.ADDRESS);
            int res = (int) RUSTY_GRAPH_CYPHER.invokeExact(graphPtr, cQuery, outPtr);
            if (res != 0) {
                return new CypherResult(false, List.of(), List.of(), "Cypher execution error");
            }
            MemorySegment resultPtr = outPtr.get(ValueLayout.ADDRESS, 0);
            if (resultPtr.equals(MemorySegment.NULL)) {
                return new CypherResult(false, List.of(), List.of(), "Null result pointer from engine");
            }

            try {
                String json = readCString(resultPtr);
                return parseCypherResult(json);
            } finally {
                RUSTY_GRAPH_STRING_FREE.invokeExact(resultPtr);
            }
        } catch (Throwable t) {
            throw new RuntimeException("Cypher execution failed", t);
        }
    }

    /**
     * Create a vector index on the graph.
     */
    public void createVectorIndex(String name, String label, String propKey, int dimension, VectorSimilarity similarity) {
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cName = arena.allocateUtf8String(name);
            MemorySegment cLabel = arena.allocateUtf8String(label);
            MemorySegment cProp = arena.allocateUtf8String(propKey);

            int res = (int) RUSTY_GRAPH_CREATE_VECTOR_INDEX.invokeExact(
                graphPtr, cName, cLabel, cProp, (long) dimension, similarity.getValue()
            );

            if (res != 0) {
                throw new RuntimeException("Failed to create vector index '" + name + "'");
            }
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Query vector index for top-k nearest neighbors.
     */
    public List<VectorHit> queryVectorIndex(String name, int k, float[] queryVector) {
        checkOpen();
        if (queryVector == null || queryVector.length == 0) {
            return List.of();
        }

        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cName = arena.allocateUtf8String(name);
            MemorySegment vecSeg = arena.allocate((long) queryVector.length * Float.BYTES);
            for (int i = 0; i < queryVector.length; i++) {
                vecSeg.setAtIndex(ValueLayout.JAVA_FLOAT, i, queryVector[i]);
            }
            MemorySegment outPtr = arena.allocate(ValueLayout.ADDRESS);

            int res = (int) RUSTY_GRAPH_QUERY_VECTOR_INDEX.invokeExact(
                graphPtr, cName, (long) k, vecSeg, (long) queryVector.length, outPtr
            );

            if (res != 0) {
                return List.of();
            }

            MemorySegment resultPtr = outPtr.get(ValueLayout.ADDRESS, 0);
            if (resultPtr.equals(MemorySegment.NULL)) {
                return List.of();
            }

            try {
                String json = readCString(resultPtr);
                return parseVectorHits(json);
            } finally {
                RUSTY_GRAPH_STRING_FREE.invokeExact(resultPtr);
            }
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Export the entire graph to a JSON dump string.
     */
    public String toJson() {
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment outPtr = arena.allocate(ValueLayout.ADDRESS);
            int res = (int) RUSTY_GRAPH_TO_JSON.invokeExact(graphPtr, outPtr);
            if (res != 0) {
                throw new RuntimeException("Failed to serialize graph to JSON");
            }
            MemorySegment strPtr = outPtr.get(ValueLayout.ADDRESS, 0);
            if (strPtr.equals(MemorySegment.NULL)) {
                throw new RuntimeException("Failed to serialize graph to JSON");
            }
            try {
                return readCString(strPtr);
            } finally {
                RUSTY_GRAPH_STRING_FREE.invokeExact(strPtr);
            }
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    private static String readCString(MemorySegment ptr) {
        if (ptr == null || ptr.equals(MemorySegment.NULL)) {
            return "";
        }
        return ptr.reinterpret(Integer.MAX_VALUE).getUtf8String(0);
    }

    /**
     * Deserialize a graph from a JSON dump string.
     */
    public static RustyGraph fromJson(String jsonStr) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cStr = arena.allocateUtf8String(jsonStr);
            MemorySegment ptr = (MemorySegment) RUSTY_GRAPH_FROM_JSON.invokeExact(cStr);
            if (ptr.equals(MemorySegment.NULL)) {
                throw new RuntimeException("Failed to deserialize graph from JSON");
            }
            return new RustyGraph(ptr);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    // =========================================================================
    // Helpers: Dynamic Library Discovery & Lightweight JSON Formatting
    // =========================================================================

    private static Path findLibrary() {
        String os = System.getProperty("os.name").toLowerCase();
        String libName;
        if (os.contains("win")) {
            libName = "rusty_graph.dll";
        } else if (os.contains("mac")) {
            libName = "librusty_graph.dylib";
        } else {
            libName = "librusty_graph.so";
        }

        String env = System.getenv("RUSTY_GRAPH_LIB");
        if (env != null && !env.isBlank()) {
            Path p = Paths.get(env);
            if (Files.exists(p)) return p.toAbsolutePath();
        }

        List<Path> candidates = List.of(
            Paths.get(libName),
            Paths.get("target", "debug", libName),
            Paths.get("target", "release", libName),
            Paths.get("..", "target", "debug", libName),
            Paths.get("..", "..", "target", "debug", libName),
            Paths.get("..", "..", "..", "target", "debug", libName),
            Paths.get("..", "..", "..", "target", "release", libName)
        );

        for (Path c : candidates) {
            if (Files.exists(c)) {
                return c.toAbsolutePath();
            }
        }

        return Paths.get(libName).toAbsolutePath();
    }

    private static String toJsonArray(List<String> list) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < list.size(); i++) {
            if (i > 0) sb.append(",");
            sb.append("\"").append(escape(list.get(i))).append("\"");
        }
        sb.append("]");
        return sb.toString();
    }

    private static String toJsonObject(Map<String, Object> map) {
        StringBuilder sb = new StringBuilder("{");
        int i = 0;
        for (Map.Entry<String, Object> entry : map.entrySet()) {
            if (i > 0) sb.append(",");
            sb.append("\"").append(escape(entry.getKey())).append("\":");
            Object v = entry.getValue();
            if (v == null) {
                sb.append("null");
            } else if (v instanceof String s) {
                sb.append("\"").append(escape(s)).append("\"");
            } else if (v instanceof Number || v instanceof Boolean) {
                sb.append(v);
            } else if (v instanceof float[] fa) {
                sb.append("[");
                for (int j = 0; j < fa.length; j++) {
                    if (j > 0) sb.append(",");
                    sb.append(fa[j]);
                }
                sb.append("]");
            } else if (v instanceof List<?> l) {
                sb.append("[");
                for (int j = 0; j < l.size(); j++) {
                    if (j > 0) sb.append(",");
                    sb.append(l.get(j));
                }
                sb.append("]");
            } else {
                sb.append("\"").append(escape(v.toString())).append("\"");
            }
            i++;
        }
        sb.append("}");
        return sb.toString();
    }

    private static String escape(String s) {
        return s.replace("\\", "\\\\").replace("\"", "\\\"");
    }

    private static CypherResult parseCypherResult(String json) {
        boolean success = json.contains("\"success\":true");
        String error = null;
        if (json.contains("\"error\":") && !json.contains("\"error\":null")) {
            Matcher m = Pattern.compile("\"error\":\"([^\"]+)\"").matcher(json);
            if (m.find()) {
                error = m.group(1);
            }
        }

        List<String> columns = new ArrayList<>();
        Matcher colMatcher = Pattern.compile("\"columns\":\\[(.*?)\\]").matcher(json);
        if (colMatcher.find()) {
            String colStr = colMatcher.group(1);
            if (!colStr.isBlank()) {
                for (String c : colStr.split(",")) {
                    columns.add(c.trim().replace("\"", ""));
                }
            }
        }

        List<List<Object>> rows = new ArrayList<>();
        Matcher rowMatcher = Pattern.compile("\\[([^\\[\\]]+)\\]").matcher(json.substring(json.indexOf("\"rows\":")));
        while (rowMatcher.find()) {
            String rowStr = rowMatcher.group(1);
            List<Object> row = new ArrayList<>();
            for (String item : rowStr.split(",")) {
                item = item.trim().replace("\"", "");
                row.add(item);
            }
            rows.add(row);
        }

        return new CypherResult(success, columns, rows, error);
    }

    private static List<VectorHit> parseVectorHits(String json) {
        List<VectorHit> hits = new ArrayList<>();
        Matcher m = Pattern.compile("\"node_id\":(\\d+),\"score\":([\\d\\.]+)").matcher(json);
        while (m.find()) {
            long nodeId = Long.parseLong(m.group(1));
            float score = Float.parseFloat(m.group(2));
            hits.add(new VectorHit(nodeId, score));
        }
        return hits;
    }
}
