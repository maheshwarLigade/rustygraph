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

/// Executes a parsed Cypher AST against a mutable Graph instance with optional parameters.
pub fn execute_ast_with_params(
    graph: &mut Graph,
    ast: AstQuery,
    params: &HashMap<String, PropertyValue>,
) -> Result<CypherResult> {
    // Keep track of aliases created or matched to node and relationship IDs
    let mut alias_node_map: HashMap<String, NodeId> = HashMap::new();
    let mut alias_edge_map: HashMap<String, crate::id::RelId> = HashMap::new();

    // 1. Handle standalone or pre-match CREATE patterns
    for (nodes, rels) in &ast.creates {
        let mut pattern_node_ids = Vec::new();
        for n in nodes {
            let node_id = if !n.alias.is_empty()
                && alias_node_map.contains_key(&n.alias)
                && n.label.is_none()
                && n.props.is_empty()
            {
                alias_node_map[&n.alias]
            } else {
                let mut resolved_props = Vec::with_capacity(n.props.len());
                for (k, v) in &n.props {
                    resolved_props.push((k.clone(), v.resolve(params)?));
                }

                let nid = graph.create_node_with(|node| {
                    if let Some(lbl) = &n.label {
                        node.add_label(lbl);
                    }
                    for (k, val) in resolved_props {
                        node.set_prop(k, val);
                    }
                })?;
                if !n.alias.is_empty() {
                    alias_node_map.insert(n.alias.clone(), nid);
                }
                nid
            };
            pattern_node_ids.push(node_id);
        }

        for (i, rel) in rels.iter().enumerate() {
            let src = pattern_node_ids[i];
            let dst = pattern_node_ids[i + 1];
            let rel_type = rel.rel_type.as_deref().unwrap_or("RELATED");
            let mut resolved_rel_props = Vec::with_capacity(rel.props.len());
            for (k, v) in &rel.props {
                resolved_rel_props.push((k.clone(), v.resolve(params)?));
            }

            let edge_id = graph.create_edge_with(src, dst, rel_type, |edge| {
                for (k, val) in resolved_rel_props {
                    edge.set_prop(k, val);
                }
            })?;
            if let Some(ref alias) = rel.alias {
                alias_edge_map.insert(alias.clone(), edge_id);
            }
        }
    }

    // 1b. Handle MERGE patterns (atomic upsert)
    for merge in &ast.merges {
        let (pattern_nodes, pattern_rels) = &merge.pattern;
        let mut pattern_node_ids = Vec::new();
        let mut is_newly_created = false;

        for n in pattern_nodes {
            let mut resolved_props = Vec::with_capacity(n.props.len());
            for (k, v) in &n.props {
                resolved_props.push((k.clone(), v.resolve(params)?));
            }

            let existing_id = if let Some(&id) = alias_node_map.get(&n.alias) {
                Some(id)
            } else {
                let candidates: Vec<NodeId> = if let Some(lbl) = &n.label {
                    graph.nodes_with_label(lbl)
                } else {
                    graph.nodes().map(|node| node.id).collect()
                };

                candidates.into_iter().find(|&id| {
                    if let Some(node) = graph.get_node(id) {
                        resolved_props
                            .iter()
                            .all(|(k, expected)| node.properties.get(k) == Some(expected))
                    } else {
                        false
                    }
                })
            };

            let node_id = match existing_id {
                Some(id) => id,
                None => {
                    is_newly_created = true;
                    graph.create_node_with(|node| {
                        if let Some(lbl) = &n.label {
                            node.add_label(lbl);
                        }
                        for (k, val) in &resolved_props {
                            node.set_prop(k, val.clone());
                        }
                    })?
                }
            };

            if !n.alias.is_empty() {
                alias_node_map.insert(n.alias.clone(), node_id);
            }
            pattern_node_ids.push(node_id);
        }

        for (i, rel) in pattern_rels.iter().enumerate() {
            let src = pattern_node_ids[i];
            let dst = pattern_node_ids[i + 1];
            let rel_type = rel.rel_type.as_deref().unwrap_or("RELATED");
            let mut resolved_rel_props = Vec::with_capacity(rel.props.len());
            for (k, v) in &rel.props {
                resolved_rel_props.push((k.clone(), v.resolve(params)?));
            }

            let existing_edge = graph
                .incident_edges(src, Direction::Outgoing)
                .ok()
                .and_then(|edges| {
                    edges.into_iter().find(|e| {
                        e.target == dst
                            && (rel.rel_type.is_none() || e.rel_type == rel_type)
                            && resolved_rel_props
                                .iter()
                                .all(|(k, expected)| e.properties.get(k) == Some(expected))
                    })
                });

            let edge_id = match existing_edge {
                Some(e) => e.id,
                None => {
                    is_newly_created = true;
                    graph.create_edge_with(src, dst, rel_type, |edge| {
                        for (k, val) in resolved_rel_props {
                            edge.set_prop(k, val);
                        }
                    })?
                }
            };

            if let Some(ref alias) = rel.alias {
                alias_edge_map.insert(alias.clone(), edge_id);
            }
        }

        let set_targets = if is_newly_created {
            &merge.on_create_set
        } else {
            &merge.on_match_set
        };

        for s in set_targets {
            if let Some(&node_id) = alias_node_map.get(&s.alias) {
                let val = s.val.resolve(params)?;
                let _ = graph.set_node_property(node_id, &s.prop, val);
            }
        }
    }

    // 2. Process MATCH queries or construct bindings from created/merged aliases
    let bindings: Vec<MatchBinding> = if !ast.matches.is_empty() {
        let (pattern_nodes, pattern_rels) = &ast.matches[0];
        let mut builder = graph.query();

        if let Some(first_node) = pattern_nodes.first() {
            builder = builder.match_node(&first_node.alias);
            if let Some(lbl) = &first_node.label {
                builder = builder.with_label(lbl);
            }
            for (k, v) in &first_node.props {
                let val = v.resolve(params)?;
                builder = builder.with_prop(k, val);
            }
        }

        for (i, rel) in pattern_rels.iter().enumerate() {
            let target_node = &pattern_nodes[i + 1];
            builder = builder.rel_hops(
                rel.alias.clone(),
                rel.rel_type.clone(),
                rel.direction,
                rel.hops,
                target_node.alias.clone(),
            );
            if let Some(lbl) = &target_node.label {
                builder = builder.with_label(lbl);
            }
            for (k, v) in &target_node.props {
                let val = v.resolve(params)?;
                builder = builder.with_prop(k, val);
            }
        }

        // Apply WHERE clauses with resolved parameters
        for w in ast.where_clauses {
            let alias = w.alias.clone();
            let prop = w.prop.clone();
            let op = w.op.clone();
            let expected_val = w.val.resolve(params)?;

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

        builder
            .execute()
            .into_iter()
            .map(|ctx| MatchBinding {
                nodes: ctx.node_bindings().clone(),
                edges: ctx.edge_bindings().clone(),
            })
            .collect()
    } else {
        vec![MatchBinding {
            nodes: alias_node_map.clone(),
            edges: alias_edge_map.clone(),
        }]
    };

    // 3. Process SET clauses on matched entities with resolved parameters
    if !ast.set_clauses.is_empty() {
        for b in &bindings {
            for s in &ast.set_clauses {
                if let Some(&node_id) = b.nodes.get(&s.alias) {
                    let val = s.val.resolve(params)?;
                    let _ = graph.set_node_property(node_id, &s.prop, val);
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

    // If query was purely a mutation (CREATE/MERGE/SET/DELETE) without RETURN
    if ast.returns.is_empty() {
        let count = alias_node_map.len().max(bindings.len());
        let columns = vec!["result".to_string()];
        let rows = vec![vec![PropertyValue::String(format!(
            "Applied mutations across {} entities",
            count
        ))]];
        return Ok(CypherResult::new(columns, rows));
    }

    // 5. Evaluate RETURN columns and projections
    let columns: Vec<String> = ast.returns.iter().map(|r| r.output_name.clone()).collect();
    let has_aggregations = ast.returns.iter().any(|r| match &r.kind {
        ReturnKind::CountStar
        | ReturnKind::Count(_, _, _)
        | ReturnKind::Sum(_, _)
        | ReturnKind::Avg(_, _)
        | ReturnKind::Min(_, _)
        | ReturnKind::Max(_, _)
        | ReturnKind::Collect(_, _, _) => true,
        _ => false,
    });

    let mut rows: Vec<Vec<PropertyValue>> = Vec::new();

    if has_aggregations {
        let mut agg_row = Vec::new();
        for item in &ast.returns {
            match &item.kind {
                ReturnKind::CountStar => {
                    agg_row.push(PropertyValue::Int(bindings.len() as i64));
                }
                ReturnKind::Count(alias, prop_opt, is_distinct) => {
                    let mut count = 0;
                    let mut seen = std::collections::HashSet::new();
                    for b in &bindings {
                        if let Some(&node_id) = b.nodes.get(alias) {
                            if let Some(prop) = prop_opt {
                                if let Some(val) = graph.get_node_property(node_id, prop) {
                                    if !*is_distinct || seen.insert(val.clone()) {
                                        count += 1;
                                    }
                                }
                            } else {
                                if !*is_distinct
                                    || seen.insert(PropertyValue::Int(node_id.raw() as i64))
                                {
                                    count += 1;
                                }
                            }
                        }
                    }
                    agg_row.push(PropertyValue::Int(count));
                }
                ReturnKind::Collect(alias, prop_opt, is_distinct) => {
                    let mut list = Vec::new();
                    let mut seen = std::collections::HashSet::new();
                    for b in &bindings {
                        if let Some(&node_id) = b.nodes.get(alias) {
                            let val = if let Some(prop) = prop_opt {
                                graph
                                    .get_node_property(node_id, prop)
                                    .cloned()
                                    .unwrap_or(PropertyValue::Null)
                            } else if let Some(node) = graph.get_node(node_id) {
                                let mut map = BTreeMap::new();
                                map.insert(
                                    "id".to_string(),
                                    PropertyValue::Int(node.id.raw() as i64),
                                );
                                for (k, v) in node.properties.iter() {
                                    map.insert(k.clone(), v.clone());
                                }
                                PropertyValue::Map(map)
                            } else {
                                PropertyValue::Null
                            };

                            if val != PropertyValue::Null {
                                if !*is_distinct || seen.insert(val.clone()) {
                                    list.push(val);
                                }
                            }
                        }
                    }
                    agg_row.push(PropertyValue::List(list));
                }
                ReturnKind::Sum(alias, prop) => {
                    let mut sum_f = 0.0;
                    let mut is_int = true;
                    let mut sum_i = 0i64;
                    for b in &bindings {
                        if let Some(&node_id) = b.nodes.get(alias) {
                            if let Some(val) = graph.get_node_property(node_id, prop) {
                                match val {
                                    PropertyValue::Int(i) => {
                                        sum_i += i;
                                        sum_f += *i as f64;
                                    }
                                    PropertyValue::Float(f) => {
                                        is_int = false;
                                        sum_f += f;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    if is_int {
                        agg_row.push(PropertyValue::Int(sum_i));
                    } else {
                        agg_row.push(PropertyValue::Float(sum_f));
                    }
                }
                ReturnKind::Avg(alias, prop) => {
                    let mut sum = 0.0;
                    let mut count = 0;
                    for b in &bindings {
                        if let Some(&node_id) = b.nodes.get(alias) {
                            if let Some(val) = graph.get_node_property(node_id, prop) {
                                if let Some(f) = val.as_f64() {
                                    sum += f;
                                    count += 1;
                                }
                            }
                        }
                    }
                    if count > 0 {
                        agg_row.push(PropertyValue::Float(sum / count as f64));
                    } else {
                        agg_row.push(PropertyValue::Null);
                    }
                }
                ReturnKind::Min(alias, prop) => {
                    let mut min_val: Option<PropertyValue> = None;
                    for b in &bindings {
                        if let Some(&node_id) = b.nodes.get(alias) {
                            if let Some(val) = graph.get_node_property(node_id, prop) {
                                match &min_val {
                                    None => min_val = Some(val.clone()),
                                    Some(curr) if val < curr => min_val = Some(val.clone()),
                                    _ => {}
                                }
                            }
                        }
                    }
                    agg_row.push(min_val.unwrap_or(PropertyValue::Null));
                }
                ReturnKind::Max(alias, prop) => {
                    let mut max_val: Option<PropertyValue> = None;
                    for b in &bindings {
                        if let Some(&node_id) = b.nodes.get(alias) {
                            if let Some(val) = graph.get_node_property(node_id, prop) {
                                match &max_val {
                                    None => max_val = Some(val.clone()),
                                    Some(curr) if val > curr => max_val = Some(val.clone()),
                                    _ => {}
                                }
                            }
                        }
                    }
                    agg_row.push(max_val.unwrap_or(PropertyValue::Null));
                }
                _ => {
                    agg_row.push(PropertyValue::Null);
                }
            }
        }
        rows.push(agg_row);
    } else {
        for b in &bindings {
            let mut row = Vec::new();
            for item in &ast.returns {
                match &item.kind {
                    ReturnKind::Property(alias, prop) => {
                        let mut val = PropertyValue::Null;
                        if let Some(&node_id) = b.nodes.get(alias) {
                            if let Some(v) = graph.get_node_property(node_id, prop) {
                                val = v.clone();
                            }
                        } else if let Some(&rel_id) = b.edges.get(alias) {
                            if let Some(v) = graph.get_edge_property(rel_id, prop) {
                                val = v.clone();
                            }
                        }
                        row.push(val);
                    }
                    ReturnKind::Node(alias) => {
                        if let Some(&node_id) = b.nodes.get(alias) {
                            if let Some(node) = graph.get_node(node_id) {
                                let mut map = BTreeMap::new();
                                map.insert(
                                    "id".to_string(),
                                    PropertyValue::Int(node.id.raw() as i64),
                                );
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
                    _ => row.push(PropertyValue::Null),
                }
            }
            rows.push(row);
        }
    }

    // Deduplicate rows if RETURN DISTINCT
    if ast.distinct {
        let mut seen = std::collections::HashSet::new();
        rows.retain(|r| seen.insert(r.clone()));
    }

    // 6. Apply ORDER BY sorting if specified
    if !ast.order_by.is_empty() && !rows.is_empty() {
        for order in ast.order_by.iter().rev() {
            // Find target column index matching alias or alias.prop
            let col_name = if let Some(ref p) = order.prop {
                format!("{}.{}", order.alias, p)
            } else {
                order.alias.clone()
            };

            if let Some(col_idx) = columns.iter().position(|c| c == &col_name) {
                let desc = order.descending;
                rows.sort_by(|a, b| {
                    let ord = a[col_idx].cmp(&b[col_idx]);
                    if desc { ord.reverse() } else { ord }
                });
            }
        }
    }

    // 7. Apply LIMIT pagination
    if let Some(lim) = ast.limit {
        rows.truncate(lim);
    }

    Ok(CypherResult::new(columns, rows))
}

/// Executes a parsed Cypher AST against a mutable Graph instance with empty parameters.
pub fn execute_ast(graph: &mut Graph, ast: AstQuery) -> Result<CypherResult> {
    execute_ast_with_params(graph, ast, &HashMap::new())
}
