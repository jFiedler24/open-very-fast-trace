use std::env;
use std::path::PathBuf;

// [impl->feat~example-usage~1]
fn main() {
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=../docs/requirements/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_root = PathBuf::from(&manifest_dir);
    
    // Use the library directly
    match generate_report(&project_root) {
        Ok(()) => {
            println!("cargo:warning=✅ Requirements report generated at target/requirements_report.html");
        }
        Err(e) => {
            println!("cargo:warning=❌ Failed to generate report: {}", e);
        }
    }
}

fn generate_report(project_root: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use ovft_core::{Config, Tracer};
    
    // Create configuration - point to workspace root for source files
    let workspace_root = project_root.parent().unwrap();
    let config = Config::empty()
        .add_source_dir(workspace_root.join("ovft-core/src").to_string_lossy().to_string())
        .add_spec_dir(workspace_root.join("docs/requirements").to_string_lossy().to_string());
    
    // Create tracer and run tracing
    let tracer = Tracer::new(config);
    let trace_result = tracer.trace()?;
    
    // Generate HTML report
    let output_path = project_root.join("target/requirements_report.html");
    tracer.generate_html_report(&trace_result, &output_path)?;
    
    // Print summary
    if trace_result.is_success {
        println!("cargo:warning=✅ No defects found - all requirements properly traced");
    } else {
        println!("cargo:warning=⚠️ {} defects found - see report for details", trace_result.defect_count);
    }
    
    Ok(())
}
