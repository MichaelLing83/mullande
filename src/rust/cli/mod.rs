//! mullande - Large Model Agent System command line interface

use std::fs;
use std::path::Path;
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::Colorize;
use prettytable::{Table, Row, Cell};

use crate::agent::AgentSystem;
use crate::config::{get_config, Config, ModelConfig};
use crate::workspace::WorkspaceManager;
use crate::agent::ollama::OllamaClient;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(infer_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    config: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the Agent system with the given input
    Run {
        #[arg(short, long)]
        model: Option<String>,

        #[arg(short, long)]
        prompt: Option<String>,

        input: Option<String>,
    },

    /// Start interactive chat session with Agent
    Chat,

    /// Show, validate, or interactively edit configuration
    Config {
        #[arg(short, long)]
        output: Option<String>,

        #[arg(long)]
        check: bool,

        #[arg(long)]
        edit: bool,

        #[arg(short, long)]
        import: Option<String>,
    },

    /// Show version information
    Version,
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut workspace = WorkspaceManager::new(None);
    if !workspace.is_initialized() {
        println!("{}", "Initializing mullande workspace...".yellow());
        workspace.initialize()?;
        println!("{} {}", "Workspace initialized at".green(), workspace.get_memory_path().to_string_lossy());
        println!();
    }

    match cli.command {
        None => {
            println!("mullande v{} - Large Model Agent System", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Use 'mullande --help' or 'mullande -h' to see available commands");
            println!();
            println!("To enable shell autocompletion:");
            println!("  Bash:  echo 'eval \"\\$(_MULLANDE_COMPLETE=bash_source mullande)\"' >> ~/.bashrc");
            println!("  Zsh:   echo 'eval \"\\$(_MULLANDE_COMPLETE=zsh_source mullande)\"' >> ~/.zshrc");
            println!("  Fish:  echo '_MULLANDE_COMPLETE=fish_source mullande | source' >> ~/.config/fish/completions/mullande.fish");
            Ok(())
        }
        Some(Commands::Run { model, prompt, input }) => {
            run_command(model, prompt, input)
        }
        Some(Commands::Chat) => {
            chat_command()
        }
        Some(Commands::Config { output, check, edit, import }) => {
            config_command(output, check, edit, import, &workspace)
        }
        Some(Commands::Version) => {
            println!("mullande v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn run_command(model: Option<String>, prompt: Option<String>, input: Option<String>) -> Result<()> {
    let mut agent = AgentSystem::new(model);

    let content = match (input, prompt) {
        (Some(input), _) => {
            let input_path = Path::new(&input);
            if input_path.exists() && input_path.is_file() {
                fs::read_to_string(input_path)?
            } else {
                input
            }
        }
        (None, Some(prompt)) => prompt,
        (None, None) => {
            let mut content = String::new();
            std::io::stdin().read_line(&mut content)?;
            if content.is_empty() {
                println!("Please provide input via argument (file or text), --prompt, or stdin");
                return Ok(());
            }
            content
        }
    };

    let result = agent.process(&content)?;
    println!("\n{} Model: {}", "►".blue(), result.model);
    println!("{} Input tokens: ~{}", "►".blue(), result.input_tokens);
    println!("{} Time: {:.2}s", "►".blue(), result.duration_seconds);
    println!("\n{}", result.content);
    println!();
    Ok(())
}

fn chat_command() -> Result<()> {
    println!("{}", "Starting interactive chat session (Ctrl+C to exit)".yellow());
    let mut agent = AgentSystem::new(None);

    let stdin = std::io::stdin();
    loop {
        print!("{} ", "You >".blue());
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut prompt = String::new();
        match stdin.read_line(&mut prompt) {
            Ok(0) => break,
            Ok(_) => {
                let prompt = prompt.trim();
                if prompt.starts_with('/') {
                    handle_special_command(prompt, &mut agent);
                    continue;
                }
                if prompt.is_empty() {
                    continue;
                }
                 match agent.process(prompt) {
                     Ok(result) => {
                         println!("\n{} {}", "Agent >".green(), result.content);
                     }
                     Err(e) => {
                         println!("\n{} {}", "Error:".red(), e);
                     }
                 }
            }
            Err(e) => {
                println!("{} {}", "Error reading input:".red(), e);
                break;
            }
        }
    }
    println!("\nExiting chat...");
    Ok(())
}

fn handle_special_command(cmd: &str, agent: &mut AgentSystem) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let command = parts[0].to_lowercase();

    match command.as_str() {
        "/models" => cmd_list_models(agent),
        "/model" => {
            if parts.len() < 2 {
                println!("{} Usage: /model <model_name>", "Agent >".green());
                println!("{} Current model: {}", "Agent >".green(), agent.effective_model_id());
            } else {
                cmd_switch_model(parts[1], agent);
            }
        }
        "/stats" => {
            crate::performance::show_stats();
        }
        "/version" => {
            println!("{} mullande version: {}", "Agent >".green(), env!("CARGO_PKG_VERSION"));
        }
        "/config" => {
            let workspace = WorkspaceManager::default();
            let config = get_config(&workspace.mullande_dir);
            if let Ok(config) = config {
                println!("{} Current configuration:", "Agent >".green());
                println!("{}", config.to_json().unwrap());
            }
        }
        "/help" => {
            println!("{} Available special commands:", "Agent >".green());
            println!("  {}{} {}", "/models".bold(), "          ", "List all configured models");
            println!("  {}{} {}", "/model <name>".bold(), "   ", "Switch to specified model");
            println!("  {}{} {}", "/stats".bold(), "          ", "Show performance statistics");
            println!("  {}{} {}", "/version".bold(), "        ", "Show mullande version");
            println!("  {}{} {}", "/config".bold(), "         ", "Show current configuration");
            println!("  {}{} {}", "/help".bold(), "           ", "Show this help message");
            println!("  {}{} {}", "/exit".bold(), "          ", "Exit interactive chat");
        }
        "/exit" => {
            println!("{} Exiting chat...", "Agent >".green());
            std::process::exit(0);
        }
        _ => {
            println!("{} Unknown command: {}", "Agent >".red(), command);
            println!("{} Type /help to see available commands", "Agent >".green());
        }
    }
}

fn cmd_list_models(agent: &AgentSystem) {
    let default_model = agent.effective_model_id();
    let mut table = Table::new();
    table.set_titles(Row::new(vec![
        Cell::new("Model").style_spec("cFb"),
        Cell::new("Provider").style_spec("gFb"),
        Cell::new("Default").style_spec("yFb"),
    ]));

    let config = &agent.config;
    table.add_row(Row::new(vec![
        Cell::new(&config.data.model.model_id.clone().unwrap_or_default()),
        Cell::new(&config.data.model.provider),
        Cell::new("*default*"),
    ]));

    if let Some(models) = &config.data.models {
        for (name, model_config) in models {
            table.add_row(Row::new(vec![
                Cell::new(name),
                Cell::new(&model_config.provider),
                Cell::new(""),
            ]));
        }
    }

    table.printstd();
    println!();
    println!("Current active model: \x1b[1;36m{}\x1b[0m", default_model);
}

fn cmd_switch_model(model_name: &str, agent: &mut AgentSystem) {
    agent.requested_model = Some(model_name.to_string());
    println!("{} Switched to model: \x1b[1;36m{}\x1b[0m", "✅".green(), model_name);
}

fn config_command(output: Option<String>, check: bool, edit: bool, import: Option<String>, workspace: &WorkspaceManager) -> Result<()> {
    let config = get_config(&workspace.mullande_dir)?;

    if let Some(source) = import {
        if source == "ollama" {
            return import_ollama_models(config, workspace);
        }
    }

    if let Some(output_path) = output {
        config.save(Some(Path::new(&output_path)))?;
        println!("{} Configuration exported to {}", "✓".green(), output_path);
        return Ok(());
    }

    if check {
        println!("{} Configuration is valid", "✓".green());
        return Ok(());
    }

    if edit {
        return create_config_interactive(config, workspace);
    }

    println!("{}", config.to_json()?);
    Ok(())
}

fn import_ollama_models(mut config: Config, workspace: &WorkspaceManager) -> Result<()> {
    let base_url = config.data.model.base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
    let api_key = config.get_api_key(None);
    let client = OllamaClient::new(&base_url, api_key);

    println!("{} Fetching models from local ollama...", "→".blue());
    let local_models = match client.list_models() {
        Ok(models) => models,
        Err(e) => {
            return Err(anyhow!("Failed to fetch models from ollama: {}\nMake sure ollama is running.", e));
        }
    };

    let mut existing_models: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(models) = &config.data.models {
        for name in models.keys() {
            existing_models.insert(name.clone());
        }
    }

    let mut added = 0;
    let mut skipped = 0;
    let mut deleted: Vec<String> = Vec::new();

    for model_name in &local_models {
        if existing_models.contains(model_name) {
            skipped += 1;
            continue;
        }

        if config.data.models.is_none() {
            config.data.models = Some(std::collections::HashMap::new());
        }

        let models = config.data.models.as_mut().unwrap();
        models.insert(model_name.clone(), ModelConfig {
            provider: "ollama".to_string(),
            model_id: Some(model_name.clone()),
            base_url: Some(base_url.clone()),
            context_window: None,
            api_key_env: None,
        });
        added += 1;
    }

    if let Some(models) = &config.data.models {
        for existing in models.keys() {
            if !local_models.contains(existing) {
                deleted.push(existing.clone());
            }
        }
    }

    if !deleted.is_empty() {
        if let Some(models) = &mut config.data.models {
            for name in &deleted {
                models.remove(name);
            }
        }
    }

    config.save(None)?;

    let total = if let Some(models) = &config.data.models {
        models.len()
    } else {
        0
    };

    println!("{} Import complete:", "✓".green());
    println!("  Added:   {}", added);
    println!("  Skipped:  {}", skipped);
    println!("  Deleted:  {}", deleted.len());
    println!("  Total:    {} models", total + 1);

    if !deleted.is_empty() {
        println!("\nRemoved models that are not present in local ollama:");
        for name in deleted {
            println!("  - {}", name);
        }
    }

    Ok(())
}

fn create_config_interactive(mut config: Config, workspace: &WorkspaceManager) -> Result<()> {
    use dialoguer::{Input, Select, Confirm};

    println!("=== {} Interactive Configuration Creation ===", "mullande".yellow());
    println!("Note: Authentication information (API keys) should be stored in environment variables,");
    println!("not in the configuration file. We'll just ask for the environment variable name.\n");

    let providers = vec!["ollama", "volcengine", "copilot"];
    let provider_idx = Select::new()
        .with_prompt("Default model provider")
        .items(&providers)
        .default(0)
        .interact()?;
    let provider = providers[provider_idx].to_string();

    let default_model_id = if provider == "ollama" { "llama3" } else { "" };
    let model_id: String = Input::new()
        .with_prompt("Default model ID")
        .default(default_model_id.to_string())
        .interact_text()?;
    let model_id = if model_id.is_empty() { None } else { Some(model_id) };

    let mut base_url = None;
    if provider == "ollama" {
        let url: String = Input::new()
            .with_prompt("Ollama base URL")
            .default("http://localhost:11434".to_string())
            .interact_text()?;
        base_url = Some(url);
    }

    let mut api_key_env = None;
    if provider == "volcengine" || provider == "copilot" {
        let default_env = match provider.as_str() {
            "volcengine" => "VOLCENGINE_API_KEY",
            "copilot" => "GITHUB_TOKEN",
            _ => "",
        };
        let env: String = Input::new()
            .with_prompt("Environment variable containing API key")
            .default(default_env.to_string())
            .interact_text()?;
        api_key_env = if env.is_empty() { None } else { Some(env) };
    }

    let context_window: Option<u32> = if Confirm::new()
        .with_prompt("Configure custom context window for default model?")
        .default(false)
        .interact()? {
            let cw: u32 = Input::new()
                .with_prompt("Context window size")
                .default(4096)
                .interact_text()?;
            Some(cw)
        } else {
            None
        };

    let global_context_window: Option<u32> = if Confirm::new()
        .with_prompt("Configure global default context window?")
        .default(false)
        .interact()? {
            let cw: u32 = Input::new()
                .with_prompt("Global context window size")
                .default(4096)
                .interact_text()?;
            Some(cw)
        } else {
            None
        };

    let default_model = ModelConfig {
        provider,
        model_id,
        base_url,
        context_window,
        api_key_env,
    };

    let mut models = config.data.models.take().unwrap_or_default();

    if Confirm::new()
        .with_prompt("Add additional model configurations?")
        .default(false)
        .interact()? {
            loop {
                let model_name: String = Input::new()
                    .with_prompt("Model ID (enter to stop adding)")
                    .default(String::new())
                    .interact_text()?;
                if model_name.is_empty() {
                    break;
                }

                println!("Configuring {}:", model_name);

                let p_idx = Select::new()
                    .with_prompt(format!("Provider for {}", model_name))
                    .items(&providers)
                    .default(0)
                    .interact()?;
                let p = providers[p_idx].to_string();

                let mid: String = Input::new()
                    .with_prompt(format!("Model ID for {}", model_name))
                    .default(String::new())
                    .interact_text()?;
                let mid = if mid.is_empty() { None } else { Some(mid) };

                let mut bu = None;
                if p == "ollama" {
                    let url: String = Input::new()
                        .with_prompt(format!("Base URL for {}", model_name))
                        .default("http://localhost:11434".to_string())
                        .interact_text()?;
                    bu = Some(url);
                }

                let mut ake = None;
                if p == "volcengine" || p == "copilot" {
                    let default_de = match p.as_str() {
                        "volcengine" => "VOLCENGINE_API_KEY",
                        "copilot" => "GITHUB_TOKEN",
                        _ => "",
                    };
                    let de: String = Input::new()
                        .with_prompt("Environment variable with API key")
                        .default(default_de.to_string())
                        .interact_text()?;
                    ake = if de.is_empty() { None } else { Some(de) };
                }

                let cw: Option<u32> = if Confirm::new()
                    .with_prompt(format!("Custom context window for {}?", model_name))
                    .default(false)
                    .interact()? {
                        let c: u32 = Input::new()
                            .with_prompt("Context window size")
                            .default(4096)
                            .interact_text()?;
                        Some(c)
                    } else {
                        None
                    };

                models.insert(model_name.clone(), ModelConfig {
                    provider: p,
                    model_id: mid,
                    base_url: bu,
                    context_window: cw,
                    api_key_env: ake,
                });

                if !Confirm::new()
                    .with_prompt("Add another model?")
                    .default(false)
                    .interact()? {
                    break;
                }
            }
        }

    config.data.model = default_model;
    config.data.models = if models.is_empty() { None } else { Some(models) };
    config.data.global_context_window = global_context_window;

    let config_path = workspace.mullande_dir.join("config.json");
    config.save(Some(&config_path))?;

    println!("\n{} Configuration saved to {}", "✓".green(), config_path.to_string_lossy());
    println!("\nNew configuration:");
    println!("{}", config.to_json()?);

    Ok(())
}
