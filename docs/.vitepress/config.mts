import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Usage",
  description: "Schema for CLIs",
  lastUpdated: true,
  cleanUrls: true,
  markdown: {
    // languages: [
    //   "kdl"
    // ]
  },
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Spec', link: '/spec/' },
      { text: 'CLI', link: '/cli/' },
    ],

    sidebar: [
      {
        text: 'Spec',
        link: '/spec/',
        items: [
          {
            text: 'Reference',
            link: '/spec/reference/',
            items: [
              { text: 'cmd', link: '/spec/reference/cmd' },
              { text: 'arg', link: '/spec/reference/arg' },
              { text: 'flag', link: '/spec/reference/flag' },
              { text: 'complete', link: '/spec/reference/complete' },
              // { text: 'env', link: '/spec/reference/env' },
              // { text: 'config', link: '/spec/reference/config' },
            ]
          },
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
