import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitepress";
import spec from "../cli/reference/commands.json";
import kdlGrammar from "./grammars/kdl.tmLanguage.json";

function getCommands(cmd): string[][] {
  const commands = [];
  for (const [name, sub] of Object.entries(cmd.subcommands)) {
    if (sub.hide) continue;
    commands.push(sub.full_cmd);
    commands.push(...getCommands(sub));
  }
  return commands;
}

const commands = getCommands(spec.cmd);
const configDir = dirname(fileURLToPath(import.meta.url));
const cargoToml = readFileSync(resolve(configDir, "../../lib/Cargo.toml"), "utf8");
const versionMatch = cargoToml.match(/^\[package\][\s\S]*?^\s*version\s*=\s*"([^"]+)"/m);
if (!versionMatch) {
  console.warn("Unable to find package version in lib/Cargo.toml");
}
const latestVersion = versionMatch?.[1] ?? "0.0.0";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "--usage",
  description:
    "A spec and reference frameworks for building CLIs — define commands, flags, and args once in KDL; get argument parsing, shell completions, docs, and manpages everywhere",
  appearance: "force-dark",
  lastUpdated: true,
  cleanUrls: true,
  markdown: {
    shikiSetup: async (shiki) => {
      await shiki.loadLanguage(kdlGrammar as any);
    }
  },
  sitemap: {
    hostname: "https://usage.jdx.dev"
  },
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    logo: "/icon.svg",
    nav: [
      { text: "Home", link: "/" },
      { text: "Spec", link: "/spec/" },
      {
        text: "Frameworks",
        items: [
          { text: "Rust", link: "/rust/" },
          { text: "Go", link: "/go/" }
        ]
      },
      { text: "CLI", link: "/cli/" },
      { text: `v${latestVersion}`, link: "https://github.com/jdx/usage/releases" }
    ],

    sidebar: [
      { text: "Contributing", link: "/contributing" },
      {
        text: "Rust Framework",
        link: "/rust/",
        items: [
          { text: "Args and Flags", link: "/rust/args-and-flags" },
          { text: "Subcommands", link: "/rust/subcommands" },
          { text: "Dispatch", link: "/rust/dispatch" },
          { text: "Migrating from clap", link: "/rust/migrating-from-clap" },
          { text: "clap Compatibility", link: "/rust/clap-compatibility" },
          { text: "Validation", link: "/rust/validation" },
          { text: "Settings", link: "/rust/settings" },
          { text: "Help and Errors", link: "/rust/help" },
          { text: "Performance", link: "/rust/performance" },
          { text: "Completions", link: "/rust/completions" },
          { text: "Testing", link: "/rust/testing" },
          { text: "Spec Output", link: "/rust/spec" }
        ]
      },
      {
        text: "Go Framework",
        link: "/go/",
        items: [
          { text: "Generated Code", link: "/go/generated-code" },
          { text: "The Parser", link: "/go/parser" },
          { text: "Binding and Values", link: "/go/binding" },
          { text: "Help and Errors", link: "/go/help" },
          { text: "Completions", link: "/go/completions" }
        ]
      },
      {
        text: "CLI",
        link: "/cli/",
        items: [
          { text: "Completions", link: "/cli/completions" },
          { text: "Manpages", link: "/cli/manpages" },
          { text: "Markdown", link: "/cli/markdown" },
          { text: "SDK Generation", link: "/cli/sdk" },
          { text: "Scripts", link: "/cli/scripts" },
          {
            text: "CLI Reference", link: "/cli/reference/", items:
              commands.map((command) => ({
                text: command.join(" "),
                link: `/cli/reference/${command.join("/")}`
              }))
          }
        ]
      },
      {
        text: "Spec",
        link: "/spec/",
        items: [
          { text: "argv grammar", link: "/spec/argv" },
          { text: "config resolution", link: "/spec/resolution" },
          {
            text: "Reference",
            link: "/spec/reference/",
            items: [
              { text: "arg", link: "/spec/reference/arg" },
              { text: "cmd", link: "/spec/reference/cmd" },
              { text: "complete", link: "/spec/reference/complete" },
              { text: "flag", link: "/spec/reference/flag" },
              { text: "flagset", link: "/spec/reference/flagset" },
              { text: "group", link: "/spec/reference/group" },
              // { text: 'env', link: '/spec/reference/env' },
              { text: "config", link: "/spec/reference/config" }
            ]
          },
          {
            text: "Integrations",
            link: "/spec/integrations",
            collapsed: true,
            items: [
              { text: "Cobra (Go)", link: "/spec/integrations/cobra" },
              { text: "Kong (Go)", link: "https://github.com/gaojunran/usage-integrations/tree/main/packages/kong-usage" },
              { text: "urfave/cli (Go)", link: "https://github.com/gaojunran/usage-integrations/tree/main/packages/urfavecli-usage" },
              { text: "clap (Rust)", link: "/spec/integrations/clap" },
              { text: "argparse (Python)", link: "https://github.com/acidghost/argparse-usage" },
              { text: "OptionParser (Ruby)", link: "https://github.com/packrat386/option_parser_usage" },
              { text: "Commander.js (Node.js)", link: "https://www.npmjs.com/package/@usage-spec/commander" },
              { text: "oclif (Node.js)", link: "https://www.npmjs.com/package/@usage-spec/oclif" },
              { text: "yargs (Node.js)", link: "https://www.npmjs.com/package/@usage-spec/yargs" },
              { text: "Typer (Python)", link: "https://pypi.org/project/usage-spec-typer/" },
              { text: "Click (Python)", link: "https://pypi.org/project/usage-spec-click/" },
              { text: "JCommander (Java)", link: "https://github.com/gaojunran/usage-integrations/packages/3045397" },
              { text: "picocli (Java)", link: "https://github.com/gaojunran/usage-integrations/packages/3045398" },
              { text: "Clikt (Kotlin)", link: "https://github.com/gaojunran/usage-integrations/packages/3045396" },
            ]
          }
        ]
      }
    ],

    socialLinks: [{ icon: "github", link: "https://github.com/jdx/usage" }],
    editLink: {
      pattern: "https://github.com/jdx/usage/edit/main/docs/:path"
    },
    // carbonAds: {
    //   code: 'CWYIPKQN',
    //   placement: 'misejdxdev',
    // },
    search: {
      provider: "local"
    },
    footer: false,
  },
  head: [
    [
      "script",
      {},
      `(function () {
  try {
    var d = document.documentElement;
    var c = JSON.parse(localStorage.getItem("jdx-banner-cache") || "null");
    var expires = c && c.expires ? Date.parse(c.expires) : NaN;
    var now = Date.now();
    var metadataValid =
      c &&
      typeof c.id === "string" &&
      typeof c.height === "string" &&
      /^[1-9]\\d*(?:\\.\\d+)?px$/.test(c.height) &&
      Number.isFinite(c.width) &&
      typeof c.fontSize === "string" &&
      Number.isFinite(c.pixelRatio) &&
      Number.isFinite(c.cachedAt) &&
      c.cachedAt <= now &&
      now - c.cachedAt < 300000 &&
      (!c.expires || (typeof c.expires === "string" && Number.isFinite(expires) && now < expires));
    var contextMatches =
      metadataValid &&
      c.width === innerWidth &&
      c.fontSize === getComputedStyle(d).fontSize &&
      c.pixelRatio === devicePixelRatio;
    if (contextMatches && localStorage.getItem("jdx-banner-dismissed") !== c.id)
      d.style.setProperty("--vp-layout-top-height", c.height);
    else if (c && !metadataValid)
      localStorage.removeItem("jdx-banner-cache");
  } catch (e) {}
})();`,
    ],
    ["link", { rel: "icon", type: "image/svg+xml", href: "/icon.svg" }],
    ["link", { rel: "icon", type: "image/png", sizes: "32x32", href: "/favicon-32x32.png" }],
    ["link", { rel: "icon", type: "image/png", sizes: "16x16", href: "/favicon-16x16.png" }],
    ["link", { rel: "apple-touch-icon", sizes: "180x180", href: "/apple-touch-icon.png" }],
    ["link", { rel: "manifest", href: "/site.webmanifest" }],
    ["meta", { name: "theme-color", content: "#0d0221" }],
    // OpenGraph
    ["meta", { property: "og:site_name", content: "--usage" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:image", content: "https://usage.jdx.dev/android-chrome-512x512.png" }],
    ["meta", { name: "twitter:card", content: "summary" }],
    ["meta", { name: "twitter:image", content: "https://usage.jdx.dev/android-chrome-512x512.png" }]
  ]
});
