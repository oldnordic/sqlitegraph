# Contributing

## Project Model

This project is centrally maintained by a single developer. External contributions are not accepted at this time. Issues and pull requests from external sources will be closed without review.

## Internal Development Workflow

### Development Methodology

- **Test-Driven Development**: Tests are written before implementation
- **Deterministic Engineering**: All algorithms must produce reproducible results
- **No Guessing**: Implementation must be based on verified requirements
- **Minimal Diffs**: Changes are kept small and focused
- **Comprehensive Testing**: All code paths must have test coverage

### Quality Gates

Before committing changes, run the full validation pipeline:

```bash
cargo fmt --all
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

### Performance Requirements

- Benchmark regressions must pass gates in `sqlitegraph_bench.json`
- Performance changes require baseline updates with justification
- Deterministic ordering is required for all graph operations

### Code Organization

- Module files limited to 300 lines maximum
- Strict module boundaries with clear interfaces
- No circular dependencies between modules
- Internal invariants documented in code comments

## Commit and Branching Rules

### Branch Strategy

- Single `main` branch maintained by owner
- No long-lived feature branches
- All development happens on short-lived branches

### Merge Policy

- Fast-forward merges only
- No merge commits
- Linear history required

### Commit Messages

- Short, factual descriptions (50 characters max)
- No explanatory body unless necessary
- Reference issue numbers when applicable

## Code Practices

### Testing Requirements

- No mocks or stubs for critical paths
- Real backends only (no fakes for production code)
- Integration tests must use actual SQLite databases
- Deterministic test data and ordering

### Algorithm Requirements

- All graph algorithms must be deterministic
- Stable sorting for all collections
- No reliance on undefined behavior
- Memory safety is mandatory

### Documentation

- Public APIs must have rustdoc comments
- Internal invariants documented inline
- Examples must be tested and functional

## Future Considerations

External contributions may be accepted in the future. Until then, this policy remains in effect. Any changes to contribution policy will be announced in project releases.