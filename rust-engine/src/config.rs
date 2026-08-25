//! Config-driven knowledge base: question registry, predicate->evidence map,
//! policy->predicate map. Loaded once at startup from YAML.

use crate::domain::{AssuranceLevel, Kind};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct QuestionEntry {
    pub canonical: String,
    pub kind: Kind,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct QuestionsFile {
    questions: Vec<QuestionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EvidenceOption {
    #[serde(rename = "type")]
    pub evidence_type: String,
    pub assurance: AssuranceLevel,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PredicateDef {
    pub evidence: Vec<EvidenceOption>,
    #[serde(default)]
    pub prefer: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyDef {
    pub version: u32,
    pub requires: Vec<String>,
}

/// The complete deterministic knowledge base.
#[derive(Debug, Clone)]
pub struct Registry {
    pub questions: Vec<QuestionEntry>,
    pub predicates: BTreeMap<String, PredicateDef>,
    pub policies: BTreeMap<String, PolicyDef>,
}

impl Registry {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let q: QuestionsFile = read_yaml(&dir.join("questions.yaml"))?;
        let predicates: BTreeMap<String, PredicateDef> =
            read_yaml(&dir.join("predicates.yaml"))?;
        let policies: BTreeMap<String, PolicyDef> = read_yaml(&dir.join("policies.yaml"))?;
        Ok(Registry {
            questions: q.questions,
            predicates,
            policies,
        })
    }

    /// True if `id` is a known canonical predicate or policy id.
    pub fn is_canonical(&self, id: &str) -> bool {
        self.predicates.contains_key(id) || self.policies.contains_key(id)
    }

    /// Kind of a known canonical id.
    pub fn kind_of(&self, id: &str) -> Option<Kind> {
        if self.policies.contains_key(id) {
            Some(Kind::Policy)
        } else if self.predicates.contains_key(id) {
            Some(Kind::Predicate)
        } else {
            None
        }
    }

    /// All supported questions (canonical ids) for 422 responses.
    pub fn supported_questions(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.questions.iter().map(|q| q.canonical.clone()).collect();
        ids.sort();
        ids
    }
}

fn read_yaml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_yaml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}
