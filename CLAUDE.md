# PMCP SDK Development Standards

## Toyota Way Quality System - ZERO TOLERANCE FOR DEFECTS

We have ZERO tolerance for defects. Your "clippy warnings won't..." is a P0 problem.

## Quality Gate Enforcement

### Pre-Commit Quality Gates (MANDATORY)
**ALL commits are blocked until quality gates pass:**
- Pre-commit hook automatically runs Toyota Way quality checks
- Format checking: `cargo fmt --check`  
- Clippy analysis: Zero warnings allowed
- Build verification: Must compile successfully
- Doctest validation: All doctests must pass

**To commit code:**
```bash
make pre-commit-gate  # Run before any commit
git add -A
git commit -m "message"  # Will be blocked if quality fails
```

### PMAT Quality-Gate Proxy Mode (REQUIRED DURING DEVELOPMENT)

**MANDATORY: Use pmat quality-gate proxy via MCP during development**

All code changes MUST go through pmat quality-gate proxy before writing:

```bash
# Start pmat MCP server with quality-gate proxy
pmat mcp-server --enable-quality-proxy

# In Claude Code, use quality_proxy MCP tool for all file operations:
# - write operations
# - edit operations  
# - append operations
```

**Quality Proxy Enforcement Modes:**
- **Strict Mode** (default): Reject code that doesn't meet quality standards
- **Advisory Mode**: Warn about quality issues but allow changes
- **Auto-Fix Mode**: Automatically refactor code to meet standards

**Quality Checks Applied:**
- Cognitive complexity limits (≤25 per function)
- Zero SATD (Self-Admitted Technical Debt) comments
- Comprehensive documentation requirements
- Lint violation prevention
- Automatic refactoring suggestions

## Task Management - PDMT Style

**MANDATORY: Use PDMT (Pragmatic Deterministic MCP Templating) for all todos**

### PDMT Todo Generation
Use the `pdmt_deterministic_todos` MCP tool for creating quality-enforced todo lists:

```bash
# Generate PDMT todos with quality enforcement
pdmt_deterministic_todos --requirement "implement feature X" --mode strict --coverage-target 80
```

**PDMT Todo Features:**
- **Quality Gates Built-in**: Each todo includes validation commands
- **Success Criteria**: Clear, measurable completion requirements  
- **Test Coverage**: Enforce 80%+ coverage targets
- **Zero SATD**: No technical debt tolerance
- **Complexity Limits**: Automatic complexity validation
- **Documentation**: Comprehensive docs required

### PDMT Todo Structure
```
## Todo: [ID] Implementation Task
**Quality Gate**: `cargo test --coverage && cargo clippy`
**Success Criteria**: 
- [ ] Feature implemented with 80%+ test coverage
- [ ] Zero clippy warnings
- [ ] Comprehensive documentation with examples
- [ ] Property tests included
- [ ] Integration tests passing
**Validation Command**: `make quality-gate && make test-coverage`
```

## Development Workflow (MANDATORY)

### 1. Planning Phase
- Use `pdmt_deterministic_todos` for task breakdown
- Set quality targets: 80%+ coverage, zero SATD, complexity ≤25

### 2. Development Phase  
- **ALL code changes via pmat quality-gate proxy**
- Use MCP `quality_proxy` tool for file operations
- Continuous quality validation during development

### 3. Pre-Commit Phase
- Pre-commit hook enforces Toyota Way quality gates
- **Cannot commit** without passing all quality checks
- Zero tolerance: formatting, clippy, build, tests

### 4. CI/CD Phase
- Tests run with `--test-threads=1` (race condition prevention)
- Full quality gate validation
- Documentation coverage verification

## Quality Standards Summary

✅ **Zero tolerance for defects**
✅ **Pre-commit quality gates enforced**  
✅ **PMAT quality-gate proxy mandatory during development**
✅ **PDMT style todos with built-in quality gates**
✅ **Toyota Way principles: Jidoka, Genchi Genbutsu, Kaizen**
✅ **80%+ test coverage with quality doctests**
✅ **Cognitive complexity ≤25 per function**
✅ **Zero SATD comments allowed**
✅ **Comprehensive documentation required**

## Emergency Override (USE WITH EXTREME CAUTION)
```bash
# Only for critical hotfixes - requires justification
git commit --no-verify -m "HOTFIX: critical issue - bypassing quality gates"
```

**Note**: Emergency overrides require immediate follow-up commits to restore quality standards.