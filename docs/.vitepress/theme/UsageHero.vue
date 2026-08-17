<script setup lang="ts">
</script>

<template>
  <section class="usage-hero">
    <!-- Grid floor -->
    <svg
      class="usage-hero-grid"
      viewBox="0 0 1200 260"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <line x1="0" y1="1" x2="1200" y2="1" stroke="#ff2d95" stroke-width="2" opacity="0.7" />
      <g stroke="#b967ff" stroke-width="1" opacity="0.3">
        <line x1="600" y1="0" x2="-500" y2="260" />
        <line x1="600" y1="0" x2="-150" y2="260" />
        <line x1="600" y1="0" x2="150" y2="260" />
        <line x1="600" y1="0" x2="380" y2="260" />
        <line x1="600" y1="0" x2="600" y2="260" />
        <line x1="600" y1="0" x2="820" y2="260" />
        <line x1="600" y1="0" x2="1050" y2="260" />
        <line x1="600" y1="0" x2="1350" y2="260" />
        <line x1="600" y1="0" x2="1700" y2="260" />
        <line x1="0" y1="30" x2="1200" y2="30" />
        <line x1="0" y1="70" x2="1200" y2="70" />
        <line x1="0" y1="125" x2="1200" y2="125" />
        <line x1="0" y1="195" x2="1200" y2="195" />
      </g>
    </svg>

    <!-- Hero Content -->
    <div class="usage-hero-content">
      <!-- Left Column: Text -->
      <div class="usage-hero-text">
        <h1 class="usage-hero-title"><span class="accent">--</span>usage</h1>

        <p class="usage-hero-tagline">One spec. Every shell.<span class="cursor"></span></p>

        <p class="usage-hero-label">Frameworks</p>
        <div class="usage-hero-tiles">
          <a
            href="https://github.com/jdx/usage/tree/main/derive"
            class="usage-tile usage-tile-live"
          >
            <span>🦀</span>
            <span>Rust</span>
            <div class="usage-tile-tooltip">
              <strong>Rust framework</strong>
              <p>Derive your CLI from Rust types — parsing, help, and completions powered by the usage spec.</p>
            </div>
          </a>
          <a
            href="https://github.com/jdx/usage/tree/main/go"
            class="usage-tile usage-tile-live"
          >
            <span>🐹</span>
            <span>Go</span>
            <div class="usage-tile-tooltip">
              <strong>Go framework</strong>
              <p>Build Go CLIs on the usage spec with the same parsing and completion behavior everywhere.</p>
            </div>
          </a>
          <span class="usage-tile usage-tile-soon">
            <span>🐍</span>
            <span>Python</span>
            <span class="usage-tile-soon-pill">soon</span>
          </span>
          <span class="usage-tile usage-tile-soon">
            <span>✨</span>
            <span>JavaScript</span>
            <span class="usage-tile-soon-pill">soon</span>
          </span>
        </div>

        <p class="usage-hero-label">Powered by the spec</p>
        <div class="usage-hero-tiles">
          <a href="/cli/completions" class="usage-tile usage-tile-feature">
            <span>🐚</span>
            <span>Completions</span>
            <div class="usage-tile-tooltip">
              <strong>Shell completions</strong>
              <p>Tab completions for bash, zsh, fish, PowerShell, and nushell — every shell, one spec.</p>
            </div>
          </a>
          <a href="/cli/markdown" class="usage-tile usage-tile-feature">
            <span>📖</span>
            <span>Docs &amp; manpages</span>
            <div class="usage-tile-tooltip">
              <strong>Auto-generated docs</strong>
              <p>--help output, markdown docs, and manpages that stay in sync automatically.</p>
            </div>
          </a>
          <a href="/cli/scripts" class="usage-tile usage-tile-feature">
            <span>📜</span>
            <span>Scripts</span>
            <div class="usage-tile-tooltip">
              <strong>Bash scripts</strong>
              <p>Write bash scripts with modern arg parsing, validation, and completions built in.</p>
            </div>
          </a>
          <a href="/spec/" class="usage-tile usage-tile-feature">
            <span>🎯</span>
            <span>KDL spec</span>
            <div class="usage-tile-tooltip">
              <strong>KDL specification</strong>
              <p>Define your CLI once in human-readable KDL. One spec powers everything.</p>
            </div>
          </a>
          <a href="/cli/" class="usage-tile usage-tile-feature">
            <span>🛠️</span>
            <span>usage-cli</span>
            <div class="usage-tile-tooltip">
              <strong>usage-cli utility</strong>
              <p>The standalone tool for generating completions, docs, and parsing from any spec file.</p>
            </div>
          </a>
        </div>

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

      <!-- Right Column: planted flag illustration -->
      <div class="usage-hero-art-wrapper">
        <div class="usage-hero-glow"></div>
        <svg
          class="usage-hero-art"
          viewBox="0 0 360 320"
          role="img"
          aria-label="A flag striped like a vaporwave sunset, planted on a neon grid horizon"
        >
          <defs>
            <linearGradient id="usage-sunset" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stop-color="#ff2d95" />
              <stop offset="0.6" stop-color="#ff8a5c" />
              <stop offset="1" stop-color="#ffd319" />
            </linearGradient>
            <clipPath id="usage-flag-clip">
              <path d="M120 42 L318 42 L282 114 L318 186 L120 186 Z" />
            </clipPath>
          </defs>

          <!-- ghost echoes -->
          <path d="M138 66 L336 66 L300 138 L336 210 L138 210 Z" fill="#01cdfe" opacity="0.35" />
          <path d="M129 54 L327 54 L291 126 L327 198 L129 198 Z" fill="#b967ff" opacity="0.35" />

          <!-- sunset flag with dash stripes -->
          <g clip-path="url(#usage-flag-clip)">
            <rect x="120" y="42" width="198" height="144" fill="url(#usage-sunset)" />
            <rect x="120" y="100" width="198" height="9" fill="#0d0221" />
            <rect x="120" y="121" width="198" height="12" fill="#0d0221" />
            <rect x="120" y="147" width="198" height="16" fill="#0d0221" />
          </g>
          <path d="M120 42 L318 42 L282 114 L318 186 L120 186 Z" fill="none" stroke="#0d0221" stroke-width="3" />

          <!-- pole planted at the horizon -->
          <line x1="108" y1="34" x2="108" y2="252" stroke="#f5f0ff" stroke-width="9" stroke-linecap="round" />
          <circle cx="108" cy="28" r="7" fill="#f5f0ff" />

          <!-- local grid under the flag -->
          <line x1="0" y1="252" x2="360" y2="252" stroke="#ff2d95" stroke-width="2" opacity="0.9" />
          <g stroke="#01cdfe" stroke-width="1" opacity="0.5">
            <line x1="180" y1="252" x2="-80" y2="320" />
            <line x1="180" y1="252" x2="40" y2="320" />
            <line x1="180" y1="252" x2="140" y2="320" />
            <line x1="180" y1="252" x2="220" y2="320" />
            <line x1="180" y1="252" x2="320" y2="320" />
            <line x1="180" y1="252" x2="440" y2="320" />
            <line x1="0" y1="268" x2="360" y2="268" />
            <line x1="0" y1="288" x2="360" y2="288" />
            <line x1="0" y1="313" x2="360" y2="313" />
          </g>
        </svg>
      </div>
    </div>
  </section>
</template>
