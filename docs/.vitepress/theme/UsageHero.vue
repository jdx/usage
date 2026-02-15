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
        <h1 class="usage-hero-title">--usage</h1>

        <!-- Tagline -->
        <p class="usage-hero-tagline">A specification for CLIs</p>

        <!-- Feature Buttons -->
        <div class="usage-hero-features">
          <a href="/cli/completions" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🐚</span>
            <span class="usage-hero-feature-text">Completions</span>
            <div class="usage-hero-feature-tooltip">
              <strong>Shell Completions</strong>
              <p>Generate tab completions for bash, zsh, fish, PowerShell, and nushell from a single spec.</p>
            </div>
          </a>
          <a href="/cli/markdown" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">📖</span>
            <span class="usage-hero-feature-text">Documentation</span>
            <div class="usage-hero-feature-tooltip">
              <strong>Auto-Generated Docs</strong>
              <p>Create --help output, markdown docs, and manpages that stay in sync automatically.</p>
            </div>
          </a>
          <a href="/cli/scripts" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">📜</span>
            <span class="usage-hero-feature-text">Scripts</span>
            <div class="usage-hero-feature-tooltip">
              <strong>Bash Scripts</strong>
              <p>Write bash scripts with modern arg parsing, validation, and completions built-in.</p>
            </div>
          </a>
          <a href="/spec/" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🎯</span>
            <span class="usage-hero-feature-text">KDL Spec</span>
            <div class="usage-hero-feature-tooltip">
              <strong>KDL Specification</strong>
              <p>Define your CLI once in human-readable KDL format. One spec powers everything.</p>
            </div>
          </a>
          <a href="/cli/completions" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🖥️</span>
            <span class="usage-hero-feature-text">5+ Shells</span>
            <div class="usage-hero-feature-tooltip">
              <strong>Multi-Shell Support</strong>
              <p>Works with bash, zsh, fish, PowerShell, and nushell out of the box.</p>
            </div>
          </a>
          <a href="/spec/integrations" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🌐</span>
            <span class="usage-hero-feature-text">Integrations</span>
            <div class="usage-hero-feature-tooltip">
              <strong>Framework Integrations</strong>
              <p>Generate usage specs from CLI frameworks like clap, with more integrations coming soon.</p>
            </div>
          </a>
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
