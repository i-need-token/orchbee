//! Pure reference semantics; caller supplies canonical facts and stable input order.
use super::{
    AgentViewBuiltinField, AgentViewBuiltinSortField, AgentViewContextOperand, AgentViewField,
    AgentViewFilter, AgentViewSort, AgentViewSortField, AgentViewSortOrder, AgentViewValue,
};
use std::{borrow::Cow, cmp::Ordering};

pub struct Context<'a> {
    pub scope: &'a str,
    pub workspace_id: Option<&'a str>,
    pub tab_id: Option<&'a str>,
}
pub trait Entry {
    fn scope(&self) -> &str;
    fn status(&self) -> &str;
    fn workspace_id(&self) -> Option<&str>;
    fn tab_id(&self) -> Option<&str>;
    fn pane_id(&self) -> Option<&str>;
    fn agent(&self) -> Option<&str>;
    fn seen(&self) -> bool;
    fn state_change_seq(&self) -> Option<u64>;
    fn token(&self, name: &str) -> Option<&str>;
    fn workspace_order(&self) -> Option<u64>;
    fn tab_order(&self) -> Option<u64>;
    fn panel_order(&self) -> Option<u64>;
    fn attention(&self) -> u64;
}
#[derive(Eq, PartialEq, Ord, PartialOrd)]
enum Value<'a> {
    String(Cow<'a, str>),
    Id(&'a str, &'a str),
    Bool(bool),
    Number(u64),
}
fn field<'a>(entry: &'a impl Entry, field: &AgentViewField) -> Option<Value<'a>> {
    use AgentViewBuiltinField as B;
    Some(match field {
        AgentViewField::Builtin(b) => match b {
            B::Status => Value::String(entry.status().into()),
            B::WorkspaceId => Value::Id(entry.scope(), entry.workspace_id()?),
            B::TabId => Value::Id(entry.scope(), entry.tab_id()?),
            B::PaneId => Value::Id(entry.scope(), entry.pane_id()?),
            B::Agent => Value::String(entry.agent()?.into()),
            B::Seen => Value::Bool(entry.seen()),
            B::StateChangeSeq => Value::Number(entry.state_change_seq()?),
        },
        AgentViewField::Token(t) => Value::String(entry.token(&t.token)?.into()),
    })
}
fn operand<'a>(
    context: &'a Context<'_>,
    f: &AgentViewField,
    value: &'a AgentViewValue,
) -> Option<Value<'a>> {
    Some(match value {
        AgentViewValue::String(s)
            if matches!(
                f,
                AgentViewField::Builtin(
                    AgentViewBuiltinField::WorkspaceId
                        | AgentViewBuiltinField::TabId
                        | AgentViewBuiltinField::PaneId
                )
            ) =>
        {
            Value::Id(context.scope, s)
        }
        AgentViewValue::String(s) => Value::String(s.as_str().into()),
        AgentViewValue::Bool(b) => Value::Bool(*b),
        AgentViewValue::Number(n) => Value::Number(*n),
        AgentViewValue::Context(c) => Value::Id(
            context.scope,
            match c.context {
                AgentViewContextOperand::CurrentWorkspaceId => context.workspace_id?,
                AgentViewContextOperand::CurrentTabId => context.tab_id?,
            },
        ),
    })
}
#[must_use]
pub fn matches_filter(context: &Context<'_>, entry: &impl Entry, filter: &AgentViewFilter) -> bool {
    match filter {
        AgentViewFilter::All { filters } => {
            filters.iter().all(|f| matches_filter(context, entry, f))
        }
        AgentViewFilter::Any { filters } => {
            filters.iter().any(|f| matches_filter(context, entry, f))
        }
        AgentViewFilter::Not { filter } => !matches_filter(context, entry, filter),
        AgentViewFilter::Eq { field: f, value } => field(entry, f) == operand(context, f, value),
        AgentViewFilter::In { field: f, values } => values
            .iter()
            .any(|v| field(entry, f) == operand(context, f, v)),
        AgentViewFilter::Exists { field: f } => field(entry, f).is_some(),
    }
}
fn sort_value<'a>(e: &'a impl Entry, f: &AgentViewSortField) -> Option<Value<'a>> {
    use AgentViewBuiltinSortField as B;
    Some(match f {
        AgentViewSortField::Token(t) => Value::String(e.token(&t.token)?.into()),
        AgentViewSortField::Builtin(b) => match b {
            B::WorkspaceOrder => Value::Number(e.workspace_order()?),
            B::TabOrder => Value::Number(e.tab_order()?),
            B::PaneOrder => Value::Number(e.panel_order()?),
            B::Attention => Value::Number(e.attention()),
            B::Status => Value::String(e.status().into()),
            B::Agent => Value::String(e.agent()?.into()),
            B::Seen => Value::Bool(e.seen()),
            B::StateChangeSeq => Value::Number(e.state_change_seq()?),
        },
    })
}
#[must_use]
pub fn compare_entries(left: &impl Entry, right: &impl Entry, sorts: &[AgentViewSort]) -> Ordering {
    for sort in sorts {
        let order = match (
            sort_value(left, &sort.field),
            sort_value(right, &sort.field),
        ) {
            (Some(l), Some(r)) if sort.order == AgentViewSortOrder::Desc => r.cmp(&l),
            (Some(l), Some(r)) => l.cmp(&r),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests;
