use brainstone::runtime::OperationId;
use std::path::PathBuf;

const USAGE: &str = "usage: coding_agent demo <workspace> [--crash] | run <workspace> <request-id> <quoted-task> | resume <workspace> <operation-id>";

pub(crate) enum Action {
    Demo { crash: bool },
    Run { request_id: String, task: String },
    Resume(OperationId),
}
pub(crate) struct Options {
    pub(crate) root: PathBuf,
    pub(crate) action: Action,
}
pub(crate) fn options() -> Result<Options, super::Error> {
    parse(std::env::args().skip(1))
}
pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, super::Error> {
    let command = args.next().ok_or(USAGE)?;
    let root = args.next().ok_or(USAGE)?.into();
    let action = match command.as_str() {
        "demo" => demo(args.next().as_deref())?,
        "run" => Action::Run {
            request_id: args.next().ok_or(USAGE)?,
            task: args.next().ok_or(USAGE)?,
        },
        "resume" => Action::Resume(OperationId::new(args.next().ok_or(USAGE)?.parse()?)?),
        _ => return Err(USAGE.into()),
    };
    if args.next().is_some() {
        return Err(USAGE.into());
    }
    Ok(Options { root, action })
}
fn demo(argument: Option<&str>) -> Result<Action, super::Error> {
    match argument {
        None => Ok(Action::Demo { crash: false }),
        Some("--crash") => Ok(Action::Demo { crash: true }),
        _ => Err(USAGE.into()),
    }
}
impl Action {
    pub(crate) const fn is_demo(&self) -> bool {
        matches!(self, Self::Demo { .. })
    }
    pub(crate) const fn crashes(&self) -> bool {
        matches!(self, Self::Demo { crash: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_extra_and_invalid_arguments() {
        for args in [
            vec![],
            vec!["demo"],
            vec!["demo", "repo", "--approve"],
            vec!["demo", "repo", "--crash", "extra"],
            vec!["resume", "repo", "0"],
            vec!["run", "repo", "request"],
        ] {
            assert!(parse(args.into_iter().map(str::to_owned)).is_err());
        }
    }
}
