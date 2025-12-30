#!/usr/bin/env -S usage exec python3
#USAGE bin "test-exec-help"
#USAGE flag "-f --force" help="Force the operation"
#USAGE flag "-v --verbose" help="Enable verbose output"
#USAGE arg "<file>" help="File to process"

import os
print(f"force: {os.environ.get('usage_force', '')}")
print(f"verbose: {os.environ.get('usage_verbose', '')}")
print(f"file: {os.environ.get('usage_file', '')}")
