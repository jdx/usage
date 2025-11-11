#!/usr/bin/env -S usage node
//
// Test script for new USAGE syntax with // comments
//
// [USAGE] bin "test-double-slash"
// [USAGE] flag "--debug" help="Debug mode"
// [USAGE] flag "--port <port>" help="Port number" default="3000"
// [USAGE] arg "command" help="Command to run"

console.log("This would be a JavaScript file");
console.log("debug:", process.env.usage_debug);
console.log("port:", process.env.usage_port);
console.log("command:", process.env.usage_command);
