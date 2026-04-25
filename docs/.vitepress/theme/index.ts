import DefaultTheme from 'vitepress/theme'
import { h, onMounted } from 'vue'
import UsageHero from './UsageHero.vue'
import EndevFooter from './EndevFooter.vue'
import { initBanner } from './banner'
import { data as starsData } from '../stars.data'
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
  },
  setup() {
    onMounted(() => {
      const addStarCount = () => {
        const githubLink = document.querySelector(
          '.VPSocialLinks a[href*="github.com/jdx/usage"]',
        )
        if (githubLink && !githubLink.querySelector('.star-count')) {
          const starBadge = document.createElement('span')
          starBadge.className = 'star-count'
          starBadge.textContent = starsData.stars
          starBadge.title = 'GitHub Stars'
          githubLink.appendChild(starBadge)
        }
      }

      addStarCount()
      setTimeout(addStarCount, 100)
      const observer = new MutationObserver(addStarCount)
      observer.observe(document.body, { childList: true, subtree: true })
    })
  }
}
