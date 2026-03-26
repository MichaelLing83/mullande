//! Display performance statistics as table

use prettytable::{Table, Row, Cell};
use crate::performance::PerformanceCollector;

pub fn show_stats() {
    let mut collector = PerformanceCollector::new();
    let _ = collector.ensure_initialized();
    let mut table = Table::new();
    table.set_titles(Row::new(vec![
        Cell::new("Model").style_spec("cFb"),
        Cell::new("Calls").style_spec("rFb"),
        Cell::new("TTFT").style_spec("rFb"),
        Cell::new("Think Time").style_spec("rFb"),
        Cell::new("Ans Time").style_spec("rFb"),
        Cell::new("Think Toks").style_spec("rFb"),
        Cell::new("Ans Toks").style_spec("rFb"),
        Cell::new("Think/s").style_spec("rFb"),
        Cell::new("Ans/s").style_spec("rFb"),
        Cell::new("Ans/Total").style_spec("rFb"),
    ]));

    let mut tool_table = Table::new();
    tool_table.set_titles(Row::new(vec![
        Cell::new("Model").style_spec("cFb"),
        Cell::new("Tool Calls").style_spec("rFb"),
        Cell::new("Avg Rounds").style_spec("rFb"),
        Cell::new("Tool Toks").style_spec("rFb"),
        Cell::new("Tool/s").style_spec("rFb"),
        Cell::new("Exec Time").style_spec("rFb"),
        Cell::new("Ollama Time").style_spec("rFb"),
    ]));
    let mut has_tool_data = false;

    let mut total_calls_all = 0;
    match collector.list_models_with_data() {
        Ok(models) => {
            for model_name in models {
                if let Ok(Some(stats)) = collector.get_model_stats(&model_name) {
                    table.add_row(Row::new(vec![
                        Cell::new(&stats.model_name),
                        Cell::new(&stats.total_calls.to_string()),
                        Cell::new(&format!("{:.2}s", stats.avg_ttft_seconds)),
                        Cell::new(&format!("{:.1}s", stats.avg_thinking_time_seconds)),
                        Cell::new(&format!("{:.1}s", stats.avg_answering_time_seconds)),
                        Cell::new(&format!("{:.1}", stats.avg_thinking_tokens)),
                        Cell::new(&format!("{:.1}", stats.avg_answering_tokens)),
                        Cell::new(&format!("{:.1}", stats.thinking_tokens_per_second)),
                        Cell::new(&format!("{:.1}", stats.answering_tokens_per_second)),
                        Cell::new(&format!("{:.1}", stats.answering_tokens_per_total_time)),
                    ]));
                    total_calls_all += stats.total_calls;

                    if stats.tool_calls_count > 0 {
                        has_tool_data = true;
                        tool_table.add_row(Row::new(vec![
                            Cell::new(&stats.model_name),
                            Cell::new(&stats.tool_calls_count.to_string()),
                            Cell::new(&format!("{:.1}", stats.avg_tool_call_rounds)),
                            Cell::new(&format!("{:.1}", stats.avg_tool_call_tokens)),
                            Cell::new(&format!("{:.1}", stats.tool_tokens_per_second)),
                            Cell::new(&format!("{:.2}s", stats.avg_tool_exec_time_seconds)),
                            Cell::new(&format!("{:.2}s", stats.avg_tool_ollama_time_seconds)),
                        ]));
                    }
                }
            }

            if table.is_empty() {
                println!("No performance data collected yet.");
                return;
            }

            if let Ok(Some(sys_info)) = collector.get_system_info_cached() {
             println!("\n\x1b[1;34mSystem Information\x1b[0m");
             let os = &sys_info.os;
             let cpu = &sys_info.cpu;
             let mem = &sys_info.memory;
             let ollama_ver = sys_info.ollama_version.as_deref().unwrap_or("Unknown");
             println!("  OS: {} {} {} ({})", os.name, os.release, os.version, os.architecture);
             match (cpu.physical_cores, cpu.logical_cores) {
                 (Some(physical), Some(logical)) => {
                     println!("  CPU: {} physical / {} logical cores", physical, logical);
                 }
                 (None, Some(logical)) => {
                     println!("  CPU: {} logical cores", logical);
                 }
                 (Some(physical), None) => {
                     println!("  CPU: {} physical cores", physical);
                 }
                 (None, None) => {
                     println!("  CPU: Unknown");
                 }
             }
             println!("  Memory: {:.2} GB total", mem.total_gb);
             println!("  Ollama version: {}", ollama_ver);
             println!();
            }

            println!("\n\x1b[1;34mTime Metrics (avg per call)\x1b[0m");
            println!("  \x1b[33mTTFT\x1b[0m = Time To First Token (request to first token)");
            println!("  \x1b[33mThink Time\x1b[0m = Time spent on thinking");
            println!("  \x1b[33mAns Time\x1b[0m = Time spent on answering (after thinking)");
            println!();
            
            table.printstd();
            
            println!("\n\x1b[1;34mToken Metrics (avg per call)\x1b[0m");
            println!("  \x1b[33mThink Toks\x1b[0m = Tokens used for thinking");
            println!("  \x1b[33mAns Toks\x1b[0m = Tokens used for answering");
            println!();
            
            println!("\n\x1b[1;34mSpeed Metrics\x1b[0m");
            println!("  \x1b[33mThink/s\x1b[0m = Thinking tokens / Thinking time");
            println!("  \x1b[33mAns/s\x1b[0m = Answering tokens / Answering time");
            println!("  \x1b[33mAns/Total\x1b[0m = Answering tokens / Total time");
            println!();

            if has_tool_data {
                println!("\n\x1b[1;34mTool Call Metrics (avg per tool-enabled call)\x1b[0m");
                println!("  \x1b[33mTool Calls\x1b[0m = Number of runs that used tools");
                println!("  \x1b[33mAvg Rounds\x1b[0m = Average tool invocations per run");
                println!("  \x1b[33mTool Toks\x1b[0m = Tokens Ollama generated for tool decisions");
                println!("  \x1b[33mTool/s\x1b[0m = Tool-decision tokens / Ollama time (tool rounds)");
                println!("  \x1b[33mExec Time\x1b[0m = Avg time executing tool functions locally");
                println!("  \x1b[33mOllama Time\x1b[0m = Avg Ollama eval time for tool-planning rounds");
                println!();
                tool_table.printstd();
                println!();
            }
            
            println!("\n\x1b[1mTotal recorded calls across all models: {}\x1b[0m", total_calls_all);
        }
        Err(e) => {
            println!("Error listing models: {}", e);
        }
    }
}
