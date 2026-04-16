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

        #[arg(long, help = "Don't save to memory (.mullande/.memory)")]
        no_memory: bool,

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
    /// Manage memory (clean, compact, status)
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
}

#[derive(Subcommand)]
pub enum MemoryAction {
    /// Commit current state (for checkpointing before clean operations)
    Clean,
    /// Compact conversation history using LLM
    Compact {
        #[arg(long, help = "Model to use for compaction (default: latest qwen3.5)")]
        model: Option<String>,
    },
    /// Show memory repository status
    Status,
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
           Some(Commands::Run { model, models, judge_model, prompt, timeout, verbose, temperature, top_k, top_p, presence_penalty, think, no_think, tools, no_tools, no_memory, input }) => {
               run_command(model, models, judge_model, prompt, timeout, verbose, temperature, top_k, top_p, presence_penalty, think, no_think, tools, no_tools, no_memory, input, &workspace)
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
          Some(Commands::Memory { action }) => {
              memory_command(action, &workspace)
          }
      }
}

fn run_command(model: Option<String>, models: Option<String>, judge_model: Option<String>, 
               prompt: Option<String>, timeout: Option<u64>, verbose: bool,
               temperature: Option<f32>, top_k: Option<u32>, top_p: Option<f32>, presence_penalty: Option<f32>,
               think: bool, no_think: bool,
               tools: bool, no_tools: bool, no_memory: bool,
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
        if no_memory {
            return Err(anyhow!("--no-memory is not supported with --models (multi-model comparison requires memory for branch isolation)"));
        }

        let model_list: Vec<&str> = models_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if model_list.is_empty() {
            return Err(anyhow!("No valid models specified"));
        }

        use chrono::Local;
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        
        let original_branch = workspace.git_current_branch().unwrap_or_else(|_| "main".to_string());

        let thinking = if think { Some(true) } else if no_think { Some(false) } else { None };
        let cli_params = ModelParams { temperature, top_k, top_p, presence_penalty, thinking };
        let tools_active = tools || (!no_tools && get_config(&workspace.mullande_dir)?.get_model_config(None).tools_enabled.unwrap_or(false));

        let mut results: Vec<(String, String, f64, String)> = Vec::new();
        let mut branch_names: Vec<String> = Vec::new();

        println!("{} Running {} models with git branch isolation...", "►".blue(), model_list.len());

        for model_name in &model_list {
            let safe_name = model_name.replace('/', "-").replace(':', "-");
            let branch_name = format!("model-{}-{}", safe_name, timestamp);
            branch_names.push(branch_name.clone());

            println!("\n{} Processing with model: {} (branch: {})", "→".blue(), model_name, branch_name);

            if let Err(e) = workspace.git_create_branch(&branch_name) {
                println!("{} Failed to create branch for {}: {}", "✗".red(), model_name, e);
                continue;
            }

            if let Err(e) = workspace.git_checkout(&branch_name) {
                println!("{} Failed to checkout branch {}: {}", "✗".red(), branch_name, e);
                continue;
            }

            let mut agent = AgentSystem::new(Some(model_name.to_string()));
            if no_memory {
                agent.set_skip_conversation(true);
            }
            if let Some(t) = timeout {
                agent.set_timeout(std::time::Duration::from_secs(t));
            }
            if cli_params.temperature.is_some() || cli_params.top_k.is_some()
                || cli_params.top_p.is_some() || cli_params.presence_penalty.is_some()
                || cli_params.thinking.is_some() {
                agent.set_model_params(cli_params.clone());
            }
            agent.set_tools_enabled(tools_active);

            match agent.process(&content) {
                Ok(result) => {
                    println!("  {} Completed in {:.2}s", "✓".green(), result.duration_seconds);
                    results.push((model_name.to_string(), result.content, result.duration_seconds, branch_name));
                }
                Err(e) => {
                    println!("{} Model {} failed: {}", "✗".red(), model_name, e);
                }
            }

            if let Err(e) = workspace.git_checkout(&original_branch) {
                println!("{} Warning: Failed to checkout back to {}: {}", "⚠".yellow(), original_branch, e);
            }
        }

        if results.is_empty() {
            return Err(anyhow!("All models failed"));
        }

        if results.len() == 1 {
            println!("\n{} Only one model succeeded, skipping evaluation", "→".yellow());
            let (m, content, duration, branch) = &results[0];
            println!("\n{} Model: {} (branch: {})", "►".blue(), m, branch);
            println!("{} Time: {:.2}s", "►".blue(), duration);
            println!("\n{}", content);
            println!();
            return Ok(());
        }

        let config = get_config(&workspace.mullande_dir)?;
        let default_judge = config.get_judge_model();
        let effective_judge = judge_model.or(default_judge).unwrap_or_else(|| model_list[0].to_string());

        println!("\n{} Evaluating {} outputs with judge model: {}", "►".blue(), results.len(), effective_judge);
        
        let mut judge_agent = AgentSystem::new(Some(effective_judge.clone()));
        judge_agent.set_skip_conversation(true);
        if no_memory {
            judge_agent.set_skip_conversation(true);
        }
        if let Some(t) = timeout {
            judge_agent.set_timeout(std::time::Duration::from_secs(t * 2));
        }

        let mut best_output: Option<(String, String, String, String)> = None;
        let mut best_score: i32 = -1;

        let mut all_outputs = Vec::new();
        for (model_name, output, duration, branch) in &results {
            all_outputs.push(format!("Model: {} (branch: {})\nOutput:\n{}", model_name, branch, output));
        }

        for (idx, (model_name, output, duration, branch)) in results.iter().enumerate() {
            let other_outputs: Vec<String> = all_outputs.iter()
                .enumerate()
                .filter(|(i, _)| *i != idx)
                .map(|(_, o)| o.clone())
                .collect();
            
            let eval_prompt = format!(
                "You are an expert evaluator. Compare the following outputs for the same user request and determine which is better.\n\nUser Request:\n{}\n\n{}\n\nRespond with ONLY the number (1-{}) of the BEST output. Consider accuracy, clarity, helpfulness, and completeness.",
                content,
                all_outputs.iter().enumerate().map(|(i, o)| format!("{}. {}", i + 1, o)).collect::<Vec<_>>().join("\n\n"),
                all_outputs.len()
            );

            match judge_agent.process(&eval_prompt) {
                Ok(eval_result) => {
                    let response = eval_result.content.trim().to_string();
                    let best_num = response.chars().find(|c| c.is_ascii_digit()).and_then(|c| c.to_digit(10)).unwrap_or(1) as usize;
                    
                    let score = if best_num == idx + 1 { 1 } else { 0 };
                    
                    if score > best_score {
                        best_score = score;
                        best_output = Some((model_name.clone(), output.clone(), format!("{:.2}s", duration), branch.clone()));
                    }
                    println!("  {}: chose #{}", model_name, best_num);
                }
                Err(e) => {
                    println!("{} Evaluation failed for {}: {}", "✗".red(), model_name, e);
                }
            }
        }

        let eval_log_path = save_evaluation_log(
            &workspace.mullande_dir,
            &timestamp,
            &effective_judge,
            &model_list,
            &results,
            &best_output,
        )?;

        if let Some((best_model, best_content, best_duration, best_branch)) = best_output {
            println!("\n{} Merging best branch '{}' into main...", "►".blue(), best_branch);
            if let Err(e) = workspace.git_merge(&best_branch) {
                println!("{} Merge warning: {}", "⚠".yellow(), e);
            } else {
                println!("{} Successfully merged branch '{}'", "✓".green(), best_branch);
            }

            println!("\n{} Best output from: {} (branch: {})", "★".green(), best_model, best_branch);
            println!("{} Time: {}", "►".blue(), best_duration);
            println!("\n{}", best_content);
        } else {
            let (first_model, first_content, first_duration, first_branch) = &results[0];
            println!("\n{} Using first output (evaluation inconclusive)", "→".yellow());
            println!("\n{} Model: {} (branch: {})", "►".blue(), first_model, first_branch);
            println!("{} Time: {:.2}s", "►".blue(), first_duration);
            println!("\n{}", first_content);
            
            println!("\n{} Merging branch '{}' into main...", "►".blue(), first_branch);
            if let Err(e) = workspace.git_merge(first_branch) {
                println!("{} Merge warning: {}", "⚠".yellow(), e);
            } else {
                println!("{} Successfully merged", "✓".green());
            }
        }

        println!("\n{} Evaluation log saved to: {}", "►".blue(), eval_log_path);
        println!();
        return Ok(());
    }

    let mut agent = AgentSystem::new(model);
    if no_memory {
        agent.set_skip_conversation(true);
    }
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

fn save_evaluation_log(
    mullande_dir: &Path,
    timestamp: &str,
    judge_model: &str,
    model_list: &[&str],
    results: &[(String, String, f64, String)],
    best_output: &Option<(String, String, String, String)>,
) -> Result<String> {
    let eval_dir = mullande_dir.join("evaluations");
    fs::create_dir_all(&eval_dir)?;
    
    let filename = format!("evaluation_{}.md", timestamp);
    let eval_path = eval_dir.join(&filename);
    
    let mut content = String::new();
    content.push_str(&format!("# Multi-Model Evaluation\n\n"));
    content.push_str(&format!("- **Timestamp**: {}\n", timestamp));
    content.push_str(&format!("- **Judge Model**: {}\n", judge_model));
    content.push_str(&format!("- **Models Evaluated**: {}\n\n", model_list.join(", ")));
    
    content.push_str("## All Outputs\n\n");
    for (i, (model_name, output, duration, branch)) in results.iter().enumerate() {
        content.push_str(&format!("### {}. {} (branch: {})\n", i + 1, model_name, branch));
        content.push_str(&format!("- **Duration**: {:.2}s\n\n", duration));
        content.push_str("```\n");
        content.push_str(output);
        content.push_str("\n```\n\n");
    }
    
    content.push_str("## Evaluation Result\n\n");
    if let Some((best_model, best_output_text, best_duration, best_branch)) = best_output {
        content.push_str(&format!("**Winner**: {} (branch: {})\n", best_model, best_branch));
        content.push_str(&format!("**Duration**: {:.2}s\n\n", best_duration));
        content.push_str("**Output**:\n```\n");
        content.push_str(best_output_text);
        content.push_str("\n```\n");
    } else {
        content.push_str("*Evaluation inconclusive, first output used*\n");
    }
    
    fs::write(&eval_path, content)?;
    Ok(eval_path.to_string_lossy().to_string())
}

fn memory_command(action: MemoryAction, workspace: &WorkspaceManager) -> Result<()> {
    match action {
        MemoryAction::Clean => {
            println!("{} Memory clean: committing current state", "→".blue());
            if workspace.git_has_changes() {
                workspace.git_add(Path::new("."));
                workspace.git_commit("Memory clean: checkpoint before compaction")?;
                println!("{} Committed current state", "✓".green());
            } else {
                println!("{} No changes to commit", "→".yellow());
            }
            Ok(())
        }
        MemoryAction::Compact { model } => {
            println!("{} Compacting conversation history...", "→".blue());
            
            let model_name = if let Some(m) = model {
                m
            } else {
                let client = OllamaClient::new("http://localhost:11434", None);
                match client.list_models() {
                    Ok(models) => {
                        models.into_iter()
                            .filter(|m| m.to_lowercase().contains("qwen3.5"))
                            .max()
                            .unwrap_or_else(|| "qwen3.5".to_string())
                    }
                    Err(_) => "qwen3.5".to_string(),
                }
            };
            println!("{} Using model: {}", "→".blue(), model_name);
            
            let memory = crate::memory::Memory::new(Some(workspace.clone()));
            let history = match memory.load_conversation_history() {
                Ok(h) => h,
                Err(e) => return Err(anyhow!("Failed to load conversation: {}", e)),
            };
            
            if history.is_empty() {
                println!("{} No conversation history to compact", "→".yellow());
                return Ok(());
            }
            
            let conversation_text: String = history.chunks(2)
                .filter(|chunk| chunk.len() == 2)
                .enumerate()
                .map(|(i, chunk)| format!("Turn {}:\nUser: {}\nAssistant: {}\n", i + 1, chunk[0], chunk[1]))
                .collect();
            
            let summary_prompt = format!(
                "Summarize the following conversation concisely, preserving key information, decisions, and important context:\n\n{}\n\nProvide a concise summary (max 500 words):",
                conversation_text
            );
            
            let mut agent = AgentSystem::new(Some(model_name));
            let result = agent.process(&summary_prompt)?;
            
            let compact_content = format!(
                "# Mullande Conversation Log\n\nThis file stores all conversations from mullande run and mullande chat.\n\n---\n\n**Compacted Summary** (model: {})\n\n{}\n",
                result.model, result.content
            );
            
            let mut memory = crate::memory::Memory::new(Some(workspace.clone()));
            if !memory.write_one("CONVERSATIONS.md", &compact_content, "Compact conversation history") {
                return Err(anyhow!("Failed to save compacted conversation"));
            }
            
            println!("{} Compacted conversation history", "✓".green());
            println!("\nSummary:\n{}", result.content);
            Ok(())
        }
        MemoryAction::Status => {
            println!("{} Memory Repository Status\n", "►".blue());
            
            let memory = workspace.get_memory_path();
            println!("  Path: {}", memory.to_string_lossy());
            
            if !memory.exists() {
                println!("  Status: Not initialized");
                return Ok(());
            }
            
            let conversations_md = memory.join("CONVERSATIONS.md");
            if conversations_md.exists() {
                if let Ok(content) = fs::read_to_string(&conversations_md) {
                    let turns = content.matches("**User:**").count();
                    println!("  Conversation turns: {}", turns);
                }
            }
            
            let tool_calls_dir = memory.join("tool_calls");
            if tool_calls_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&tool_calls_dir) {
                    let count = entries.filter_map(|e| e.ok()).filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false)).count();
                    println!("  Tool calls: {}", count);
                }
            }
            
            let subagents_dir = memory.join("subagents");
            if subagents_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&subagents_dir) {
                    let count = entries.filter_map(|e| e.ok()).filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false)).count();
                    println!("  Subagents: {}", count);
                }
            }
            
            let evaluations_dir = workspace.mullande_dir.join("evaluations");
            if evaluations_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&evaluations_dir) {
                    let count = entries.filter_map(|e| e.ok()).filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false)).count();
                    println!("  Evaluations: {}", count);
                }
            }
            
            let output = std::process::Command::new("git")
                .args(&["rev-list", "--count", "HEAD"])
                .current_dir(memory)
                .output();
            if let Ok(o) = output {
                if o.status.success() {
                    let commits = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    println!("  Git commits: {}", commits);
                }
            }
            
            Ok(())
        }
    }
}
