# Open Very Fast Trace 🚀

[![Crates.io](https://img.shields.io/crates/v/cargo-ovft.svg)](https://crates.io/crates/cargo-ovft)
[![Documentation](https://docs.rs/cargo-ovft/badge.svg)](https://docs.rs/cargo-ovft)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://github.com/yourusername/open-very-fast-trace/workflows/CI/badge.svg)](https://github.com/yourusername/open-very-fast-trace/actions)

**A Rust-native requirements tracing solution inspired by and compatible with [OpenFastTrace](https://github.com/itsallcode/openfasttrace).**

## 🙏 **Huge Thanks to OpenFastTrace!**

This project stands on the shoulders of giants! **[OpenFastTrace (OFT)](https://github.com/itsallcode/openfasttrace)** is an incredible requirements tracing tool that has been serving the software development community for years. We owe a massive debt of gratitude to the OFT team for:

- 🎯 **Pioneering requirements tracing** in modern software development
- 📚 **Establishing the OFT format** and tracing methodology
- 🔧 **Creating robust tooling** that works across multiple languages
- 📖 **Comprehensive documentation** and best practices
- 🌟 **Inspiring this Rust implementation** through their excellent design

**👉 Please check out the original [OpenFastTrace project](https://github.com/itsallcode/openfasttrace) - it's fantastic!**

## 🎯 **Why Open Very Fast Trace?**

While OpenFastTrace is excellent, Rust projects have unique needs:

### **🚀 Seamless Rust Integration**
- **Native Cargo plugin**: `cargo ovft` - feels like a built-in Cargo command
- **Build script ready**: Drop into any `build.rs` with zero external dependencies
- **Workspace aware**: Handles multi-crate projects naturally
- **Zero JVM**: No Java runtime required - pure Rust performance

### **⚡ Rust-First Design**
- **Lightning fast**: Native Rust performance, no startup overhead
- **Memory efficient**: Leverages Rust's zero-cost abstractions
- **Type safe**: Catch configuration errors at compile time
- **Async ready**: Built for modern Rust async workflows

### **🔧 Developer Experience**
```bash
# Install once, use everywhere
cargo install cargo-ovft

# Use in any Rust project
cargo ovft

# Integrate into build process
# Just add to build.rs - no external tools needed!
```

## 📦 **Installation**

### **As Cargo Plugin (Recommended)**
```bash
cargo install cargo-ovft
```

### **As Library Dependency**
```toml
[build-dependencies]
ovft-core = "0.1"
```

## 🏗️ **Project Structure**

This workspace contains three projects:

- **`ovft-core/`** - Core library for requirements tracing
- **`cargo-ovft/`** - Cargo plugin for command-line usage  
- **`ovft-example/`** - Example project demonstrating library integration

The modular design allows you to use OVFT in different ways:
- Use `cargo-ovft` as a standalone tool in your CI/CD pipeline
- Integrate `ovft-core` directly into your build scripts
- Reference `ovft-example` for implementation patterns

## 🚀 **Quick Start**

### **1. Tag Your Code**
```rust
// src/auth.rs

// [impl->dsn~authentication~1]
pub fn authenticate_user(token: &str) -> Result<User, AuthError> {
    // [impl->req~secure-validation~1]
    if !is_valid_token(token) {
        return Err(AuthError::InvalidToken);
    }
    
    // Implementation here...
}

// [utest->req~secure-validation~1]
#[cfg(test)]
mod tests {
    #[test]
    fn test_invalid_token_rejected() {
        assert!(authenticate_user("invalid").is_err());
    }
}
```

### **2. Create Requirements Documents**
```markdown
<!-- docs/requirements/auth.md -->

## feat~user-authentication~1

The system shall provide secure user authentication.

**Needs:** req, dsn

## req~secure-validation~1

Authentication tokens must be validated securely.

**Needs:** dsn, impl, utest
**Covers:** feat~user-authentication~1

## dsn~authentication~1

Use JWT tokens with RS256 signing.

**Needs:** impl
**Covers:** req~secure-validation~1
```

### **3. Generate Traceability Report**
```bash
# Using cargo plugin
cargo ovft

# Custom input directory and output file
cargo ovft --input docs/requirements --output trace_report.html

# Check mode - fail if requirements not covered (great for CI!)
cargo ovft --check
```

### **4. Beautiful HTML Reports**

Open `target/requirements_report.html` in your browser to see:

- 📊 **Complete traceability matrix** with coverage status
- 🔗 **Clickable requirement links** - jump between related items
- ⚠️ **Defect detection** - uncovered requirements highlighted
- 🎨 **Professional styling** - easy to read and navigate
- 📈 **Coverage statistics** - see project health at a glance

## 🔧 **Build Integration**

### **Simple build.rs Integration**
```rust
// build.rs
use ovft_core::{Config, Tracer};

fn main() {
    let config = Config::empty()
        .add_source_dir("src")
        .add_spec_dir("docs/requirements");
    
    let tracer = Tracer::new(config);
    let result = tracer.trace().expect("Tracing failed");
    
    // Generate HTML report
    tracer.generate_html_report(&result, "target/requirements_report.html")
        .expect("Report generation failed");
    
    // Fail build if defects found
    if !result.is_success {
        panic!("Found {} requirement defects!", result.defect_count);
    }
    
    println!("✅ All requirements traced successfully!");
}
```

### **GitHub Actions Integration**
```yaml
# .github/workflows/requirements.yml
name: Requirements Tracing

on: [push, pull_request]

jobs:
  requirements:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    
    - name: Install cargo-ovft
      run: cargo install cargo-ovft
      
    - name: Check requirements tracing
      run: cargo ovft --check
      
    - name: Upload traceability report
      uses: actions/upload-artifact@v4
      if: always()
      with:
        name: requirements-report
        path: target/requirements_report.html
```

## 🎯 **OpenFastTrace Compatibility**

Open Very Fast Trace is **fully compatible** with OpenFastTrace syntax:

### **Supported Tag Formats**
```rust
// All these work exactly like in OFT:
// [impl->dsn~module-name~1]
// [utest->req~feature~2] 
// [itest->feat~integration~1]
// [[req~backwards-compat~1]]
// [covers:req~something~1]
```

### **Markdown Requirements**
```markdown
## req~requirement-name~1

Requirements content here.

**Needs:** feat, dsn, impl, utest
**Covers:** feat~parent-feature~1, feat~another~2  
**Tags:** security, performance
```

### **File Support**
- ✅ **Rust files** (`.rs`) - native tag parsing
- ✅ **Markdown** (`.md`) - requirements documents  
- ✅ **Architecture Description Language** (`.adl`)
- ✅ **Architecture Template Language** (`.atl`)

## 🌟 **Features**

- 🔍 **Smart parsing** - understands Rust syntax and OpenFastTrace formats
- 🔗 **Complete linking** - tracks covers/needs relationships automatically
- ⚠️ **Defect detection** - finds orphaned requirements and missing coverage
- 📊 **Rich reporting** - beautiful HTML reports with hyperlinked navigation
- ⚡ **Fast execution** - native Rust performance, no JVM startup
- 🔧 **Zero config** - works out of the box with sensible defaults
- 🎯 **CI/CD ready** - perfect for automated builds and quality gates
- 📦 **Cargo native** - feels like a built-in Cargo command

## 📚 **Documentation**

- 📖 **[User Guide](docs/USER_GUIDE.md)** - Complete usage documentation
- 🔧 **[Integration Guide](docs/INTEGRATION.md)** - Build system integration
- 🎯 **[OpenFastTrace Migration](docs/OFT_MIGRATION.md)** - Coming from OFT
- 🏗️ **[Architecture](docs/ARCHITECTURE.md)** - How it works internally

## 🤝 **Contributing**

We welcome contributions! This project benefits the entire Rust ecosystem.

- 🐛 **Bug reports** - help us improve reliability
- 💡 **Feature requests** - what would make your workflow better?
- 📖 **Documentation** - help others use requirements tracing
- 🔧 **Code contributions** - make it faster, better, more robust

## 📄 **License**

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

## 🙏 **Acknowledgments**

- **[OpenFastTrace Team](https://github.com/itsallcode/openfasttrace)** - for creating the foundation this builds upon
- **Rust Community** - for the amazing ecosystem that makes this possible
- **Contributors** - everyone who helps make requirements tracing better

---

**🔗 Links:**
- 🌟 **[OpenFastTrace](https://github.com/itsallcode/openfasttrace)** - The original and still excellent requirements tracing tool
- 📦 **[Crates.io](https://crates.io/crates/cargo-ovft)** - Install with `cargo install cargo-ovft`
- 📖 **[Documentation](https://docs.rs/cargo-ovft)** - API documentation and examples
- 🐛 **[Issues](https://github.com/yourusername/open-very-fast-trace/issues)** - Bug reports and feature requests
