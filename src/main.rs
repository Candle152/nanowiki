//! NanoWiki — lightweight code knowledge base generator

use clap::Parser;
use dialoguer::{Select, theme::ColorfulTheme};
use nanowiki::agent;
use nanowiki::cli::{Cli, Command};
use nanowiki::config;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = config::load_or_create()?;
    config::validate(&cfg)?;

    match cli.command {
        Some(Command::List) => {
            print_status(&cfg);
        }
        Some(Command::Default) => {
            interactive_default(&cfg)?;
        }
        Some(Command::Init) | Some(Command::Update) => {
            let (name, pc) = cfg.resolve()?;
            let model = cfg.resolve_model(pc);
            let client = agent::Client::from_provider_config(pc)?;
            let repo_root = env::current_dir()?;

            let is_init = matches!(cli.command, Some(Command::Init));
            if is_init {
                println!("NanoWiki init — {} / {}\n", name, model);
                agent::run_init(client, &repo_root, &model).await?;
            } else {
                println!("NanoWiki update — {} / {}\n", name, model);
                agent::run_update(client, &repo_root, &model).await?;
            }
        }
        None => {
            print_status(&cfg);
            println!();
            println!("nanowiki --help to see all commands");
        }
    }

    Ok(())
}

fn interactive_default(cfg: &config::Config) -> anyhow::Result<()> {
    let mut cfg = cfg.clone();
    let mut items: Vec<(String, String, String)> = vec![];
    let mut current_idx = 0usize;

    for (name, pc) in &cfg.providers {
        for m in &pc.models {
            let is_current = Some(name.as_str()) == cfg.default_provider.as_deref()
                && Some(m.as_str()) == cfg.current_model.as_deref();
            let marker = if is_current { "❯" } else { " " };
            items.push((format!("{} {} / {}", marker, name, m), name.clone(), m.clone()));
            if is_current {
                current_idx = items.len() - 1;
            }
        }
    }

    let display: Vec<&str> = items.iter().map(|(d, _, _)| d.as_str()).collect();

    let selection = match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select model (arrow keys to move, Enter to confirm, Esc to cancel)")
        .default(current_idx)
        .items(&display)
        .interact_opt()?
    {
        Some(s) => s,
        None => return Ok(()),
    };

    let (_, provider, model) = &items[selection];
    config::switch_by_model(&mut cfg, model)?;
    config::save(&cfg)?;
    println!("Switched → {} / {}", provider, model);
    Ok(())
}

fn print_status(cfg: &config::Config) {
    if cfg.providers.is_empty() {
        println!("(no providers configured)");
        return;
    }

    for (name, pc) in &cfg.providers {
        let star = if Some(name.as_str()) == cfg.default_provider.as_deref() { "*" } else { " " };
        println!("  {} {}", star, name);
        for m in &pc.models {
            let mark = if Some(m.as_str()) == cfg.current_model.as_deref()
                && Some(name.as_str()) == cfg.default_provider.as_deref()
            {
                "← current"
            } else {
                ""
            };
            println!("    {} {}", m, mark);
        }
    }
}
