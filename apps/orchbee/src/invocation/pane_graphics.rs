//! Closed graphics inputs are admitted before discovery or visual writes.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, UsageContext, UsageReason, check_scope, missing_command,
    missing_required, target, unknown_command, usage_with,
};
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_SET_BYTES, PaneGraphicsFrame, TerminalPresentationId, WorkbenchClientId,
    graphics_layer_id,
};
use serde::Deserialize;

pub(crate) const MAXIMUM_SET_DOCUMENT_BYTES: usize = 768 * 1024;
fn primary() -> String {
    "primary".into()
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetSpec {
    #[serde(default = "primary")]
    pub layer_id: String,
    #[serde(default)]
    pub z_index: i32,
    pub image: PaneGraphicsFrame,
}
impl SetSpec {
    pub(crate) fn valid(&self) -> bool {
        graphics_layer_id(&self.layer_id) && self.image.validate(MAXIMUM_GRAPHICS_SET_BYTES).is_ok()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LayerSpec {
    #[serde(default = "primary")]
    pub layer_id: String,
    #[serde(default)]
    pub z_index: i32,
}
impl Default for LayerSpec {
    fn default() -> Self {
        Self {
            layer_id: primary(),
            z_index: 0,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClearSpec {
    #[serde(default = "primary")]
    layer_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Input {
    Inline(Box<SetSpec>),
    File(String),
    Stdin,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Set(Input),
    Clear(String),
    Info(Option<WorkbenchClientId>),
    Stream(LayerSpec),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Command {
    pub target: Target,
    pub presentation: Option<TerminalPresentationId>,
    pub timeout_ms: Option<u32>,
    pub action: Action,
}
impl Command {
    pub(crate) const fn operation(&self) -> &'static str {
        match self.action {
            Action::Set(_) => "set",
            Action::Clear(_) => "clear",
            Action::Info(_) => "info",
            Action::Stream(_) => "stream",
        }
    }
}
pub(super) fn parse(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let (verb, operands) = match words {
        ["panel", "graphics"] => {
            return Err(missing_command(
                UsageContext::Path("panel graphics"),
                envelope,
            ));
        }
        ["panel", "graphics", verb, rest @ ..]
            if matches!(*verb, "set" | "clear" | "info" | "stream") =>
        {
            (*verb, rest)
        }
        ["panel", "graphics", other, ..] => {
            return Err(unknown_command(
                UsageContext::Path("panel graphics"),
                other,
                envelope,
            ));
        }
        _ => unreachable!("graphics family routed here"),
    };
    let context = UsageContext::Path(match verb {
        "set" => "panel graphics set",
        "clear" => "panel graphics clear",
        "info" => "panel graphics info",
        _ => "panel graphics stream",
    });
    check_scope(
        options,
        Scope {
            graphics: true,
            agent_view_spec: verb != "info",
            client: verb == "info",
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let [operand] = operands else {
        return Err(if operands.is_empty() {
            missing_required(context, "<panel>", envelope)
        } else {
            usage_with(
                UsageReason::UnexpectedArgument {
                    context,
                    value: operands[1].into(),
                },
                envelope,
            )
        });
    };
    let invalid = |option, value: &str| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value: value.into(),
                expected: ExpectedValue::GraphicsSpecification,
            },
            envelope,
        )
    };
    let raw = options.agent_view_spec.as_deref();
    let action = match verb {
        "set" => {
            let raw = raw.ok_or_else(|| missing_required(context, "--spec", envelope))?;
            let input = if raw == "-" {
                Input::Stdin
            } else if let Some(path) = raw.strip_prefix('@') {
                if path.is_empty() || path.contains('\0') {
                    return Err(invalid("--spec", raw));
                }
                Input::File(path.into())
            } else {
                let spec: SetSpec =
                    serde_json::from_str(raw).map_err(|_| invalid("--spec", raw))?;
                if !spec.valid() {
                    return Err(invalid("--spec", raw));
                }
                Input::Inline(Box::new(spec))
            };
            Action::Set(input)
        }
        "clear" => {
            let layer = raw
                .map(|raw| {
                    serde_json::from_str::<ClearSpec>(raw)
                        .map(|spec| spec.layer_id)
                        .map_err(|_| invalid("--spec", raw))
                })
                .transpose()?
                .unwrap_or_else(primary);
            if !graphics_layer_id(&layer) {
                return Err(invalid("--spec", raw.unwrap_or("")));
            }
            Action::Clear(layer)
        }
        "info" => Action::Info(super::client_control::client_id(
            options, context, envelope,
        )?),
        _ => {
            let spec = raw
                .map(|raw| {
                    serde_json::from_str::<LayerSpec>(raw).map_err(|_| invalid("--spec", raw))
                })
                .transpose()?
                .unwrap_or_default();
            if !graphics_layer_id(&spec.layer_id) {
                return Err(invalid("--spec", raw.unwrap_or("")));
            }
            Action::Stream(spec)
        }
    };
    let timeout_ms = options
        .timeout
        .as_ref()
        .map(|_| super::wait_timeout_option(options, context, envelope))
        .transpose()?;
    Ok(Request::PaneGraphics {
        command: Command {
            target: target((*operand).into(), Resource::Panel, envelope)?,
            presentation: super::report_target::parse(options, context, envelope)?,
            timeout_ms,
            action,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
        super::super::parse_request(words.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn graphics_grammar_is_closed_before_any_source_or_site_effect() {
        for verb in ["set", "clear", "info", "stream"] {
            let mut args = vec!["panel", "graphics", verb, "@panel:1", "--format=json"];
            if verb == "set" {
                args.push("--spec=@C:/image.png");
            }
            assert!(matches!(
                parse(&args).unwrap(),
                Request::PaneGraphics { .. }
            ));
            args.push("--exec=C:/other.exe");
            assert!(parse(&args).is_err());
        }
        for args in [
            vec!["panel", "graphics", "set", "@panel:1"],
            vec![
                "panel",
                "graphics",
                "clear",
                "@panel:1",
                "--spec={\"zIndex\":0}",
            ],
            vec!["panel", "graphics", "info", "@panel:1", "--spec={}"],
            vec![
                "panel",
                "graphics",
                "stream",
                "@panel:1",
                "--spec={\"layerId\":\"../foreign\"}",
            ],
            vec![
                "panel",
                "graphics",
                "stream",
                "@panel:1",
                "--spec={\"zIndex\":0,\"zIndex\":1}",
            ],
            vec!["panel", "graphics", "set", "@panel:1", "--spec=@"],
            vec!["panel", "graphics", "clear", "@panel:1", "--timeout=0"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
}
