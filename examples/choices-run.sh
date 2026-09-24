#!/usr/bin/env -S usage bash
# Choices listed by a command: `--help` shows them, a value outside them is refused,
# and completion offers them. A real task might run `docker compose config --services`.
#USAGE bin "build"
#USAGE arg "<service>" help="The service to build" {
#USAGE   choices run="echo app; echo database"
#USAGE }

echo "building $usage_service"
