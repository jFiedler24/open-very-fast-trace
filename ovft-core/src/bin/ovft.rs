use ovft_core::{Config, Tracer};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} [OPTIONS]", args[0]);
        println!("Options:");
        println!("  --source-dirs <dirs>   Source directories to scan (comma separated)");
        println!("  --spec-dirs <dirs>     Specification directories to scan (comma separated)");
        println!("  --output <file>        Output HTML file path");
        println!("  --help                 Show this help message");
        process::exit(1);
    }

    let mut source_dirs = Vec::new();
    let mut spec_dirs = Vec::new();
    let mut output_path = PathBuf::from("requirements_report.html");

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
            "--help" => {
                println!("Open Very Fast Trace - Requirements Tracing Tool");
                println!();
                println!("Usage: {} [OPTIONS]", args[0]);
                println!();
                println!("Options:");
                println!("  --source-dirs <dirs>   Source directories to scan (comma separated)");
                println!(
                    "  --spec-dirs <dirs>     Specification directories to scan (comma separated)"
                );
                println!("  --output <file>        Output HTML file path (default: requirements_report.html)");
                println!("  --help                 Show this help message");
                process::exit(0);
            }
            _ => {
                eprintln!("Error: Unknown option '{}'", args[i]);
                process::exit(1);
            }
        }
    }

    // Build configuration
    let mut config = Config::empty();

    for source_dir in source_dirs {
        config = config.add_source_dir(source_dir);
    }

    for spec_dir in spec_dirs {
        config = config.add_spec_dir(spec_dir);
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
