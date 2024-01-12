import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Usage",
  description: "Schema for CLIs",
  lastUpdated: true,
  markdown: {
    // languages: [
    //   "kdl"
    // ]
  },
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Examples', link: '/markdown-examples' }
    ],

    sidebar: [
      {
        text: 'Examples',
        items: [
          { text: 'Markdown Examples', link: '/markdown-examples' },
          { text: 'Runtime API Examples', link: '/api-examples' }
        ]
      }
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/jdx/usage' }
    ],
    editLink: {
      pattern: 'https://github.com/jdx/usage/edit/main/:path',
    },
    // carbonAds: {
    //   code: 'CWYIPKQN',
    //   placement: 'misejdxdev',
    // },
    search: {
      provider: 'local'
    },
    footer: {
      message: 'Licensed under the MIT License. Maintained by <a href="https://github.com/jdx">@jdx</a> and <a href="https://github.com/jdx/usage/graphs/contributors">friends</a>.',
      copyright: 'Copyright © 2024 <a href="https://github.com/jdx">@jdx</a>',
    },
  }
})
