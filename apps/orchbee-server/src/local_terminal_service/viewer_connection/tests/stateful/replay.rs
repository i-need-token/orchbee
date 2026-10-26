use super::{Kind, Step};

pub(super) fn encode(steps: &[Step]) -> String {
    let rows: Vec<_> = steps
        .iter()
        .map(|s| {
            serde_json::json!([
                s.kind as u8,
                s.connection,
                s.viewer,
                s.target,
                s.grant,
                s.value,
                s.alternate
            ])
        })
        .collect();
    serde_json::json!({"version":2,"steps":rows}).to_string()
}
pub(super) fn decode(input: &str) -> Result<Vec<Step>, String> {
    let value: serde_json::Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    if value["version"] != 2 {
        return Err("unsupported replay version".into());
    }
    value["steps"]
        .as_array()
        .ok_or("steps")?
        .iter()
        .map(|row| {
            let values = row.as_array().ok_or("row")?;
            if values.len() != 7 {
                return Err("field count".into());
            }
            let n = |i: usize| {
                values[i]
                    .as_u64()
                    .ok_or_else(|| "unsigned field".to_owned())
            };
            let kind = match n(0)? {
                0 => Kind::Attach,
                1 => Kind::Acquire,
                2 => Kind::Control,
                3 => Kind::Poll,
                4 => Kind::Feedback,
                5 => Kind::Detach,
                6 => Kind::Drop,
                7 => Kind::Dispose,
                8 => Kind::ReplaceWorker,
                _ => return Err("kind".into()),
            };
            let connection = usize::try_from(n(1)?).map_err(|_| "session")?;
            let target = usize::try_from(n(3)?).map_err(|_| "target")?;
            if connection > 1 || target > 9 || n(5)? == 0 {
                return Err("bounds".into());
            }
            Ok(Step {
                kind,
                connection,
                viewer: usize::try_from(n(2)?).map_err(|_| "viewer")?,
                target,
                grant: usize::try_from(n(4)?).map_err(|_| "grant")?,
                value: n(5)?,
                alternate: values[6].as_bool().ok_or("alternate")?,
            })
        })
        .collect()
}
pub(super) fn shrink(
    mut trace: Vec<Step>,
    fingerprint: &(&'static str, String),
    classify: impl Fn(&[Step]) -> Option<(&'static str, String)>,
) -> Vec<Step> {
    let mut width = trace.len();
    while width > 0 {
        let mut at = 0;
        while at + width <= trace.len() {
            let mut candidate = trace.clone();
            candidate.drain(at..at + width);
            if classify(&candidate).as_ref() == Some(fingerprint) {
                trace = candidate;
            } else {
                at += 1;
            }
        }
        width /= 2;
    }
    for index in 0..trace.len() {
        for field in 0..6 {
            let mut candidate = trace.clone();
            let s = &mut candidate[index];
            match field {
                0 => s.connection = 0,
                1 => s.target = 0,
                2 => s.viewer = 0,
                3 => s.grant = 0,
                4 => s.value = 1,
                _ => s.alternate = false,
            }
            if candidate != trace && classify(&candidate).as_ref() == Some(fingerprint) {
                trace = candidate;
            }
        }
    }
    trace
}
