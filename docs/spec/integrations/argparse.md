# argparse (Python)

[`argparse-usage`](https://github.com/acidghost/argparse-usage) converts a Python [argparse](https://docs.python.org/3/library/argparse.html) `ArgumentParser` into a usage spec.

## Installation

```bash
pip install argparse-usage
# or
uv add argparse-usage
```

## Quick Start

```python
import argparse
import argparse_usage

parser = argparse.ArgumentParser(prog='mycli', description='My CLI tool')
parser.add_argument('-v', '--verbose', action='count', default=0)
parser.add_argument('files', nargs='+', help='Files to process')

spec = argparse_usage.generate(
    parser,
    name='My CLI',
    version='1.0.0',
    author='Your Name',
)
print(spec)
```

Then pipe the output to `usage`:

```bash
python mycli.py --usage-spec | usage generate completion bash
python mycli.py --usage-spec | usage generate md --out-file docs.md
python mycli.py --usage-spec | usage generate manpage --out-file mycli.1
```

## API

| Function | Description |
| --- | --- |
| `generate(parser, name=None, version=None, author=None, bin_name=None)` | Returns the usage spec as a KDL string |

## Feature Mapping

| argparse | Usage Spec |
| --- | --- |
| `action='store_true'` / `'store_false'` | Bool flag (no arg child) |
| `action='count'` | `count=#true` |
| `nargs='+'` | Variadic arg with minimum |
| Positional arguments | `arg` nodes |
| Subparsers | `cmd` nodes |
| Parent parsers | Inherited arguments |
