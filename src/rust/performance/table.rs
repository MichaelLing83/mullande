//! Display performance statistics as table

use prettytable::{Table, Row, Cell};
use crate::performance::{PerformanceCollector, ModelStats};
use anyhow::Result;

pub fn show_stats() {
    let mut collector = PerformanceCollector::new();
    let _ = collector.ensure_initialized();
    let mut table = Table::new();
    table.set_titles(Row::new(vec![
        Cell::new("Model").style_spec("cFb"),
        Cell::new("Calls").style_spec("rFb"),
        Cell::new("Avg Duration").style_spec("rFb"),
        Cell::new("Tokens/sec").style_spec("rFb"),
        Cell::new("Avg Input").style_spec("rFb"),
        Cell::new("Avg Output").style_spec("rFb"),
    ]));

    let mut total_calls_all = 0;
    match collector.list_models_with_data() {
        Ok(models) => {
            for model_name in models {
                if let Ok(Some(stats)) = collector.get_model_stats(&model_name) {
                    table.add_row(Row::new(vec![
                        Cell::new(&stats.model_name),
                        Cell::new(&stats.total_calls.to_string()),
                        Cell::new(&format!("{:.2}s", stats.avg_duration_seconds)),
                        Cell::new(&format!("{:.2}", stats.avg_tokens_per_second)),
                        Cell::new(&format!("{:.1}", stats.avg_input_chars)),
                        Cell::new(&format!("{:.1}", stats.avg_output_chars)),
                    ]));
                    total_calls_all += stats.total_calls;
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

            table.printstd();
            println!("\n\x1b[1mTotal recorded calls across all models: {}\x1b[0m", total_calls_all);
        }
        Err(e) => {
            println!("Error listing models: {}", e);
        }
    }
}
