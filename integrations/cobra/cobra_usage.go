package cobra_usage

import (
	"encoding/json"
	"os"

	"github.com/spf13/cobra"
)

// Generate converts a Cobra command tree into a usage spec in KDL format.
func Generate(cmd *cobra.Command) string {
	spec := convertRoot(cmd)
	return renderKDL(&spec)
}

// GenerateJSON converts a Cobra command tree into a usage spec in JSON format,
// matching usage-lib's JSON schema with a root "cmd" object and map-based subcommands.
func GenerateJSON(cmd *cobra.Command) ([]byte, error) {
	spec := convertRoot(cmd)
	js := toJSON(&spec)
	return json.MarshalIndent(js, "", "  ")
}

// GenerateToFile converts a Cobra command tree and writes the KDL spec to a file.
func GenerateToFile(cmd *cobra.Command, path string) error {
	kdl := Generate(cmd)
	return os.WriteFile(path, []byte(kdl), 0644)
}

// GenerateJSONToFile converts a Cobra command tree and writes the JSON spec to a file.
func GenerateJSONToFile(cmd *cobra.Command, path string) error {
	data, err := GenerateJSON(cmd)
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}
