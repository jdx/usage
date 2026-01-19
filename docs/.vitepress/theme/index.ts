import DefaultTheme from 'vitepress/theme'
import { h } from 'vue'
import UsageHero from './UsageHero.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  Layout() {
    return h(DefaultTheme.Layout, null, {
      'home-hero-before': () => h(UsageHero)
    })
  }
}
