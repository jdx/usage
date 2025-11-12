#!/usr/bin/env -S usage bash
#
# Test script for blank comment lines in USAGE blocks
# This verifies that blank comment lines don't stop parsing
#
#USAGE bin "test-blank-lines"
#USAGE long_about """
#USAGE This script tests that blank comment lines
#USAGE in USAGE blocks don't stop the parser.
#USAGE """
#
#USAGE arg "[workspace]" help="Workspace name" default="default-ws"
#USAGE flag "-r --region <region>" help="AWS region" default="us-west-2"
#
#
#USAGE flag "-t --tail" help="Follow logs in real-time"
set -eo pipefail

# Declare variables set by usage to avoid SC2154
: "${usage_workspace:=}"
: "${usage_region:=}"
: "${usage_tail:=}"

echo "workspace: $usage_workspace"
echo "region: $usage_region"
echo "tail: $usage_tail"
