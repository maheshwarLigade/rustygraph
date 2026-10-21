use super::ast::{AstQuery, CompOp, ReturnKind};
use super::result::CypherResult;
use crate::error::{GraphError, Result};
use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::Direction;
use crate::value::PropertyValue;
use std::collections::{BTreeMap, HashMap};

struct MatchBinding {
    nodes: HashMap<String, NodeId>,
    edges: HashMap<String, crate::id::RelId>,
}

/// Executes a parsed Cypher AST against a mutable Graph instance.
pub fn execute_ast(graph: &mut Graph, ast: AstQuery) -> Result<CypherResult> {
    // Keep track of aliases created or matched to node IDs
    let mut alias_node_map: HashMap<String, NodeId> = HashMap::new();

    // 1. Handle standalone or pre-match CREATE patterns
    for (nodes, rels) in &ast.creates {
        let mut pattern_node_ids = Vec::new();
        for n in nodes {
            let node_id = graph.create_node_with(|node| {
                if let Some(lbl) = &n.label {
                    node.add_label(lbl);
                }
                for (k, v) in &n.props {
                    node.set_prop(k, v.clone());
                }
            })?;
            if !n.alias.is_empty() {
                alias_node_map.insert(n.alias.clone(), node_id);
            }
            pattern_node_ids.push(node_id);
        }

        for (i, rel) in rels.iter().enumerate() {
            let src = pattern_node_ids[i];
            let dst = pattern_node_ids[i + 1];
            let rel_type = rel.rel_type.as_deref().unwrap_or("RELATED");
            graph.create_edge_with(src, dst, rel_type, |edge| {
                for (k, v) in &rel.props {
                    edge.set_prop(k, v.clone());
                }
            })?;
        }
    }

    // If purely a CREATE query with no MATCH and no RETURN
    if ast.matches.is_empty() && ast.returns.is_empty() {
        let columns = vec!["result".to_string()];
        let rows = vec![vec![PropertyValue::String(format!(
            "Created {} nodes",
            alias_node_map.len()
        ))]];
        return Ok(CypherResult::new(columns, rows));
    }

    // If CREATE with RETURN
    if ast.matches.is_empty() && !ast.returns.is_empty() {
        let columns: Vec<String> = ast.returns.iter().map(|r| r.output_name.clone()).collect();
        let mut row = Vec::new();
        for item in &ast.returns {
            match &item.kind {
                ReturnKind::Property(alias, prop) => {
                    let val = alias_node_map
                        .get(alias)
                        .and_then(|id| graph.get_node_property(*id, prop))
                        .cloned()
                        .unwrap_or(PropertyValue::Null);
                    row.push(val);
                }
                ReturnKind::Node(alias) => {
                    if let Some(id) = alias_node_map.get(alias) {
                        if let Some(node) = graph.get_node(*id) {
                            let mut map = BTreeMap::new();
                            map.insert("id".to_string(), PropertyValue::Int(node.id.raw() as i64));
                            for (k, v) in node.properties.iter() {
                                map.insert(k.clone(), v.clone());
                            }
                            row.push(PropertyValue::Map(map));
                        } else {
                            row.push(PropertyValue::Null);
                        }
                    } else {
                        row.push(PropertyValue::Null);
                    }
                }
                ReturnKind::CountStar | ReturnKind::Count(_, _) => {
                    row.push(PropertyValue::Int(alias_node_map.len() as i64));
                }
                _ => row.push(PropertyValue::Null),
            }
        }
        return Ok(CypherResult::new(columns, vec![row]));
    }

    // 2. Process MATCH queries
    let (pattern_nodes, pattern_rels) = &ast.matches[0];
    let mut builder = graph.query();

    if let Some(first_node) = pattern_nodes.first() {
        builder = builder.match_node(&first_node.alias);
        if let Some(lbl) = &first_node.label {
            builder = builder.with_label(lbl);
        }
        for (k, v) in &first_node.props {
            builder = builder.with_prop(k, v.clone());
        }
    }

    for (i, rel) in pattern_rels.iter().enumerate() {
        let target_node = &pattern_nodes[i + 1];
        builder = builder.rel(
            rel.alias.clone(),
            rel.rel_type.clone(),
            rel.direction,
            target_node.alias.clone(),
        );
        if let Some(lbl) = &target_node.label {
            builder = builder.with_label(lbl);
        }
        for (k, v) in &target_node.props {
            builder = builder.with_prop(k, v.clone());
        }
    }

    // Apply WHERE clauses
    for w in ast.where_clauses {
        let alias = w.alias.clone();
        let prop = w.prop.clone();
        let op = w.op.clone();
        let expected_val = w.val.clone();

        builder = builder.filter(move |ctx| {
            if let Some(actual) = ctx.prop(&alias, &prop) {
                match op {
                    CompOp::Eq => actual == &expected_val,
                    CompOp::Neq => actual != &expected_val,
                    CompOp::Lt => actual < &expected_val,
                    CompOp::Lte => actual <= &expected_val,
                    CompOp::Gt => actual > &expected_val,
                    CompOp::Gte => actual >= &expected_val,
                }
            } else {
                false
            }
        });
    }

    let bindings: Vec<MatchBinding> = builder
        .execute()
        .into_iter()
        .map(|ctx| MatchBinding {
            nodes: ctx.node_bindings().clone(),
            edges: ctx.edge_bindings().clone(),
        })
        .collect();

    // 3. Process SET clauses on matched entities
    if !ast.set_clauses.is_empty() {
        for b in &bindings {
            for s in &ast.set_clauses {
                if let Some(&node_id) = b.nodes.get(&s.alias) {
                    let _ = graph.set_node_property(node_id, &s.prop, s.val.clone());
                }
            }
        }
    }

    // 4. Process DELETE & DETACH DELETE clauses
    if !ast.delete_clauses.is_empty() {
        let mut nodes_to_delete = Vec::new();
        for b in &bindings {
            for d in &ast.delete_clauses {
                if let Some(&node_id) = b.nodes.get(&d.alias) {
                    nodes_to_delete.push((node_id, d.detach));
                }
            }
        }

        for (node_id, detach) in nodes_to_delete {
            if detach {
                let _ = graph.delete_node(node_id);
            } else {
                let deg = graph.degree(node_id, Direction::Both)?;
                if deg > 0 {
                    return Err(GraphError::InvalidOperation(format!(
                        "Cannot delete node {} because it still has {} relationships; use DETACH DELETE",
                        node_id.0, deg
                    )));
                }
                graph.delete_node(node_id)?;
            }
        }
    }

    // If query was purely a mutation (SET/DELETE) without RETURN
    if ast.returns.is_empty() {
        let columns = vec!["result".to_string()];
        let rows = vec![vec![PropertyValue::String(format!(
            "Updated {} matched entities",
            bindings.len()
        ))]];
        return Ok(CypherResult::new(columns, rows));
    }

    // Helper closures for reading properties / nodes post-mutation
    let get_prop = |b: &MatchBinding, alias: &str, prop: &str| -> Option<PropertyValue> {
        if let Some(&nid) = b.nodes.get(alias) {
            graph.get_node(nid).and_then(|n| n.prop_path(prop)).cloned()
        } else if let Some(&eid) = b.edges.get(alias) {
            graph.get_edge(eid).and_then(|e| e.prop_path(prop)).cloned()
        } else {
            None
        }
    };

    let get_node_val = |b: &MatchBinding, alias: &str| -> PropertyValue {
        if let Some(&nid) = b.nodes.get(alias) {
            if let Some(n) = graph.get_node(nid) {
                let mut map = BTreeMap::new();
                map.insert("id".to_string(), PropertyValue::Int(n.id.raw() as i64));
                for (k, v) in n.properties.iter() {
                    map.insert(k.clone(), v.clone());
                }
                PropertyValue::Map(map)
            } else {
                PropertyValue::Null
            }
        } else {
            PropertyValue::Null
        }
    };

    // 5. Check if query contains aggregation functions
    let has_aggregations = ast.returns.iter().any(|r| {
        matches!(
            r.kind,
            ReturnKind::CountStar
                | ReturnKind::Count(_, _)
                | ReturnKind::Sum(_, _)
                | ReturnKind::Avg(_, _)
                | ReturnKind::Min(_, _)
                | ReturnKind::Max(_, _)
        )
    });

    let columns: Vec<String> = ast.returns.iter().map(|r| r.output_name.clone()).collect();

    if has_aggregations {
        let mut row = Vec::new();
        for item in &ast.returns {
            match &item.kind {
                ReturnKind::CountStar => {
                    row.push(PropertyValue::Int(bindings.len() as i64));
                }
                ReturnKind::Count(alias, prop) => {
                    let mut cnt = 0;
                    for b in &bindings {
                        if let Some(p) = prop {
                            if get_prop(b, alias, p).is_some() {
                                cnt += 1;
                            }
                        } else if b.nodes.contains_key(alias) {
                            cnt += 1;
                        }
                    }
                    row.push(PropertyValue::Int(cnt));
                }
                ReturnKind::Sum(alias, prop) => {
                    let mut sum = 0.0f64;
                    for b in &bindings {
                        if let Some(v) = get_prop(b, alias, prop) {
                            if let Some(n) = v.as_f64() {
                                sum += n;
                            }
                        }
                    }
                    row.push(PropertyValue::Float(sum));
                }
                ReturnKind::Avg(alias, prop) => {
                    let mut sum = 0.0f64;
                    let mut cnt = 0;
                    for b in &bindings {
                        if let Some(v) = get_prop(b, alias, prop) {
                            if let Some(n) = v.as_f64() {
                                sum += n;
                                cnt += 1;
                            }
                        }
                    }
                    let avg = if cnt > 0 { sum / cnt as f64 } else { 0.0 };
                    row.push(PropertyValue::Float(avg));
                }
                ReturnKind::Min(alias, prop) => {
                    let mut min_val: Option<PropertyValue> = None;
                    for b in &bindings {
                        if let Some(v) = get_prop(b, alias, prop) {
                            match &min_val {
                                None => min_val = Some(v),
                                Some(curr) => {
                                    if &v < curr {
                                        min_val = Some(v);
                                    }
                                }
                            }
                        }
                    }
                    row.push(min_val.unwrap_or(PropertyValue::Null));
                }
                ReturnKind::Max(alias, prop) => {
                    let mut max_val: Option<PropertyValue> = None;
                    for b in &bindings {
                        if let Some(v) = get_prop(b, alias, prop) {
                            match &max_val {
                                None => max_val = Some(v),
                                Some(curr) => {
                                    if &v > curr {
                                        max_val = Some(v);
                                    }
                                }
                            }
                        }
                    }
                    row.push(max_val.unwrap_or(PropertyValue::Null));
                }
                ReturnKind::Property(alias, prop) => {
                    let val = bindings
                        .first()
                        .and_then(|b| get_prop(b, alias, prop))
                        .unwrap_or(PropertyValue::Null);
                    row.push(val);
                }
                ReturnKind::Node(alias) => {
                    let node_val = bindings
                        .first()
                        .map(|b| get_node_val(b, alias))
                        .unwrap_or(PropertyValue::Null);
                    row.push(node_val);
                }
            }
        }
        return Ok(CypherResult::new(columns, vec![row]));
    }

    // Standard Projections (non-aggregated)
    let mut rows: Vec<Vec<PropertyValue>> = Vec::with_capacity(bindings.len());

    for b in &bindings {
        let mut row = Vec::with_capacity(ast.returns.len());
        for item in &ast.returns {
            match &item.kind {
                ReturnKind::Property(alias, prop) => {
                    let val = get_prop(b, alias, prop).unwrap_or(PropertyValue::Null);
                    row.push(val);
                }
                ReturnKind::Node(alias) => {
                    row.push(get_node_val(b, alias));
                }
                _ => row.push(PropertyValue::Null),
            }
        }
        rows.push(row);
    }

    // 6. Handle ORDER BY
    if !ast.order_by.is_empty() {
        let order_spec = ast.order_by.clone();
        let return_spec = ast.returns.clone();

        rows.sort_by(|row_a, row_b| {
            for order in &order_spec {
                // Locate column index
                let col_idx = return_spec.iter().position(|r| match &r.kind {
                    ReturnKind::Property(a, p) => {
                        a == &order.alias && order.prop.as_deref() == Some(p)
                    }
                    ReturnKind::Node(a) => a == &order.alias && order.prop.is_none(),
                    _ => false,
                });

                if let Some(idx) = col_idx {
                    let val_a = &row_a[idx];
                    let val_b = &row_b[idx];
                    let cmp = val_a.cmp(val_b);
                    if cmp != std::cmp::Ordering::Equal {
                        return if order.descending {
                            cmp.reverse()
                        } else {
                            cmp
                        };
                    }
                }
            }
            std::cmp::Ordering::Equal
        });
    }

    // 7. Handle LIMIT
    if let Some(lim) = ast.limit {
        if rows.len() > lim {
            rows.truncate(lim);
        }
    }

    Ok(CypherResult::new(columns, rows))
}
