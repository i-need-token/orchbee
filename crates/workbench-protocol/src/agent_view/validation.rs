//! Normalizes and validates bounded Agent view specifications, including field-value types and
//! recursive filter budgets. It does not read Agent facts, evaluate the view, or decide which
//! source owns a specification.

use super::{
    AgentViewBuiltinField, AgentViewContextOperand, AgentViewField, AgentViewFilter,
    AgentViewSortField, AgentViewSpec, AgentViewToken, AgentViewValue, ContractError,
    MAXIMUM_AGENT_VIEW_SPEC_BYTES,
};

pub(super) fn source(value: &str) -> Result<String, ContractError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 120
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":._-".contains(&b))
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(value.into())
}
fn token(value: &AgentViewToken) -> Result<(), ContractError> {
    if value.token.is_empty()
        || value.token.len() > 32
        || !value
            .token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}
fn field(field: &AgentViewField) -> Result<(), ContractError> {
    if let AgentViewField::Token(t) = field {
        token(t)?;
    }
    Ok(())
}
fn value(field_value: &AgentViewField, value: &AgentViewValue) -> Result<(), ContractError> {
    field(field_value)?;
    use AgentViewBuiltinField as B;
    use AgentViewField::Builtin;
    let valid = match field_value {
        Builtin(B::WorkspaceId) => {
            matches!(value, AgentViewValue::Context(c) if c.context == AgentViewContextOperand::CurrentWorkspaceId)
                || matches!(value, AgentViewValue::String(_))
        }
        Builtin(B::TabId) => {
            matches!(value, AgentViewValue::Context(c) if c.context == AgentViewContextOperand::CurrentTabId)
                || matches!(value, AgentViewValue::String(_))
        }
        Builtin(B::Seen) => matches!(value, AgentViewValue::Bool(_)),
        Builtin(B::StateChangeSeq) => matches!(value, AgentViewValue::Number(_)),
        Builtin(B::Status) => matches!(value, AgentViewValue::String(s)
            if matches!(s.as_str(), "idle" | "working" | "blocked" | "done" | "error" | "unknown")),
        Builtin(B::PaneId | B::Agent) | AgentViewField::Token(_) => {
            matches!(value, AgentViewValue::String(_))
        }
    };
    if valid {
        Ok(())
    } else {
        Err(ContractError::InvalidDocument)
    }
}
fn filter(f: &AgentViewFilter, depth: usize, count: &mut usize) -> Result<(), ContractError> {
    *count += 1;
    if depth > 8 || *count > 64 {
        return Err(ContractError::InvalidDocument);
    }
    match f {
        AgentViewFilter::All { filters } | AgentViewFilter::Any { filters } => {
            if filters.is_empty() {
                return Err(ContractError::InvalidDocument);
            }
            for f in filters {
                filter(f, depth + 1, count)?;
            }
        }
        AgentViewFilter::Not { filter: f } => filter(f, depth + 1, count)?,
        AgentViewFilter::Eq { field, value: v } => value(field, v)?,
        AgentViewFilter::In { field, values } => {
            if values.is_empty() || values.len() > 32 {
                return Err(ContractError::InvalidDocument);
            }
            for v in values {
                value(field, v)?;
            }
        }
        AgentViewFilter::Exists { field: f } => field(f)?,
    }
    Ok(())
}
impl AgentViewSpec {
    pub fn normalized(&self) -> Result<Self, ContractError> {
        let mut spec = self.clone();
        spec.source = source(&self.source)?;
        spec.label = self
            .label
            .as_ref()
            .map(|s| {
                let value: String = s.trim().chars().filter(|c| !c.is_control()).collect();
                if value.is_empty() || value.chars().count() > 32 {
                    Err(ContractError::InvalidDocument)
                } else {
                    Ok(value)
                }
            })
            .transpose()?;
        spec.validate()?;
        Ok(spec)
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        source(&self.source)?;
        if let Some(label) = &self.label {
            let n = label.trim().chars().filter(|c| !c.is_control()).count();
            if n == 0 || n > 32 {
                return Err(ContractError::InvalidDocument);
            }
        }
        if let Some(f) = &self.filter {
            filter(f, 1, &mut 0)?;
        }
        if self.sort.len() > 8 {
            return Err(ContractError::InvalidDocument);
        }
        for s in &self.sort {
            if let AgentViewSortField::Token(t) = &s.field {
                token(t)?;
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| ContractError::InvalidDocument)?
            .len()
            > MAXIMUM_AGENT_VIEW_SPEC_BYTES
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
pub fn parse_agent_view_spec(bytes: &[u8]) -> Result<AgentViewSpec, ContractError> {
    if bytes.len() > MAXIMUM_AGENT_VIEW_SPEC_BYTES {
        return Err(ContractError::InvalidDocument);
    }
    let spec: AgentViewSpec =
        serde_json::from_slice(bytes).map_err(|_| ContractError::InvalidDocument)?;
    spec.normalized()
}

#[cfg(test)]
mod tests;
