# Contributing to nexus-actor-rs

Thank you for your interest in contributing to nexus-actor-rs! This document provides guidelines and information for contributors.

## Project Structure

The project is organized into several main components:

- `core/`: Core actor system implementation
  - Actor lifecycle management
  - Message serialization
  - Supervisor strategies
  - Process registry
  - Event stream
  
- `remote/`: Remote actor communication
  - Remote process management
  - Network communication
  - Message routing

- `cluster/`: Clustering support
  - Member management
  - Cluster events
  - Node coordination

## Development Workflow

1. Fork the repository
2. Create a feature branch using the format: `feature/description`
3. Implement your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## Commit Message Format

We use conventional commits for versioning. Commit messages should follow the pattern:
```
type(scope): message

Types: build|ci|feat|fix|docs|style|refactor|perf|test|revert|chore
```

Example: `feat(core): add new supervisor strategy`

## Code Style

- Use `cargo fmt` for code formatting
- Follow Rust standard naming conventions
- Add documentation for public APIs
- Include unit tests for new functionality

## Testing

- Write unit tests for new features
- Include integration tests for component interactions
- Test both success and failure cases
- Verify distributed system behavior

## Documentation

- Update README.md for significant changes
- Document public APIs with rustdoc
- Include examples for new features
- Keep documentation in sync with code changes
