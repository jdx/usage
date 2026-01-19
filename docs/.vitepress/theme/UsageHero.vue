<script setup lang="ts">
</script>

<template>
  <section class="usage-hero">
    <!-- Animated Background -->
    <div class="usage-hero-bg">
      <div class="usage-hero-orb usage-hero-orb-1"></div>
      <div class="usage-hero-orb usage-hero-orb-2"></div>
      <div class="usage-hero-orb usage-hero-orb-3"></div>
    </div>

    <!-- Hero Content -->
    <div class="usage-hero-content">
      <!-- Left Column: Text -->
      <div class="usage-hero-text">
        <!-- Title -->
        <h1 class="usage-hero-title">Usage</h1>

        <!-- Tagline -->
        <p class="usage-hero-tagline">A specification for CLIs</p>

        <!-- Stat Cards -->
        <div class="usage-hero-stats">
          <div class="usage-hero-stat">
            <div class="usage-hero-stat-value">5+</div>
            <div class="usage-hero-stat-label">Shells</div>
            <div class="usage-hero-stat-tooltip">
              <strong>Supported Shells</strong>
              <ul>
                <li>bash</li>
                <li>zsh</li>
                <li>fish</li>
                <li>PowerShell</li>
                <li>nushell</li>
              </ul>
            </div>
          </div>
          <div class="usage-hero-stat">
            <div class="usage-hero-stat-value">KDL</div>
            <div class="usage-hero-stat-label">Powered</div>
            <div class="usage-hero-stat-tooltip">
              <strong>KDL Specification</strong>
              <p>Define your CLI in human-readable KDL format. One spec generates completions, docs, and parsing for any shell.</p>
            </div>
          </div>
          <div class="usage-hero-stat">
            <div class="usage-hero-stat-value">Any</div>
            <div class="usage-hero-stat-label">Language</div>
            <div class="usage-hero-stat-tooltip">
              <strong>Language Agnostic</strong>
              <p>Use with bash scripts, or integrate with CLIs written in any language via the usage CLI tool.</p>
            </div>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="usage-hero-actions">
          <a href="/spec/" class="usage-hero-btn usage-hero-btn-primary">
            Get Started
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M5 12h14"/>
              <path d="m12 5 7 7-7 7"/>
            </svg>
          </a>
          <a href="/cli/" class="usage-hero-btn usage-hero-btn-secondary">
            CLI Reference
          </a>
        </div>

        <!-- Feature Buttons -->
        <div class="usage-hero-features">
          <a href="/cli/completions" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🐚</span>
            <span class="usage-hero-feature-text">Shell Completions</span>
          </a>
          <a href="/cli/markdown" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">📖</span>
            <span class="usage-hero-feature-text">Documentation</span>
          </a>
          <a href="/cli/scripts" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">📜</span>
            <span class="usage-hero-feature-text">Scripts</span>
          </a>
          <a href="/spec/" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🎯</span>
            <span class="usage-hero-feature-text">KDL Specification</span>
          </a>
        </div>
      </div>

      <!-- Right Column: Logo -->
      <div class="usage-hero-logo-wrapper">
        <img
          src="/logo-nav.png"
          alt="Usage logo"
          class="usage-hero-logo"
        />
      </div>
    </div>
  </section>
</template>
