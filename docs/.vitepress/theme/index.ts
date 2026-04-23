import DefaultTheme from 'vitepress/theme'
import { h } from 'vue'
import UsageHero from './UsageHero.vue'
import EndevFooter from './EndevFooter.vue'
import { initBanner } from './banner'
import './custom.css'

export default {
  extends: DefaultTheme,
  Layout() {
    return h(DefaultTheme.Layout, null, {
      'home-hero-before': () => h(UsageHero),
      'layout-bottom': () => h(EndevFooter)
    })
  },
  enhanceApp() {
    initBanner()
  }
}
