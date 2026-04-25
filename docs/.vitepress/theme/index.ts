import DefaultTheme from 'vitepress/theme'
import { h, onMounted, onUnmounted } from 'vue'
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
    let observer: MutationObserver | undefined
    onMounted(() => {
      const addStarCount = () => {
        if (!starsData.stars) return false

        const githubLinks = document.querySelectorAll(
          '.VPSocialLinks a[href*="github.com/jdx/usage"]',
        )
        githubLinks.forEach((githubLink) => {
          if (!githubLink.querySelector('.star-count')) {
            const starBadge = document.createElement('span')
            starBadge.className = 'star-count'
            starBadge.textContent = starsData.stars
            starBadge.title = 'GitHub Stars'
            githubLink.appendChild(starBadge)
          }
        })
        return githubLinks.length > 0 && Array.from(githubLinks).every((link) => link.querySelector('.star-count'))
      }

      if (addStarCount()) return

      observer = new MutationObserver(() => {
        if (addStarCount()) observer?.disconnect()
      })
      observer.observe(document.querySelector('.VPNav') || document.body, { childList: true, subtree: true })
    })
    onUnmounted(() => observer?.disconnect())
  }
}
