use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardMapping {
    pub target: String,
    pub local: String,
    pub remote: String,
}

pub type Parameters = HashMap<String, String>;
