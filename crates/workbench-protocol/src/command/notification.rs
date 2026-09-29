//! Ephemeral notification delivery. Accepted delivery does not acknowledge physical presentation.
use crate::{ContractError, ProtocolVersion, SiteId, WorkbenchRequestId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationSound {
    None,
    Done,
    Request,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Notification {
    pub title: String,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub body: Option<String>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub position: Option<NotificationPosition>,
    pub sound: NotificationSound,
}
impl Notification {
    pub fn normalize(
        title: &str,
        body: Option<&str>,
        position: impl Into<Option<NotificationPosition>>,
        sound: NotificationSound,
    ) -> Result<Self, ContractError> {
        let title = normalize_text(title, 80).ok_or(ContractError::InvalidDocument)?;
        Ok(Self {
            title,
            body: body.and_then(|body| normalize_text(body, 240)),
            position: position.into(),
            sound,
        })
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.title.len() <= 320
            && self.body.as_ref().is_none_or(|body| body.len() <= 960)
            && Self::normalize(&self.title, self.body.as_deref(), self.position, self.sound)
                .as_ref()
                == Ok(self)
        {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
fn normalize_text(value: &str, maximum: usize) -> Option<String> {
    let mut text = String::new();
    let mut space = false;
    let mut count = 0;
    for ch in value.chars() {
        let ch = match ch {
            '\n' | '\r' | '\t' => ' ',
            ch if ch.is_control() => continue,
            ch => ch,
        };
        if ch.is_whitespace() {
            if space {
                continue;
            }
            space = true;
            text.push(' ');
        } else {
            space = false;
            text.push(ch);
        }
        count += 1;
        if count >= maximum {
            break;
        }
    }
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NotificationAction {
    Show { notification: Notification },
    Listen { accepting: bool },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: NotificationAction,
}
impl NotificationRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
            NotificationAction::Show { notification } => notification.validate(),
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationReason {
    Disabled,
    Shown,
    Busy,
    RateLimited,
    NoForegroundClient,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationFailure {
    Denied,
    Unavailable,
    SiteMismatch,
    Conflict,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NotificationOutcome {
    Show {
        shown: bool,
        reason: NotificationReason,
    },
    Listening {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        notification: Option<Notification>,
    },
    Failed {
        reason: NotificationFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: NotificationOutcome,
}
impl NotificationResult {
    pub fn answers(&self, request: &NotificationRequest) -> bool {
        self.request_id == request.request_id
            && self.site_id == request.site_id
            && matches!(
                (&request.action, &self.outcome),
                (_, NotificationOutcome::Failed { .. })
                    | (
                        NotificationAction::Show { .. },
                        NotificationOutcome::Show { .. }
                    )
                    | (
                        NotificationAction::Listen { .. },
                        NotificationOutcome::Listening { .. }
                    )
            )
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            NotificationOutcome::Listening {
                notification: Some(notification),
            } => notification.validate(),
            NotificationOutcome::Show { shown, reason }
                if *shown != (*reason == NotificationReason::Shown) =>
            {
                Err(ContractError::InvalidDocument)
            }
            _ => Ok(()),
        }
    }
}
