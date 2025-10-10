use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};
use ovft_core::{Config, Tracer};
use std::env;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    env_logger::init();

    let app = Command::new("cargo-ovft")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Open Very Fast Trace - Requirements traceability for Rust projects")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("ovft")
                .about("Run requirements traceability analysis")
                .arg(
                    Arg::new("input")
                        .short('i')
                        .long("input")
                        .value_name("DIR")
                        .help("Input directory containing requirements files")
                        .default_value("."),
                )
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("FILE")
                        .help("Output HTML report file")
                        .default_value("requirements_report.html"),
                )
                .arg(
                    Arg::new("format")
                        .short('f')
                        .long("format")
                        .value_name("FORMAT")
                        .help("Output format")
                        .value_parser(["html", "json"])
                        .default_value("html"),
                )
                .arg(
                    Arg::new("verbose")
                        .short('v')
                        .long("verbose")
                        .help("Enable verbose output")
                        .action(clap::ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("check")
                        .short('c')
                        .long("check")
                        .help("Check for issues and return non-zero exit code if found")
                        .action(clap::ArgAction::SetTrue),
                ),
        );

    let matches = app.try_get_matches()?;

    match matches.subcommand() {
        Some(("ovft", sub_matches)) => run_ovft(sub_matches),
        _ => unreachable!(),
    }
}

fn run_ovft(matches: &ArgMatches) -> Result<()> {
    let input_dir = matches.get_one::<String>("input").unwrap();
    let output_file = matches.get_one::<String>("output").unwrap();
    let format = matches.get_one::<String>("format").unwrap();
    let verbose = matches.get_flag("verbose");
    let check_mode = matches.get_flag("check");

    if verbose {
        println!("🔍 Running OVFT requirements traceability analysis");
        println!("📁 Input directory: {}", input_dir);
        println!("📄 Output file: {}", output_file);
        println!("📋 Format: {}", format);
    }

    // Find Cargo.toml to determine project root
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let project_root =
        find_cargo_project_root(&current_dir).context("Not in a Cargo project directory")?;

    if verbose {
        println!("🏠 Project root: {}", project_root.display());
    }

    // Create configuration using the builder pattern
    let config = Config::default()
        .add_source_dir("src")
        .add_spec_dir(input_dir)
        .output_dir(PathBuf::from(output_file).parent().unwrap())
        .verbose(verbose);

    // Run the tracer
    let tracer = Tracer::new(config);
    let trace_result = tracer
        .trace()
        .context("Failed to run requirements traceability analysis")?;

    if verbose {
        println!("✅ Analysis complete!");
        println!("📊 Requirements found: {}", trace_result.items.len());
        println!("🔗 Total items: {}", trace_result.total_items);

        if trace_result.defect_count > 0 {
            println!("❌ Defects found: {}", trace_result.defect_count);
            for defect in &trace_result.defects {
                println!("   - {:?}: {}", defect.defect_type, defect.description);
            }
        }

        // Print coverage summary
        for (artifact_type, summary) in &trace_result.coverage_summary {
            println!(
                "📊 {}: {}/{} ({:.1}% coverage)",
                artifact_type, summary.covered, summary.total, summary.percentage
            );
        }
    }

    // Generate report
    if format == "html" {
        let output_path = PathBuf::from(output_file);
        tracer
            .generate_html_report(&trace_result, &output_path)
            .context("Failed to generate HTML report")?;
        println!("📄 HTML report generated: {}", output_file);
    } else {
        // For JSON format, output the trace result data
        let json_data = serde_json::json!({
            "total_items": trace_result.total_items,
            "defect_count": trace_result.defect_count,
            "defects": trace_result.defects,
            "coverage_summary": trace_result.coverage_summary,
            "is_success": trace_result.is_success,
            "coverage_percentage": trace_result.coverage_percentage()
        });

        let json = serde_json::to_string_pretty(&json_data)
            .context("Failed to serialize result to JSON")?;
        std::fs::write(output_file, json).context("Failed to write JSON output")?;
        println!("📄 JSON report generated: {}", output_file);
    }

    // Check mode: exit with error if issues found
    if check_mode {
        if trace_result.defect_count > 0 {
            eprintln!(
                "❌ Found {} defects in requirements traceability",
                trace_result.defect_count
            );
            std::process::exit(1);
        } else {
            println!("✅ No requirements traceability issues found");
        }
    }

    Ok(())
}

fn find_cargo_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();

    loop {
        if current.join("Cargo.toml").exists() {
            return Some(current);
        }

        if !current.pop() {
            break;
        }
    }

    None
}
