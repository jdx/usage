#!/usr/bin/env -S usage bash
#
# Test fixture to verify behavior of default="" (empty string defaults)
#
# Key findings:
# 1. default="" → Variable IS SET to empty string
# 2. default="value" → Variable IS SET to "value" when not provided
# 3. No default + optional → Variable is UNSET when not provided
#
# This is important for bash scripting:
#   - Use ${var:-default} for "default if unset or empty"
#   - Use ${var-default} for "default if unset only"
#   - Use ${var:?error} to error if unset or empty
#   - Use ${var?error} to error if unset only
#
# Example usage:
#   test-empty-defaults.sh                           # Use all defaults
#   test-empty-defaults.sh --flag-value "val"        # Provide flag value
#   test-empty-defaults.sh arg1 arg2 arg3            # Provide args
#
#USAGE bin "test-defaults"
#USAGE flag "--flag-empty-default <value>" help="Flag with empty string default" default=""
#USAGE flag "--flag-value-default <value>" help="Flag with value default" default="default-value"
#USAGE flag "--flag-no-default <value>" help="Flag without default (optional)"
#USAGE arg "required-empty-default" help="Required arg with empty default" default=""
#USAGE arg "required-value-default" help="Required arg with value default" default="default-arg"
#USAGE arg "[optional-empty-default]" help="Optional arg with empty default" default=""
#USAGE arg "[optional-no-default]" help="Optional arg without default"
set -eo pipefail

# NOTE: We DON'T declare variables here because we want to test
# whether they are actually set by usage or not

echo "=== Testing Variable Set/Unset Behavior ==="
echo ""

# Test flag with empty default
echo "1. Flag with default=\"\":"
if [ -z "${usage_flag_empty_default+x}" ]; then
    echo "   UNSET ❌ (unexpected)"
else
    echo "   SET ✓"
    echo "   Value: '${usage_flag_empty_default}' (empty string)"
fi
echo ""

# Test flag with value default
echo "2. Flag with default=\"value\":"
if [ -z "${usage_flag_value_default+x}" ]; then
    echo "   UNSET ❌ (unexpected)"
else
    echo "   SET ✓"
    echo "   Value: '${usage_flag_value_default}'"
fi
echo ""

# Test flag with no default
echo "3. Flag with no default (optional):"
if [ -z "${usage_flag_no_default+x}" ]; then
    echo "   UNSET ✓ (expected for optional flag without default)"
else
    echo "   SET (value: '${usage_flag_no_default}')"
fi
echo ""

# Test required arg with empty default
echo "4. Required arg with default=\"\":"
if [ -z "${usage_required_empty_default+x}" ]; then
    echo "   UNSET ❌ (unexpected)"
else
    echo "   SET ✓"
    echo "   Value: '${usage_required_empty_default}' (empty string)"
fi
echo ""

# Test required arg with value default
echo "5. Required arg with default=\"value\":"
if [ -z "${usage_required_value_default+x}" ]; then
    echo "   UNSET ❌ (unexpected)"
else
    echo "   SET ✓"
    echo "   Value: '${usage_required_value_default}'"
fi
echo ""

# Test optional arg with empty default
echo "6. Optional arg with default=\"\":"
if [ -z "${usage_optional_empty_default+x}" ]; then
    echo "   UNSET ❌ (unexpected)"
else
    echo "   SET ✓"
    echo "   Value: '${usage_optional_empty_default}' (empty string)"
fi
echo ""

# Test optional arg with no default
echo "7. Optional arg with no default:"
if [ -z "${usage_optional_no_default+x}" ]; then
    echo "   UNSET ✓ (expected for optional arg without default)"
else
    echo "   SET (value: '${usage_optional_no_default}')"
fi
echo ""

echo "=== Testing Parameter Expansion Syntax ==="
echo ""

# Test :- vs - with empty default
echo "8. Parameter expansion with default=\"\" var:"
echo "   \${var:-fallback}  = '${usage_flag_empty_default:-fallback}' (fallback used for empty)"
echo "   \${var-fallback}   = '${usage_flag_empty_default-fallback}' (no fallback, var is set)"
echo ""

# Test :- vs - with unset var
echo "9. Parameter expansion with unset var:"
echo "   \${var:-fallback}  = '${usage_optional_no_default:-fallback}' (fallback used for unset)"
echo "   \${var-fallback}   = '${usage_optional_no_default-fallback}' (fallback used for unset)"
echo ""

# Test :? vs ?
echo "10. Error on empty vs unset:"
echo "   Testing \${usage_flag_empty_default:?} (error if unset or empty):"
if ( : "${usage_flag_empty_default:?}" ) 2>/dev/null; then
    echo "   ✗ No error (unexpected, value is empty so :? should error)"
else
    echo "   ✓ Error thrown (expected, because value is empty string)"
fi
echo ""

echo "   Testing \${usage_flag_empty_default?} (error if unset only):"
if ( : "${usage_flag_empty_default?}" ) 2>/dev/null; then
    echo "   ✓ No error (expected, because var IS set even though empty)"
else
    echo "   ✗ Error thrown (unexpected, var is set)"
fi
echo ""

echo "   Testing \${usage_optional_no_default:?} (error if unset or empty):"
if ( : "${usage_optional_no_default:?}" ) 2>/dev/null; then
    echo "   ✗ No error (unexpected, var is unset so :? should error)"
else
    echo "   ✓ Error thrown (expected, because var is unset)"
fi
echo ""

echo "   Testing \${usage_optional_no_default?} (error if unset only):"
if ( : "${usage_optional_no_default?}" ) 2>/dev/null; then
    echo "   ✗ No error (unexpected, var is unset so ? should error)"
else
    echo "   ✓ Error thrown (expected, because var is unset)"
fi
echo ""

echo "=== Summary ==="
echo ""
echo "Differences between default=\"\" and no default:"
echo "  • default=\"\"      → Variable IS SET to empty string"
echo "  • No default (opt) → Variable is UNSET"
echo ""
echo "When to use each in bash scripts:"
echo "  • Use default=\"\" when you want the variable to always exist"
echo "  • Use no default when you want to distinguish 'not provided' from 'provided empty'"
echo ""
echo "Bash parameter expansion cheat sheet:"
echo "  • \${var:-default} → Use default if var is unset OR empty"
echo "  • \${var-default}  → Use default if var is unset (empty is ok)"
echo "  • \${var:?error}   → Error if var is unset OR empty"
echo "  • \${var?error}    → Error if var is unset (empty is ok)"
