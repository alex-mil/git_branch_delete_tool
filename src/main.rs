use chrono::{Duration, NaiveDateTime};
use git2::{BranchType, Oid, Repository};
use std::convert::TryFrom;
use std::io::{Bytes, Read, Stdin, Stdout, Write};
use std::{io, string};

fn main() {
    let result = (|| -> Result<_> {
        crossterm::terminal::enable_raw_mode()?;

        let repo = Repository::open_from_env()?;
        let mut stdout = io::stdout();
        let mut stdin = io::stdin().bytes();
        let mut branches = get_branches(&repo)?;

        if branches.is_empty() {
            writeln!(stdout, "The are no branches other than 'master'\r")?;
        } else {
            for branch in &mut branches {
                if branch.is_head {
                    writeln!(stdout, "Current branch is ignored.\r")?;
                } else {
                    match handle_user_input(&mut stdout, &mut stdin, &branch)? {
                        CliAction::Quit => return Ok(()),
                        CliAction::Undo => todo!(),
                        CliAction::Keep => write!(stdout, "")?,
                        CliAction::Delete => {
                            branch.delete()?;
                            writeln!(
                                stdout,
                                "'{}' has deleted, to restore run `git branch {} {}`\r",
                                branch.name, branch.name, branch.id
                            )?;
                        }
                    }
                }
            }
        }
        // Result::<_, CliError>::Ok(()) - another way to set Ok type explicitly
        Ok(())
    })();

    crossterm::terminal::disable_raw_mode().ok();

    match result {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

//noinspection RsTypeCheck
fn get_branches(repo: &Repository) -> Result<Vec<CliBranch>> {
    let mut branches: Vec<_> = repo
        .branches(Some(BranchType::Local))?
        .map(|branch| {
            let (b, _) = branch?;
            let name = String::from_utf8(b.name_bytes()?.to_vec())?;
            let commit = b.get().peel_to_commit()?;
            let time = commit.time();
            let offset = Duration::minutes(i64::from(time.offset_minutes()));
            let time = NaiveDateTime::from_timestamp(time.seconds(), 0) + offset;
            Ok(CliBranch {
                id: commit.id(),
                time,
                name,
                is_head: b.is_head(),
                source: b,
            })
        })
        .filter(|branch| {
            if let Ok(b) = branch {
                b.name != "master"
            } else {
                true
            }
        })
        .collect::<Result<Vec<_>>>()?;
    branches.sort_unstable_by_key(|branch| branch.time);
    Ok(branches)
}

fn handle_user_input(
    stdout: &mut Stdout,
    stdin: &mut Bytes<Stdin>,
    branch: &CliBranch,
) -> Result<CliAction> {
    write!(
        stdout,
        "'{}' ({}) last commit at {} (k/d/q/u/?) >",
        branch.name,
        &branch.id.to_string()[0..10],
        branch.time
    )?;
    stdout.flush()?;
    let byte = match stdin.next() {
        Some(byte) => byte?,
        None => return handle_user_input(stdout, stdin, branch),
    };
    let c = char::from(byte);
    writeln!(stdout, " {}\r", c)?;

    if c == '?' {
        writeln!(stdout, "Available commands:\r")?;
        writeln!(stdout, "k - Keep the branch\r")?;
        writeln!(stdout, "d - Delete the branch\r")?;
        writeln!(stdout, "u - Restore last deleted branch\r")?;
        writeln!(stdout, "q - Quit the programm\r")?;
        writeln!(stdout, "? - Show this help\r")?;
        stdout.flush()?;
        handle_user_input(stdout, stdin, branch)
    } else {
        CliAction::try_from(c)
    }
}

struct CliBranch<'repo> {
    id: Oid,
    time: NaiveDateTime,
    name: String,
    is_head: bool,
    source: git2::Branch<'repo>,
}

impl<'repo> CliBranch<'repo> {
    fn delete(&mut self) -> Result<()> {
        self.source.delete().map_err(From::from)
    }
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Crossterm(#[from] crossterm::ErrorKind),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    FromUtf8(#[from] string::FromUtf8Error),
    #[error(" Error! Invalid action {0}")]
    InvalidInput(char),
}

#[derive(Debug)]
enum CliAction {
    Keep,
    Delete,
    Quit,
    Undo,
}

impl TryFrom<char> for CliAction {
    type Error = CliError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'k' => Ok(CliAction::Keep),
            'd' => Ok(CliAction::Delete),
            'q' => Ok(CliAction::Quit),
            'u' => Ok(CliAction::Undo),
            _ => Err(CliError::InvalidInput(value)),
        }
    }
}

type Result<T, E = CliError> = std::result::Result<T, E>;
