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
          <a href="/cli/completions" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🖥️</span>
            <span class="usage-hero-feature-text">5+ Shells</span>
          </a>
          <a href="/spec/integrations" class="usage-hero-feature">
            <span class="usage-hero-feature-icon">🌐</span>
            <span class="usage-hero-feature-text">Any Language</span>
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
