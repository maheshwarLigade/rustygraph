## 📝 Description
Briefly explain the goal and context of this pull request. Link any related issues:
Fixes # (issue)

## 🔍 Type of Change
- [ ] 🐛 Bug fix (non-breaking change fixing an issue)
- [ ] ✨ New feature (non-breaking change adding functionality)
- [ ] 💥 Breaking change (fix or feature causing existing functionality to change)
- [ ] 📖 Documentation update
- [ ] ⚡ Performance optimization
- [ ] 🧪 Tests / CI improvement

## 🧪 Testing Performed
Please describe the tests you ran to verify your changes:
- [ ] Rust Unit & Integration Tests: `cargo test`
- [ ] Clippy Linter: `cargo clippy --all-targets -- -D warnings`
- [ ] Code Formatting: `cargo fmt --check`
- [ ] Go SDK Tests (if touched): `go test ./bindings/go/rustygraph/...`
- [ ] Java SDK Tests (if touched): `./mvnw test` in `bindings/java`

## ✅ Contributor Checklist
- [ ] My code adheres to the style guidelines of this project
- [ ] I have performed a self-review of my code
- [ ] I have commented my code in hard-to-understand areas
- [ ] I have updated the documentation accordingly (e.g. `README.md`, `docs/`)
- [ ] My changes generate no new compiler warnings or Clippy warnings
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] All new and existing tests pass locally
