//! Recovers expected durations from the existing planner's literal arguments.
use super::{ExecError, FfmpegPaths};
use crate::verification::{keyframe_interval, probe};
use std::{collections::BTreeMap, ffi::OsString, path::Path, time::Duration};
use tokio_util::sync::CancellationToken;
use vaio_core::domain::export::ExportPlan;

fn value<'a>(args: &'a [OsString], key: &str) -> Option<&'a str> {
    args.windows(2).find(|a| a[0] == key)?.get(1)?.to_str()
}
fn time(args: &[OsString], key: &str) -> Result<Duration, ExecError> {
    value(args, key)
        .and_then(|s| s.parse::<f64>().ok())
        .and_then(|n| Duration::try_from_secs_f64(n).ok())
        .ok_or(ExecError::InvalidPlan)
}

pub(super) async fn expectations(
    plan: &ExportPlan,
    tools: &FfmpegPaths,
    cancel: &CancellationToken,
) -> Result<Vec<(Duration, Duration)>, ExecError> {
    let mut durations = Vec::new();
    let mut pieces = BTreeMap::new();
    for step in &plan.steps {
        let (duration, tolerance) = if value(&step.args, "-f") == Some("concat") {
            let list = value(&step.args, "-i").ok_or(ExecError::InvalidPlan)?;
            let support = plan
                .support_files
                .iter()
                .find(|s| s.path == Path::new(list))
                .ok_or(ExecError::InvalidPlan)?;
            let mut duration = Duration::ZERO;
            let mut tolerance = Duration::ZERO;
            for line in support.contents.lines() {
                let name = line
                    .strip_prefix("file '")
                    .and_then(|s| s.strip_suffix('\''))
                    .ok_or(ExecError::InvalidPlan)?
                    .replace("'\\''", "'");
                let &(d, t) = pieces.get(Path::new(&name)).ok_or(ExecError::InvalidPlan)?;
                duration += d;
                // Each piece may carry a frame/AAC padding or keyframe deviation.
                tolerance += t;
            }
            (duration, tolerance)
        } else {
            let duration = time(&step.args, "-to")?
                .checked_sub(time(&step.args, "-ss")?)
                .filter(|d| !d.is_zero())
                .ok_or(ExecError::InvalidPlan)?;
            let source = Path::new(value(&step.args, "-i").ok_or(ExecError::InvalidPlan)?);
            let media = probe(source, tools, cancel.clone()).await?;
            let tolerance = if value(&step.args, "-c") == Some("copy") {
                // Fast cuts can include the previous keyframe: allow one measured
                // source GOP, rather than assuming every source has a 1 s GOP.
                keyframe_interval(source, &media, tools, cancel.clone()).await?
            } else {
                // Precise cuts allow one source frame plus 0.1 s for mux/audio padding.
                media.frame + Duration::from_millis(100)
            };
            (duration, tolerance)
        };
        pieces.insert(step.final_output.clone(), (duration, tolerance));
        durations.push((duration, tolerance));
    }
    Ok(durations)
}
