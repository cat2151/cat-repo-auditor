# Agent Instructions

## Project Overview
This is a Python-based repository auditing tool project.

## Development Guidelines

### Python Standards
- **Python Version**: Use Python 3.8 or higher
- **Code Style**: Follow PEP 8 coding standards
- **Type Hints**: Use type hints for all function parameters and return values (PEP 484)
- **Docstrings**: Follow PEP 257 docstring conventions
  - Use triple-quoted strings (`"""..."""`)
  - Include brief description, parameters, return values, and exceptions

### Testing Requirements
- **MUST write tests** for all new functionality
- Use `pytest` as the testing framework
- Place tests in a `tests/` directory
- Test file naming: `test_<module_name>.py`
- Test function naming: `test_<function_description>`
- Aim for high code coverage (target: 80%+)
- Write unit tests, integration tests where applicable
- Mock external dependencies in tests

### Code Organization
- **Single Responsibility Principle**: Each module should have one clear purpose
- **200-Line Rule**: If a source file exceeds 200 lines, consider splitting it
  - Evaluate if the module is doing too many things
  - Extract classes or functions into separate modules
  - Keep related functionality together, but separate concerns
- **Module Structure**: 
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

### Development Workflow
1. **Setup Environment**:
   ```bash
   python -m venv venv
   source venv/bin/activate  # On Windows: venv\Scripts\activate
   pip install -r requirements.txt
   pip install -r requirements-dev.txt  # If exists
   ```

2. **Run Tests**:
   ```bash
   pytest
   pytest --cov=src tests/  # With coverage
   pytest -v  # Verbose output
   ```

3. **Code Quality**:
   ```bash
   # Linting
   pylint src/
   flake8 src/
   
   # Formatting
   black src/ tests/
   
   # Type checking
   mypy src/
   ```

### Commit Guidelines
- Write clear, descriptive commit messages
- Use present tense ("Add feature" not "Added feature")
- Reference issue numbers when applicable
- Keep commits focused and atomic

### Before Committing
- [ ] All tests pass (`pytest`)
- [ ] Code follows PEP 8 style guidelines
- [ ] New functionality has tests
- [ ] Type hints are added
- [ ] Docstrings are complete
- [ ] No files exceed 200 lines (or have justification for doing so)
- [ ] No unused imports or variables
- [ ] Code has been reviewed for potential bugs

### Dependencies
- Keep `requirements.txt` up to date
- Pin major versions, allow minor/patch updates (e.g., `package>=1.2.0,<2.0.0`)
- Separate dev dependencies in `requirements-dev.txt` if needed
- Document any system-level dependencies in README

### Error Handling
- Use specific exception types
- Provide meaningful error messages
- Log errors appropriately
- Handle edge cases and validate inputs

### Documentation
- Update README.md for significant changes
- Document complex algorithms or business logic
- Keep inline comments minimal; prefer self-documenting code
- Update API documentation if applicable

### What NOT to Do
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
