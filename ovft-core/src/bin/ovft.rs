use ovft_core::{Config, Tracer};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        process::exit(1);
    }

    let mut source_dirs = Vec::new();
    let mut spec_dirs = Vec::new();
    let mut output_path = PathBuf::from("requirements_report.html");
    let mut config_file = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--source-dirs" => {
                if i + 1 < args.len() {
                    source_dirs = args[i + 1].split(',').map(PathBuf::from).collect();
                    i += 2;
                } else {
                    eprintln!("Error: --source-dirs requires a value");
                    process::exit(1);
                }
            }
            "--spec-dirs" => {
                if i + 1 < args.len() {
                    spec_dirs = args[i + 1].split(',').map(PathBuf::from).collect();
                    i += 2;
                } else {
                    eprintln!("Error: --spec-dirs requires a value");
                    process::exit(1);
                }
            }
            "--output" => {
                if i + 1 < args.len() {
                    output_path = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("Error: --output requires a value");
                    process::exit(1);
                }
            }
            "--config" => {
                if i + 1 < args.len() {
                    config_file = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --config requires a value");
                    process::exit(1);
                }
            }
            "--help" => {
                print_help(&args[0]);
                process::exit(0);
            }
            _ => {
                eprintln!("Error: Unknown option '{}'", args[i]);
                process::exit(1);
            }
        }
    }

    // Load configuration - either from specified file, auto-discover .ovft.toml, or use defaults
    let mut config = if let Some(config_path) = config_file {
        match Config::from_file(&config_path) {
            Ok(config) => {
                println!("Loaded configuration from: {}", config_path.display());
                config
            }
            Err(e) => {
                eprintln!("Error loading configuration from {}: {}", config_path.display(), e);
                process::exit(1);
            }
        }
    } else {
        let loaded_config = Config::load_or_default();
        if Config::load_from_current_dir().is_some() {
            println!("Found and loaded .ovft.toml configuration");
        }
        loaded_config
    };

    // Override configuration with command line arguments
    if !source_dirs.is_empty() {
        config.source_dirs = source_dirs;
    }
    
    if !spec_dirs.is_empty() {
        config.spec_dirs = spec_dirs;
    }
    
    if let Some(output_parent) = output_path.parent() {
        config.output_dir = Some(output_parent.to_path_buf());
    }

    // Create tracer and run analysis
    let tracer = Tracer::new(config);

    println!("Running requirements tracing...");
    let trace_result = match tracer.trace() {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error during tracing: {}", e);
            process::exit(1);
        }
    };

    // Print summary
    println!("Found {} items", trace_result.total_items);
    println!("Defects: {}", trace_result.defect_count);
    println!("Success: {}", trace_result.is_success);

    if trace_result.defect_count > 0 {
        println!("\nDefects found:");
        for defect in &trace_result.defects {
            println!("  - {:?}: {}", defect.defect_type, defect.description);
        }
    }

    // Generate HTML report
    println!("Generating HTML report at {}...", output_path.display());
    if let Err(e) = tracer.generate_html_report(&trace_result, &output_path) {
        eprintln!("Error generating HTML report: {}", e);
        process::exit(1);
    }

    println!("HTML report generated successfully!");

    if trace_result.defect_count > 0 {
        process::exit(1); // Exit with error code if defects found
    }
}

fn print_usage(program_name: &str) {
    println!("Usage: {} [OPTIONS]", program_name);
    println!("Options:");
    println!("  --source-dirs <dirs>   Source directories to scan (comma separated)");
    println!("  --spec-dirs <dirs>     Specification directories to scan (comma separated)");
    println!("  --output <file>        Output HTML file path");
    println!("  --config <file>        Path to configuration file (.ovft.toml)");
    println!("  --help                 Show this help message");
}

fn print_help(program_name: &str) {
    println!("Open Very Fast Trace - Requirements Tracing Tool");
    println!();
    println!("Usage: {} [OPTIONS]", program_name);
    println!();
    println!("Options:");
    println!("  --source-dirs <dirs>   Source directories to scan (comma separated)");
    println!("  --spec-dirs <dirs>     Specification directories to scan (comma separated)");
    println!("  --output <file>        Output HTML file path (default: requirements_report.html)");
    println!("  --config <file>        Path to configuration file (.ovft.toml)");
    println!("                         If not specified, looks for .ovft.toml in current or parent directories");
    println!("  --help                 Show this help message");
    println!();
    println!("Configuration File:");
    println!("  Create a .ovft.toml file to configure file extensions, source directories,");
    println!("  and requirements directories. Command line options override configuration file settings.");
}
