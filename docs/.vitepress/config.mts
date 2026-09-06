import { socialCard, writeSocialCard } from "./social-images.mjs";
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
const cargoToml = readFileSync(
  resolve(configDir, "../../lib/Cargo.toml"),
  "utf8"
);
const versionMatch = cargoToml.match(
  /^\[package\][\s\S]*?^\s*version\s*=\s*"([^"]+)"/m
);
if (!versionMatch) {
  console.warn("Unable to find package version in lib/Cargo.toml");
}
const latestVersion = versionMatch?.[1] ?? "0.0.0";
const siteUrl = "https://usage.jdx.dev";
const siteDescription =
  "Define CLI commands, flags, and arguments once in KDL, then generate parsers, shell completions, documentation, and man pages across languages.";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "--usage",
  description: siteDescription,
  appearance: "force-dark",
  lastUpdated: true,
  cleanUrls: true,
  markdown: {
    shikiSetup: async (shiki) => {
      await shiki.loadLanguage(kdlGrammar as any);
    },
  },
  sitemap: {
    hostname: siteUrl,
  },
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    logo: "/icon.svg",
    nav: [
      { text: "Get started", link: "/guide/getting-started" },
      {
        text: "Frameworks",
        items: [
          { text: "Rust", link: "/rust/" },
          { text: "Go (development preview)", link: "/go/" },
          {
            text: "Existing framework integrations",
            link: "/spec/integrations",
          },
        ],
      },
      { text: "Spec", link: "/spec/" },
      { text: "CLI", link: "/cli/" },
      {
        text: `v${latestVersion}`,
        link: "https://github.com/jdx/usage/releases",
      },
    ],

    sidebar: {
      "/rust/": [
        {
          text: "Rust framework",
          link: "/rust/",
          items: [
            { text: "Quickstart", link: "/rust/quickstart" },
            { text: "Arguments and flags", link: "/rust/args-and-flags" },
            { text: "Subcommands", link: "/rust/subcommands" },
            { text: "Dispatch", link: "/rust/dispatch" },
            { text: "Help, version, and errors", link: "/rust/help" },
            { text: "Completions", link: "/rust/completions" },
            { text: "Configuration", link: "/rust/configuration" },
            { text: "Validation", link: "/rust/validation" },
            { text: "Testing", link: "/rust/testing" },
          ],
        },
        {
          text: "Advanced topics",
          items: [
            { text: "Dynamic commands", link: "/rust/dynamic-commands" },
            { text: "Updating values", link: "/rust/update-from" },
            { text: "Response files", link: "/rust/response-files" },
            { text: "Spec output", link: "/rust/spec" },
            { text: "Migrating from clap", link: "/rust/migrating-from-clap" },
            { text: "Performance", link: "/rust/performance" },
          ],
        },
        {
          text: "More documentation",
          items: [
            { text: "Spec reference", link: "/spec/reference/" },
            { text: "Usage CLI", link: "/cli/" },
            { text: "Contributing", link: "/contributing" },
          ],
        },
      ],
      "/go/": [
        {
          text: "Go development preview",
          link: "/go/",
          items: [
            { text: "Generated code", link: "/go/generated-code" },
            { text: "Parser", link: "/go/parser" },
            { text: "Binding and values", link: "/go/binding" },
            { text: "Help and errors", link: "/go/help" },
            { text: "Completions", link: "/go/completions" },
          ],
        },
        {
          text: "Related",
          items: [
            { text: "Rust framework", link: "/rust/" },
            { text: "Cobra integration", link: "/spec/integrations/cobra" },
            { text: "Spec reference", link: "/spec/reference/" },
            { text: "Contributing", link: "/contributing" },
          ],
        },
      ],
      "/": [
        {
          text: "Start here",
          items: [
            { text: "Get started", link: "/guide/getting-started" },
            { text: "Install Usage", link: "/cli/#installation" },
            { text: "Spec basics", link: "/spec/" },
            { text: "Framework integrations", link: "/spec/integrations" },
          ],
        },
        {
          text: "Guides",
          items: [
            { text: "Shell completions", link: "/cli/completions" },
            { text: "Scripts", link: "/cli/scripts" },
            { text: "Markdown documentation", link: "/cli/markdown" },
            { text: "Man pages", link: "/cli/manpages" },
            { text: "TypeScript and Python SDKs", link: "/cli/sdk" },
            { text: "Compare specs", link: "/cli/diff" },
          ],
        },
        {
          text: "Spec reference",
          link: "/spec/reference/",
          collapsed: false,
          items: [
            { text: "Arguments · arg", link: "/spec/reference/arg" },
            { text: "Flags · flag", link: "/spec/reference/flag" },
            { text: "Commands · cmd", link: "/spec/reference/cmd" },
            {
              text: "Completions · complete",
              link: "/spec/reference/complete",
            },
            { text: "Configuration · config", link: "/spec/reference/config" },
            { text: "Shared flags · flagset", link: "/spec/reference/flagset" },
            { text: "Argument groups · group", link: "/spec/reference/group" },
            { text: "Outputs and exit codes", link: "/spec/reference/output" },
            { text: "Sigil arguments", link: "/spec/reference/sigils" },
            { text: "Clauses", link: "/spec/reference/clause" },
            { text: "Argument grammar", link: "/spec/argv" },
            { text: "Configuration resolution", link: "/spec/resolution" },
          ],
        },
        {
          text: "CLI reference",
          link: "/cli/reference/",
          collapsed: true,
          items: commands.map((command) => ({
            text: command.join(" "),
            link: `/cli/reference/${command.join("/")}`,
          })),
        },
        {
          text: "Frameworks and project",
          items: [
            { text: "Rust framework", link: "/rust/" },
            { text: "Go development preview", link: "/go/" },
            { text: "clap integration", link: "/spec/integrations/clap" },
            { text: "Cobra integration", link: "/spec/integrations/cobra" },
            { text: "Contributing", link: "/contributing" },
          ],
        },
      ],
    },

    socialLinks: [{ icon: "github", link: "https://github.com/jdx/usage" }],
    editLink: {
      pattern: "https://github.com/jdx/usage/edit/main/docs/:path",
    },
    search: {
      provider: "local",
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
    [
      "link",
      {
        rel: "icon",
        type: "image/png",
        sizes: "32x32",
        href: "/favicon-32x32.png",
      },
    ],
    [
      "link",
      {
        rel: "icon",
        type: "image/png",
        sizes: "16x16",
        href: "/favicon-16x16.png",
      },
    ],
    [
      "link",
      {
        rel: "apple-touch-icon",
        sizes: "180x180",
        href: "/apple-touch-icon.png",
      },
    ],
    ["link", { rel: "manifest", href: "/site.webmanifest" }],
    ["meta", { name: "theme-color", content: "#0d0221" }],
    // OpenGraph
    ["meta", { property: "og:site_name", content: "--usage" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:locale", content: "en_US" }],
    ["meta", { property: "og:image:width", content: "1200" }],
    ["meta", { property: "og:image:height", content: "630" }],
    ["meta", { name: "twitter:card", content: "summary_large_image" }],
    ["meta", { name: "twitter:site", content: "@jdxcode" }],
  ],
  transformHead({ pageData, title, description, siteConfig }) {
    const heading =
      pageData.relativePath === "index.md"
        ? "CLI specs, parsers, and completions"
        : pageData.title || "usage";
    const card = socialCard(heading);
    writeSocialCard(siteConfig.outDir, card);
    const image = new URL(card.path, `${siteUrl}/`).toString();
    const imageAlt = `${heading} — usage docs`;
    const url = new URL(
      pageData.relativePath.replace(/index\.md$/, "").replace(/\.md$/, ""),
      `${siteUrl}/`
    ).toString();

    return [
      ["link", { rel: "canonical", href: url }],
      ["meta", { property: "og:url", content: url }],
      ["meta", { property: "og:image", content: image }],
      ["meta", { property: "og:image:alt", content: imageAlt }],
      ["meta", { name: "twitter:image", content: image }],
      ["meta", { name: "twitter:image:alt", content: imageAlt }],
      ["meta", { property: "og:title", content: title }],
      ["meta", { property: "og:description", content: description }],
      ["meta", { name: "twitter:title", content: title }],
      ["meta", { name: "twitter:description", content: description }],
      [
        "script",
        { type: "application/ld+json" },
        JSON.stringify({
          "@context": "https://schema.org",
          "@type": "WebPage",
          name: title,
          description,
          url,
          isPartOf: { "@type": "WebSite", name: "usage", url: siteUrl },
        }),
      ],
    ];
  },
});
