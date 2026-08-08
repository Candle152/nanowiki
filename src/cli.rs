use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nanowiki", about = "lightweight code knowledge base generator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, PartialEq, Debug)]
pub enum Command {
    /// scan repository and generate baseline docs
    Init,
    /// incrementally update existing docs
    Update,
    /// list all providers and models
    List,
    /// interactively switch default provider / model
    Default,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn parses_init() {
        let cli = Cli::parse_from(["nanowiki", "init"]);
        assert_eq!(cli.command, Some(Command::Init));
    }

    #[test]
    fn parses_update() {
        let cli = Cli::parse_from(["nanowiki", "update"]);
        assert_eq!(cli.command, Some(Command::Update));
    }

    #[test]
    fn parses_list() {
        let cli = Cli::parse_from(["nanowiki", "list"]);
        assert_eq!(cli.command, Some(Command::List));
    }

    #[test]
    fn parses_default() {
        let cli = Cli::parse_from(["nanowiki", "default"]);
        assert_eq!(cli.command, Some(Command::Default));
    }

    #[test]
    fn parses_no_subcommand() {
        let cli = Cli::parse_from(["nanowiki"]);
        assert_eq!(cli.command, None);
    }

    #[test]
    fn help_contains_commands() {
        let mut cmd = Cli::command();
        let help = cmd.render_help().to_string();
        for word in ["init", "update", "list", "default"] {
            assert!(help.contains(word), "help should contain '{}'", word);
        }
    }
}
