package com.rustygraph;

import java.util.List;

/**
 * Result of a Cypher query execution.
 */
public record CypherResult(
    boolean success,
    List<String> columns,
    List<List<Object>> rows,
    String error
) {
    public boolean hasRows() {
        return rows != null && !rows.isEmpty();
    }

    public int rowCount() {
        return rows == null ? 0 : rows.size();
    }
}
