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
      <!-- Logo -->
      <img
        src="/logo-nav.png"
        alt="Usage logo"
        class="usage-hero-logo"
      />

      <!-- Title -->
      <h1 class="usage-hero-title">Usage</h1>

      <!-- Tagline -->
      <p class="usage-hero-tagline">A specification for CLIs</p>

      <!-- Stat Cards -->
      <div class="usage-hero-stats">
        <div class="usage-hero-stat" title="bash, zsh, fish, PowerShell, nushell">
          <div class="usage-hero-stat-value">5+</div>
          <div class="usage-hero-stat-label">Shells</div>
        </div>
        <div class="usage-hero-stat" title="Human-readable CLI specification format">
          <div class="usage-hero-stat-value">KDL</div>
          <div class="usage-hero-stat-label">Powered</div>
        </div>
        <div class="usage-hero-stat" title="Use with bash scripts or any CLI tool">
          <div class="usage-hero-stat-value">Any</div>
          <div class="usage-hero-stat-label">Language</div>
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
    </div>
  </section>
</template>
