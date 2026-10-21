use crate::value::PropertyValue;
use std::fmt;

/// Tabular result returned by executing a Cypher query string.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CypherResult {
    #[serde(default = "default_true")]
    pub success: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<PropertyValue>>,
    #[serde(default)]
    pub error: Option<String>,
}

fn default_true() -> bool {
    true
}

impl CypherResult {
    pub fn new(columns: Vec<String>, rows: Vec<Vec<PropertyValue>>) -> Self {
        Self {
            success: true,
            columns,
            rows,
            error: None,
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl fmt::Display for CypherResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.columns.is_empty() {
            return write!(f, "(Empty result)");
        }

        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.len()).collect();
        for row in &self.rows {
            for (i, val) in row.iter().enumerate() {
                if i < widths.len() {
                    let s = format!("{val}");
                    widths[i] = widths[i].max(s.len());
                }
            }
        }

        // Header
        write!(f, "|")?;
        for (i, col) in self.columns.iter().enumerate() {
            write!(f, " {:<width$} |", col, width = widths[i])?;
        }
        writeln!(f)?;

        // Separator
        write!(f, "+")?;
        for w in &widths {
            write!(f, "{}+", "-".repeat(w + 2))?;
        }
        writeln!(f)?;

        // Rows
        for row in &self.rows {
            write!(f, "|")?;
            for (i, val) in row.iter().enumerate() {
                let s = format!("{val}");
                write!(f, " {:<width$} |", s, width = widths[i])?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}
