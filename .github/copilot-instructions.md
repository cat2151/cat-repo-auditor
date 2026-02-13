# GitHub Copilot Instructions for cat-repo-auditor

This file provides repository-wide custom instructions for GitHub Copilot to help generate code, pull requests, and automate tasks aligned with this repository's standards.

## Project Overview
This is a Python-based repository auditing tool project.

## Python Standards
- Use Python 3.10 or higher
- Follow PEP 8 coding standards
- Use type hints for all function parameters and return values (PEP 484)
- Follow PEP 257 docstring conventions:
  - Use triple-quoted strings (`"""..."""`)
  - Include brief description, parameters, return values, and exceptions

## Code Organization
- Follow Single Responsibility Principle: Each module should have one clear purpose
- If a source file exceeds 200 lines, consider splitting it
- Use the following module structure:
  ```
  src/
    module_name/
      __init__.py
      core.py
      utils.py
      exceptions.py
  tests/
    test_module_name.py
  ```

## Testing Requirements
- **MUST write tests** for all new functionality
- Use `pytest` as the testing framework
- Place tests in a `tests/` directory
- Test file naming: `test_<module_name>.py`
- Test function naming: `test_<function_description>`
- Target 80%+ code coverage
- Write unit tests, integration tests where applicable
- Mock external dependencies in tests

## Development Commands
- Setup environment: `python -m venv venv && source venv/bin/activate && pip install -r requirements.txt`
- Run tests: `pytest` or `pytest --cov=src tests/` for coverage
- Linting: `pylint src/` and `flake8 src/`
- Formatting: `black src/ tests/`
- Type checking: `mypy src/`

## Dependencies
- Pin major versions, allow minor/patch updates (e.g., `package>=1.2.0,<2.0.0`)
- Separate dev dependencies in `requirements-dev.txt` if needed

## Error Handling
- Use specific exception types
- Provide meaningful error messages
- Log errors appropriately
- Handle edge cases and validate inputs

## Documentation
- Update README.md for significant changes
- Document complex algorithms or business logic
- Keep inline comments minimal; prefer self-documenting code

## What NOT to Do
- Don't commit sensitive information (API keys, passwords)
- Don't commit `__pycache__/`, `*.pyc`, or `.venv/` directories
- Don't modify `.github/workflows/` without understanding the CI/CD pipeline
- Don't introduce breaking changes without proper versioning
- Don't skip writing tests
- Don't ignore linter warnings without good reason

## Repository-Specific Notes
- GitHub Actions workflows are pre-configured for issue notes and large file detection
- Workflows call reusable workflows from `cat2151/github-actions` repository
- Large file detection runs daily at 03:00 JST (18:00 UTC)
- Issue notes are automatically generated when new issues are created

## Before Committing
Ensure all of the following are completed:
- [ ] All tests pass (`pytest`)
- [ ] Code follows PEP 8 style guidelines
- [ ] New functionality has tests
- [ ] Type hints are added
- [ ] Docstrings are complete
- [ ] No files exceed 200 lines (or have justification for doing so)
- [ ] No unused imports or variables
- [ ] Code has been reviewed for potential bugs
