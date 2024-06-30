# Python bindings for Rusty Kaspa 
This project uses [PyO3](https://pyo3.rs/v0.20.0/) and [Maturin](https://www.maturin.rs) to build Rust bindings for Python. The result is a Python package that exposes rusty-kaspa/rust source for use in Python programs.

# Building from Source
1. Ensure Python 3.8 or higher (`python --version`) is installed. [*TODO validate 3.8 or higher is correct*]. Python installers can be found on [python.org](https://www.python.org).
2. `cd ./python` 
3. Create Python virtual environment: `python -m venv env`
4. Activate Python virtual env: 
- Unix-based systems: `source env/bin/activate`
- Windows: `env/scripts/activate.bat`
5. Build Python package with Maturin:
- To build and install in active Python virtual env: `maturin develop --release --features py-sdk`
- To build wheel that can be installed in another virtual env: `maturin build --release --features py-sdk`

# Usage from Python
See Python files in `./python/examples`.


# Project Layout
The Python package `kaspapy` is built from the `kaspa-python` crate, which is located at `./python`. 

As such, the `kaspapy` function in `./python/src/lib.rs` is a good starting point. This function uses PyO3 to add functionality to the package. 
