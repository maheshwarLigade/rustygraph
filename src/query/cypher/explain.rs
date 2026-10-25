use super::ast::{AstQuery, CompOp, ReturnKind};
use serde::{Deserialize, Serialize};

/// A single operator or execution step in a Cypher query execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlanStep {
    pub operator: String,
    pub description: String,
    pub estimated_cost: f64,
    pub identifiers: Vec<String>,
    pub children: Vec<QueryPlanStep>,
}

/// Structured execution plan and human-readable plan tree for a Cypher query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    pub query: String,
    pub is_mutation: bool,
    pub root_step: QueryPlanStep,
    pub text_representation: String,
}

/// Builds a structured query execution plan and text representation from a parsed AST.
pub fn explain_ast(ast: &AstQuery, query_str: &str) -> QueryPlan {
    let mut current_leaf: Option<QueryPlanStep> = None;
    let is_mutation = ast.is_mutation();

    // 1. Initial Scans: MATCH pattern scans
    if !ast.matches.is_empty() {
        let (nodes, rels) = &ast.matches[0];
        if let Some(first_node) = nodes.first() {
            let scan_op = if first_node.label.is_some() {
                "NodeByLabelScan"
            } else {
                "AllNodesScan"
            };

            let mut desc = format!("Scan nodes with alias '{}'", first_node.alias);
            if let Some(lbl) = &first_node.label {
                desc.push_str(&format!(" and label ':{}'", lbl));
            }
            if !first_node.props.is_empty() {
                let props_desc: Vec<String> = first_node
                    .props
                    .iter()
                    .map(|(k, _)| format!("{} = <val>", k))
                    .collect();
                desc.push_str(&format!(" matching [{}]", props_desc.join(", ")));
            }

            let mut leaf = QueryPlanStep {
                operator: scan_op.to_string(),
                description: desc,
                estimated_cost: 1.0,
                identifiers: vec![first_node.alias.clone()],
                children: Vec::new(),
            };

            // Relationship expansions
            for (i, rel) in rels.iter().enumerate() {
                let target = &nodes[i + 1];
                let (prefix, suffix) = match rel.direction {
                    crate::model::Direction::Outgoing => ("-", "->"),
                    crate::model::Direction::Incoming => ("<-", "-"),
                    crate::model::Direction::Both => ("-", "-"),
                };
                let rel_desc = rel.rel_type.as_deref().unwrap_or("*");
                let hops_desc = if let Some((min, max)) = rel.hops {
                    format!("*{min}..{max}")
                } else {
                    "".to_string()
                };

                let expand_desc = format!(
                    "Expand ({}){}[{}:{}{}]{}({})",
                    nodes[i].alias,
                    prefix,
                    rel.alias.as_deref().unwrap_or(""),
                    rel_desc,
                    hops_desc,
                    suffix,
                    target.alias
                );

                let mut touched_vars = vec![nodes[i].alias.clone(), target.alias.clone()];
                if let Some(a) = &rel.alias {
                    touched_vars.push(a.clone());
                }

                leaf = QueryPlanStep {
                    operator: if rel.hops.is_some() {
                        "VariableLengthExpand".to_string()
                    } else {
                        "ExpandAll".to_string()
                    },
                    description: expand_desc,
                    estimated_cost: leaf.estimated_cost + 2.5,
                    identifiers: touched_vars,
                    children: vec![leaf],
                };
            }

            current_leaf = Some(leaf);
        }
    }

    // 2. CREATE or MERGE steps if mutation
    if !ast.creates.is_empty() {
        let mut create_descs = Vec::new();
        let mut touched = Vec::new();
        for (nodes, rels) in &ast.creates {
            for n in nodes {
                create_descs.push(format!(
                    "(:{}{{{}}})",
                    n.label.as_deref().unwrap_or(""),
                    n.props.len()
                ));
                if !n.alias.is_empty() {
                    touched.push(n.alias.clone());
                }
            }
            for r in rels {
                create_descs.push(format!(
                    "-[:{}]->",
                    r.rel_type.as_deref().unwrap_or("RELATED")
                ));
            }
        }
        let create_step = QueryPlanStep {
            operator: "Create".to_string(),
            description: format!("Create graph pattern: {}", create_descs.join(" ")),
            estimated_cost: 2.0,
            identifiers: touched,
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(create_step);
    }

    for merge in &ast.merges {
        let (pattern_nodes, pattern_rels) = &merge.pattern;
        let merge_desc = format!(
            "Merge pattern with {} nodes and {} rels (ON CREATE: {} SETs, ON MATCH: {} SETs)",
            pattern_nodes.len(),
            pattern_rels.len(),
            merge.on_create_set.len(),
            merge.on_match_set.len()
        );
        let merge_step = QueryPlanStep {
            operator: "Merge".to_string(),
            description: merge_desc,
            estimated_cost: 3.0,
            identifiers: pattern_nodes.iter().map(|n| n.alias.clone()).collect(),
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(merge_step);
    }

    // 3. WHERE filter predicates
    if !ast.where_clauses.is_empty() {
        for w in &ast.where_clauses {
            let op_str = match w.op {
                CompOp::Eq => "=",
                CompOp::Neq => "<>",
                CompOp::Lt => "<",
                CompOp::Lte => "<=",
                CompOp::Gt => ">",
                CompOp::Gte => ">=",
            };
            let filter_step = QueryPlanStep {
                operator: "Filter".to_string(),
                description: format!("Filter predicate: {}.{} {} <expr>", w.alias, w.prop, op_str),
                estimated_cost: current_leaf
                    .as_ref()
                    .map_or(1.0, |c| c.estimated_cost + 1.0),
                identifiers: vec![w.alias.clone()],
                children: current_leaf.into_iter().collect(),
            };
            current_leaf = Some(filter_step);
        }
    }

    // 4. SET / DELETE mutation steps
    if !ast.set_clauses.is_empty() {
        let set_descs: Vec<String> = ast
            .set_clauses
            .iter()
            .map(|s| format!("{}.{} = <val>", s.alias, s.prop))
            .collect();
        let set_step = QueryPlanStep {
            operator: "SetProperty".to_string(),
            description: format!("Mutate properties: {}", set_descs.join(", ")),
            estimated_cost: current_leaf
                .as_ref()
                .map_or(1.0, |c| c.estimated_cost + 1.0),
            identifiers: ast.set_clauses.iter().map(|s| s.alias.clone()).collect(),
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(set_step);
    }

    if !ast.delete_clauses.is_empty() {
        let del_descs: Vec<String> = ast
            .delete_clauses
            .iter()
            .map(|d| {
                if d.detach {
                    format!("DETACH DELETE {}", d.alias)
                } else {
                    format!("DELETE {}", d.alias)
                }
            })
            .collect();
        let del_step = QueryPlanStep {
            operator: "Delete".to_string(),
            description: format!("Delete elements: {}", del_descs.join(", ")),
            estimated_cost: current_leaf
                .as_ref()
                .map_or(1.0, |c| c.estimated_cost + 2.0),
            identifiers: ast.delete_clauses.iter().map(|d| d.alias.clone()).collect(),
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(del_step);
    }

    // 5. Aggregations if present
    let has_aggregations = ast.returns.iter().any(|r| {
        matches!(
            r.kind,
            ReturnKind::CountStar
                | ReturnKind::Count(_, _, _)
                | ReturnKind::Sum(_, _)
                | ReturnKind::Avg(_, _)
                | ReturnKind::Min(_, _)
                | ReturnKind::Max(_, _)
                | ReturnKind::Collect(_, _, _)
        )
    });

    if has_aggregations {
        let aggs: Vec<String> = ast
            .returns
            .iter()
            .filter_map(|r| match &r.kind {
                ReturnKind::CountStar => Some("count(*)".to_string()),
                ReturnKind::Count(a, p, d) => Some(format!(
                    "count({}{}{})",
                    if *d { "DISTINCT " } else { "" },
                    a,
                    p.as_ref().map(|x| format!(".{x}")).unwrap_or_default()
                )),
                ReturnKind::Sum(a, p) => Some(format!("sum({a}.{p})")),
                ReturnKind::Avg(a, p) => Some(format!("avg({a}.{p})")),
                ReturnKind::Min(a, p) => Some(format!("min({a}.{p})")),
                ReturnKind::Max(a, p) => Some(format!("max({a}.{p})")),
                ReturnKind::Collect(a, p, d) => Some(format!(
                    "collect({}{}{})",
                    if *d { "DISTINCT " } else { "" },
                    a,
                    p.as_ref().map(|x| format!(".{x}")).unwrap_or_default()
                )),
                _ => None,
            })
            .collect();

        let agg_step = QueryPlanStep {
            operator: "Aggregate".to_string(),
            description: format!("Compute aggregations [{}]", aggs.join(", ")),
            estimated_cost: current_leaf
                .as_ref()
                .map_or(1.0, |c| c.estimated_cost + 3.0),
            identifiers: Vec::new(),
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(agg_step);
    }

    // 6. Distinct
    if ast.distinct {
        let distinct_step = QueryPlanStep {
            operator: "Distinct".to_string(),
            description: "Deduplicate result record tuples".to_string(),
            estimated_cost: current_leaf
                .as_ref()
                .map_or(1.0, |c| c.estimated_cost + 1.5),
            identifiers: Vec::new(),
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(distinct_step);
    }

    // 7. ORDER BY
    if !ast.order_by.is_empty() {
        let order_descs: Vec<String> = ast
            .order_by
            .iter()
            .map(|o| {
                format!(
                    "{}{} {}",
                    o.alias,
                    o.prop.as_ref().map(|p| format!(".{p}")).unwrap_or_default(),
                    if o.descending { "DESC" } else { "ASC" }
                )
            })
            .collect();
        let sort_step = QueryPlanStep {
            operator: "Sort".to_string(),
            description: format!("Order by [{}]", order_descs.join(", ")),
            estimated_cost: current_leaf
                .as_ref()
                .map_or(1.0, |c| c.estimated_cost + 2.0),
            identifiers: ast.order_by.iter().map(|o| o.alias.clone()).collect(),
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(sort_step);
    }

    // 8. Limit
    if let Some(limit_val) = ast.limit {
        let limit_step = QueryPlanStep {
            operator: "Limit".to_string(),
            description: format!("Slice top {} records", limit_val),
            estimated_cost: current_leaf
                .as_ref()
                .map_or(1.0, |c| c.estimated_cost * 0.8),
            identifiers: Vec::new(),
            children: current_leaf.into_iter().collect(),
        };
        current_leaf = Some(limit_step);
    }

    // 9. Root: ProduceResults (Projection)
    let output_cols: Vec<String> = ast.returns.iter().map(|r| r.output_name.clone()).collect();
    let root_step = QueryPlanStep {
        operator: "ProduceResults".to_string(),
        description: if output_cols.is_empty() {
            "Empty projection / mutation result".to_string()
        } else {
            format!("Yield output columns: [{}]", output_cols.join(", "))
        },
        estimated_cost: current_leaf
            .as_ref()
            .map_or(0.5, |c| c.estimated_cost + 0.1),
        identifiers: output_cols,
        children: current_leaf.into_iter().collect(),
    };

    let text_repr = format_plan_tree(&root_step, 0);

    QueryPlan {
        query: query_str.to_string(),
        is_mutation,
        root_step,
        text_representation: text_repr,
    }
}

fn format_plan_tree(step: &QueryPlanStep, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let mut s = format!("{}+ {} ({})\n", indent, step.operator, step.description);
    for child in &step.children {
        s.push_str(&format_plan_tree(child, depth + 1));
    }
    s
}
