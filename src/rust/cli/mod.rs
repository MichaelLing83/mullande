//! mullande - Large Model Agent System command line interface

use std::fs;
use std::path::Path;
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::agent::AgentSystem;
use crate::config::{get_config, Config, ModelConfig, ModelParams};
use crate::workspace::WorkspaceManager;
use crate::logging::Logger;
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

        #[arg(long, help = "Multiple models to compare (comma-separated). Will evaluate and return the best output")]
        models: Option<String>,

        #[arg(long, help = "Judge model to evaluate outputs. If not specified, uses first model in --models")]
        judge_model: Option<String>,

        #[arg(short, long)]
        prompt: Option<String>,

        #[arg(short, long, help = "Timeout in seconds for model response")]
        timeout: Option<u64>,

        #[arg(short, long, help = "Enable verbose output for debugging")]
        verbose: bool,

        #[arg(long, help = "Sampling temperature (e.g. 0.7); overrides config")]
        temperature: Option<f32>,

        #[arg(long, help = "Top-K sampling; overrides config")]
        top_k: Option<u32>,

        #[arg(long, help = "Top-P (nucleus) sampling (e.g. 0.9); overrides config")]
        top_p: Option<f32>,

        #[arg(long, help = "Presence penalty; overrides config")]
        presence_penalty: Option<f32>,

        #[arg(long, help = "Enable thinking/reasoning mode; overrides config", conflicts_with = "no_think")]
        think: bool,

        #[arg(long, help = "Disable thinking/reasoning mode; overrides config", conflicts_with = "think")]
        no_think: bool,

        #[arg(long, help = "Enable tool calling (read_file, write_file, bash, glob, grep)", conflicts_with = "no_tools")]
        tools: bool,

        #[arg(long, help = "Disable tool calling; overrides config", conflicts_with = "tools")]
        no_tools: bool,

        input: Option<String>,
    },
    /// Show performance statistics collected from previous runs
    Stats,
    /// Show, validate, or interactively edit configuration
    Config {
        output: Option<String>,

        #[arg(long)]
        check: bool,

        edit: bool,

        #[arg(short, long, help = "Import models from external source: currently only 'ollama' is supported")]
        import: Option<String>,

        #[arg(long, help = "When importing from ollama, interactively select new cloud models to add (don't synchronize, just add selected)")]
        cloud: bool,
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
    // Initialize logging directory
    let logger = Logger::new(workspace.clone());
    let _ = logger.initialize();

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
           Some(Commands::Run { model, models, judge_model, prompt, timeout, verbose, temperature, top_k, top_p, presence_penalty, think, no_think, tools, no_tools, input }) => {
               run_command(model, models, judge_model, prompt, timeout, verbose, temperature, top_k, top_p, presence_penalty, think, no_think, tools, no_tools, input, &workspace)
            }
          Some(Commands::Stats) => {
              stats_command()
          }
          Some(Commands::Config { output, check, edit, import, cloud }) => {
              config_command(output, check, edit, import, cloud, &workspace)
          }
          Some(Commands::Version) => {
              println!("mullande v{}", env!("CARGO_PKG_VERSION"));
              Ok(())
          }
      }
}

fn run_command(model: Option<String>, models: Option<String>, judge_model: Option<String>, 
               prompt: Option<String>, timeout: Option<u64>, verbose: bool,
               temperature: Option<f32>, top_k: Option<u32>, top_p: Option<f32>, presence_penalty: Option<f32>,
               think: bool, no_think: bool,
               tools: bool, no_tools: bool,
               input: Option<String>, workspace: &WorkspaceManager) -> Result<()> {
    if let Some(ref model_name) = model {
        let mut config = get_config(&workspace.mullande_dir)?;
        config.data.model.model_id = Some(model_name.clone());
        config.save(None)?;
        println!("{} Model '{}' saved as default", "✓".green(), model_name);
    }

    if let Some(ref judge_model_name) = judge_model {
        let mut config = get_config(&workspace.mullande_dir)?;
        config.data.judge_model = Some(judge_model_name.clone());
        config.save(None)?;
        println!("{} Judge model '{}' saved as default", "✓".green(), judge_model_name);
    }

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

    if let Some(models_str) = models {
        let model_list: Vec<&str> = models_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if model_list.is_empty() {
            return Err(anyhow!("No valid models specified"));
        }

        println!("{} Running {} models in parallel...", "►".blue(), model_list.len());

        use std::sync::Arc;
        use std::thread;

        let content = Arc::new(content);
        let thinking = if think { Some(true) } else if no_think { Some(false) } else { None };
        let cli_params = ModelParams { temperature, top_k, top_p, presence_penalty, thinking };
        let tools_active = tools || (!no_tools && get_config(&workspace.mullande_dir)?.get_model_config(None).tools_enabled.unwrap_or(false));

        let mut handles = vec![];
        for model_name in &model_list {
            let content = Arc::clone(&content);
            let model_str = model_name.to_string();
            let timeout_val = timeout;
            let params = cli_params.clone();
            let tools_en = tools_active;
            
            handles.push(thread::spawn(move || {
                let mut agent = AgentSystem::new(Some(model_str.clone()));
                if let Some(t) = timeout_val {
                    agent.set_timeout(std::time::Duration::from_secs(t));
                }
                if params.temperature.is_some() || params.top_k.is_some()
                    || params.top_p.is_some() || params.presence_penalty.is_some()
                    || params.thinking.is_some() {
                    agent.set_model_params(params);
                }
                agent.set_tools_enabled(tools_en);
                agent.process(&content)
            }));
        }

        let mut results: Vec<(String, String, f64)> = Vec::new();
        for (i, handle) in handles.into_iter().enumerate() {
            match handle.join() {
                Ok(Ok(result)) => {
                    results.push((model_list[i].to_string(), result.content, result.duration_seconds));
                }
                Ok(Err(e)) => {
                    println!("{} Model {} failed: {}", "✗".red(), model_list[i], e);
                }
                Err(e) => {
                    println!("{} Model {} panicked: {:?}", "✗".red(), model_list[i], e);
                }
            }
        }

        if results.is_empty() {
            return Err(anyhow!("All models failed"));
        }

        if results.len() == 1 {
            println!("\n{} Only one model succeeded, skipping evaluation", "→".yellow());
            let (m, content, duration) = &results[0];
            println!("\n{} Model: {}", "►".blue(), m);
            println!("{} Time: {:.2}s", "►".blue(), duration);
            println!("\n{}", content);
            println!();
            return Ok(());
        }

        println!("\n{} Evaluating {} outputs with judge model...", "►".blue(), results.len());
        
        let config = get_config(&workspace.mullande_dir)?;
        let default_judge = config.get_judge_model();
        let effective_judge = judge_model.or(default_judge).unwrap_or_else(|| model_list[0].to_string());
        
        let mut judge_agent = AgentSystem::new(Some(effective_judge.clone()));
        if let Some(t) = timeout {
            judge_agent.set_timeout(std::time::Duration::from_secs(t * 2));
        }

        let mut best_output: Option<(String, String, String)> = None;
        let mut best_score: i32 = -1;

        for (model_name, output, duration) in &results {
            let eval_prompt = format!(
                "You are an expert evaluator. Compare the following outputs for the same user request and determine which is better.\n\nUser Request: {}\n\nOutput A (from {}): {}\n\nOutput B (from {}): {}\n\nRespond with ONLY a single letter 'A' if Output A is better, or 'B' if Output B is better. Consider accuracy, clarity, helpfulness, and completeness.",
                content, model_name, output, "previous", "previous"
            );

            let eval_result = judge_agent.process(&eval_prompt)?;
            let response = eval_result.content.trim().to_uppercase();
            
            let score = if response.starts_with('A') { 1 } else { 0 };
            
            if score > best_score {
                best_score = score;
                best_output = Some((model_name.clone(), output.clone(), format!("{:.2}s", duration)));
            }
        }

        if let Some((best_model, best_content, best_duration)) = best_output {
            println!("\n{} Best output from: {}", "★".green(), best_model);
            println!("{} Time: {}", "►".blue(), best_duration);
            println!("\n{}", best_content);
        } else {
            let (first_model, first_content, first_duration) = &results[0];
            println!("\n{} Using first output (evaluation inconclusive)", "→".yellow());
            println!("\n{} Model: {}", "►".blue(), first_model);
            println!("{} Time: {:.2}s", "►".blue(), first_duration);
            println!("\n{}", first_content);
        }
        println!();
        return Ok(());
    }

    let mut agent = AgentSystem::new(model);
    if let Some(timeout) = timeout {
        agent.set_timeout(std::time::Duration::from_secs(timeout));
    }
    agent.set_verbose(verbose);

    let thinking = if think { Some(true) } else if no_think { Some(false) } else { None };
    let cli_params = ModelParams { temperature, top_k, top_p, presence_penalty, thinking };
    if cli_params.temperature.is_some() || cli_params.top_k.is_some()
        || cli_params.top_p.is_some() || cli_params.presence_penalty.is_some()
        || cli_params.thinking.is_some() {
        agent.set_model_params(cli_params);
    }

    let config_tools = agent.model_config().tools_enabled.unwrap_or(false);
    let tools_active = if tools { true } else if no_tools { false } else { config_tools };
    agent.set_tools_enabled(tools_active);

    let result = agent.process(&content)?;
    println!("\n{} Model: {}", "►".blue(), result.model);
    println!("{} Input tokens: ~{}", "►".blue(), result.input_tokens);
    println!("{} Time: {:.2}s", "►".blue(), result.duration_seconds);
    println!("\n{}", result.content);
    println!();
    Ok(())
}

fn stats_command() -> Result<()> {
    crate::performance::show_stats();
    Ok(())
}

fn config_command(output: Option<String>, check: bool, edit: bool, import: Option<String>, cloud: bool, workspace: &WorkspaceManager) -> Result<()> {
    let config = get_config(&workspace.mullande_dir)?;

     if let Some(source) = import {
         if source == "ollama" {
             return import_ollama_models(config, cloud, workspace);
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

fn import_ollama_models(mut config: Config, import_cloud: bool, _workspace: &WorkspaceManager) -> Result<()> {
    use dialoguer::MultiSelect;

    let base_url = config.data.model.base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
    let api_key = config.get_api_key(None);
    let client = OllamaClient::new(&base_url, api_key);

    println!("{} Fetching models from ollama...", "→".blue());
    let all_models = match client.list_models() {
        Ok(models) => models,
        Err(e) => {
            return Err(anyhow!("Failed to fetch models from ollama: {}\nMake sure ollama is running and accessible.", e));
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

    // If not cloud mode: import all found models synchronize (delete not found)
    if !import_cloud {
        for model_name in &all_models {
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
                temperature: None,
                top_k: None,
                top_p: None,
                presence_penalty: None,
                thinking: None,
                tools_enabled: None,
            });
            added += 1;
        }

        if let Some(models) = &config.data.models {
            for existing in models.keys() {
                if !all_models.contains(existing) && !existing.ends_with(":cloud") {
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
            println!("\nRemoved models that are no longer present (kept :cloud models):");
            for name in deleted {
                println!("  - {}", name);
            }
        }
    } else {
        // Cloud mode: separate existing into local vs cloud
        // All models come from the same (cloud) instance, filter new ones and let user select
        let new_models: Vec<String> = all_models.into_iter()
            .filter(|m| !existing_models.contains(m))
            .collect();

        if new_models.is_empty() {
            println!("{} No new models found on cloud that are not already configured.", "✓".green());
            return Ok(());
        }

        println!("\n{} Found {} new models on the cloud ollama server:", "→".blue(), new_models.len());
        for model in &new_models {
            println!("  - {}", model);
        }
        println!();

        let selected = MultiSelect::new()
            .with_prompt("Select the models you want to add to configuration (space to toggle, enter to confirm)")
            .items(&new_models)
            .interact()?;

        if selected.is_empty() {
            println!("{} No models selected, nothing changed.", "→".yellow());
            return Ok(());
        }

        // Add selected models
        if config.data.models.is_none() {
            config.data.models = Some(std::collections::HashMap::new());
        }

        let models = config.data.models.as_mut().unwrap();
        for idx in &selected {
            let model_name = &new_models[*idx];
            models.insert(model_name.clone(), ModelConfig {
                provider: "ollama".to_string(),
                model_id: Some(model_name.clone()),
                base_url: Some(base_url.clone()),
                context_window: None,
                api_key_env: None,
                temperature: None,
                top_k: None,
                top_p: None,
                presence_penalty: None,
                thinking: None,
                tools_enabled: None,
            });
            added += 1;
        }

        config.save(None)?;

        let total = if let Some(models) = &config.data.models {
            models.len()
        } else {
            0
        };

        println!("\n{} Import complete:", "✓".green());
        println!("  Selected: {} models added", added);
        println!("  Total:    {} models", total + 1);
        println!("\nAdded models:");
        for idx in &selected {
            println!("  + {}", new_models[*idx]);
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
        temperature: None,
        top_k: None,
        top_p: None,
        presence_penalty: None,
        thinking: None,
        tools_enabled: None,
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
                    temperature: None,
                    top_k: None,
                    top_p: None,
                    presence_penalty: None,
                    thinking: None,
                    tools_enabled: None,
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
