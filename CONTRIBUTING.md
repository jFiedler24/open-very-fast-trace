# Contributing to OVFT (Open Very Fast Trace)

Thank you for your interest in contributing to OVFT! 🎉

## 🐛 Reporting Bugs

If you find a bug, please create an issue using our [Bug Report template](https://github.com/jFiedler24/open-very-fast-trace/issues/new?template=bug_report.yml).

## ✨ Suggesting Features

Have an idea for a new feature? Please use our [Feature Request template](https://github.com/jFiedler24/open-very-fast-trace/issues/new?template=feature_request.yml).

## ❓ Getting Help

Need help using OVFT? Create a [Question/Support issue](https://github.com/jFiedler24/open-very-fast-trace/issues/new?template=question.yml).

## 🛠️ Development Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/jFiedler24/open-very-fast-trace.git
   cd open-very-fast-trace
   ```

2. **Build the project**:
   ```bash
   cargo build
   ```

3. **Run tests**:
   ```bash
   cargo test
   ```

4. **Install locally**:
   ```bash
   cargo install --path cargo-ovft
   ```

## 📝 Pull Request Guidelines

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Make** your changes
4. **Add** tests for your changes
5. **Ensure** all tests pass (`cargo test`)
6. **Commit** your changes (`git commit -m 'Add amazing feature'`)
7. **Push** to your branch (`git push origin feature/amazing-feature`)
8. **Open** a Pull Request

## 🏗️ Project Structure

```
├── cargo-ovft/          # Cargo plugin binary
├── ovft-core/           # Core library
│   ├── src/
│   │   ├── config.rs    # Configuration management
│   │   ├── core/        # Core tracing logic
│   │   ├── importers/   # File importers (markdown, etc.)
│   │   └── reporters/   # Report generators (HTML, etc.)
│   └── templates/       # HTML templates
├── ovft-example/        # Example project
└── tests/              # Integration tests
```

## 🧪 Testing

- **Unit tests**: `cargo test --lib`
- **Integration tests**: `cargo test --test integration_tests`
- **Example project**: `cd ovft-example && cargo build`

## 📋 Code Style

This project uses:
- **rustfmt** for code formatting: `cargo fmt`
- **clippy** for linting: `cargo clippy`

Please ensure your code passes both before submitting a PR.

## 🤝 Code of Conduct

Be respectful and inclusive. We want everyone to feel welcome contributing to OVFT.

## 📄 License

By contributing, you agree that your contributions will be licensed under the same license as the project.
