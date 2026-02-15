// Example CLI app demonstrating cobra_usage integration.
//
// Run:
//
//	go run . --help
//	go run . --usage-spec
//	go run . --usage-spec | usage generate md -f -
package main

import (
	"fmt"
	"os"

	cobra_usage "github.com/jdx/usage/integrations/cobra"
	"github.com/spf13/cobra"
)

func main() {
	root := &cobra.Command{
		Use:     "deploy-tool",
		Short:   "A deployment management tool",
		Long:    "deploy-tool manages deployments across multiple environments with rollback support.",
		Version: "0.1.0",
	}

	root.PersistentFlags().BoolP("verbose", "v", false, "Enable verbose output")

	// deploy subcommand
	deploy := &cobra.Command{
		Use:   "deploy <service>",
		Short: "Deploy a service",
		Long:  "Deploy a service to the specified environment. Defaults to staging if --env is not set.",
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("Deploying %s\n", args[0])
		},
	}
	deploy.Flags().StringP("env", "e", "staging", "Target environment")
	deploy.Flags().Bool("force", false, "Force deployment without confirmation")
	deploy.Flags().Bool("dry-run", false, "Show what would be deployed without making changes")

	// rollback subcommand
	rollback := &cobra.Command{
		Use:   "rollback <service> [version]",
		Short: "Rollback a service to a previous version",
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("Rolling back %s\n", args[0])
		},
	}

	// config subcommand with nested get/set
	config := &cobra.Command{
		Use:   "config",
		Short: "Manage configuration",
	}
	configGet := &cobra.Command{
		Use:   "get <key>",
		Short: "Get a configuration value",
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("Config: %s\n", args[0])
		},
	}
	configSet := &cobra.Command{
		Use:   "set <key> <value>",
		Short: "Set a configuration value",
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("Set %s = %s\n", args[0], args[1])
		},
	}
	config.AddCommand(configGet, configSet)

	// status subcommand with aliases
	status := &cobra.Command{
		Use:     "status [service]",
		Short:   "Show deployment status",
		Aliases: []string{"st", "info"},
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Println("Status: OK")
		},
	}

	root.AddCommand(deploy, rollback, config, status)

	// Handle --usage-spec: check for the flag in os.Args before Execute.
	// This is the recommended integration pattern.
	for _, arg := range os.Args[1:] {
		if arg == "--usage-spec" {
			fmt.Print(cobra_usage.Generate(root))
			return
		}
	}

	if err := root.Execute(); err != nil {
		os.Exit(1)
	}
}
