<template>
  <section class="usage-hero" aria-labelledby="usage-title">
    <div class="usage-hero-content">
      <div class="usage-hero-text">
        <p class="usage-hero-label">The toolkit for command-line interfaces</p>
        <h1 id="usage-title" class="usage-hero-title">
          <span class="accent">--</span>usage
        </h1>
        <p class="usage-hero-tagline">Define your CLI once.</p>
        <p class="usage-hero-desc">
          Commands, flags, arguments, and settings in one definition. Build a
          typed Rust CLI, or bring a KDL spec to generate completions, help,
          documentation, and client libraries.
        </p>
        <div class="usage-hero-actions">
          <a
            href="/rust/quickstart"
            class="usage-hero-btn usage-hero-btn-primary"
          >
            Build a Rust CLI <span aria-hidden="true">→</span>
          </a>
          <a
            href="/guide/getting-started"
            class="usage-hero-btn usage-hero-btn-secondary"
          >
            Start with a spec
          </a>
        </div>
        <p class="usage-hero-support">
          Already have a CLI?
          <a href="/spec/integrations">Connect your framework →</a>
        </p>
        <p class="usage-hero-shells">
          Bash · Zsh · Fish · PowerShell · Nushell
        </p>
      </div>
      <!-- Right Column: flag mark illustration + spec sample -->
      <div class="usage-hero-art-wrapper">
        <div class="usage-hero-glow"></div>
        <svg
          class="usage-hero-art"
          viewBox="0 0 360 330"
          role="img"
          aria-label="A magenta flag bearing two dashes with cyan and purple echoes, planted on a neon grid floor"
        >
          <defs>
            <linearGradient
              id="usage-scene-horizon"
              x1="0"
              y1="0"
              x2="360"
              y2="0"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0" stop-color="#ff2d95" stop-opacity="0" />
              <stop offset="0.3" stop-color="#ff2d95" stop-opacity="0.8" />
              <stop offset="0.7" stop-color="#ff2d95" stop-opacity="0.8" />
              <stop offset="1" stop-color="#ff2d95" stop-opacity="0" />
            </linearGradient>
            <linearGradient
              id="usage-grid-fade"
              x1="0"
              y1="0"
              x2="360"
              y2="0"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0" stop-color="#000" />
              <stop offset="0.25" stop-color="#fff" />
              <stop offset="0.75" stop-color="#fff" />
              <stop offset="1" stop-color="#000" />
            </linearGradient>
            <mask id="usage-grid-mask">
              <rect
                x="0"
                y="248"
                width="360"
                height="82"
                fill="url(#usage-grid-fade)"
              />
            </mask>
          </defs>

          <!-- ghost echoes -->
          <path
            d="M146 68 L348 68 L312 140 L348 212 L146 212 Z"
            fill="#01cdfe"
            opacity="0.55"
          />
          <path
            d="M131 56 L333 56 L297 128 L333 200 L131 200 Z"
            fill="#b967ff"
            opacity="0.55"
          />

          <!-- flag with dash cutouts -->
          <path
            d="M116 44 L318 44 L282 116 L318 188 L116 188 Z"
            fill="#ff2d95"
          />
          <rect x="148" y="105" width="50" height="22" rx="11" fill="#0d0221" />
          <rect x="216" y="105" width="50" height="22" rx="11" fill="#0d0221" />

          <!-- pole -->
          <line
            x1="108"
            y1="34"
            x2="108"
            y2="252"
            stroke="#f5f0ff"
            stroke-width="9"
            stroke-linecap="round"
          />
          <circle cx="108" cy="28" r="7" fill="#f5f0ff" />

          <!-- grid floor, fading at the scene edges -->
          <line
            x1="0"
            y1="252"
            x2="360"
            y2="252"
            stroke="url(#usage-scene-horizon)"
            stroke-width="2.5"
          />
          <g
            mask="url(#usage-grid-mask)"
            stroke="#01cdfe"
            stroke-width="1"
            opacity="0.45"
          >
            <line x1="180" y1="252" x2="-120" y2="330" />
            <line x1="180" y1="252" x2="20" y2="330" />
            <line x1="180" y1="252" x2="110" y2="330" />
            <line x1="180" y1="252" x2="180" y2="330" />
            <line x1="180" y1="252" x2="250" y2="330" />
            <line x1="180" y1="252" x2="340" y2="330" />
            <line x1="180" y1="252" x2="480" y2="330" />
            <line x1="0" y1="262" x2="360" y2="262" />
            <line x1="0" y1="276" x2="360" y2="276" />
            <line x1="0" y1="295" x2="360" y2="295" />
            <line x1="0" y1="319" x2="360" y2="319" />
          </g>
        </svg>

        <div class="usage-spec-card" aria-label="Example usage spec">
          <div class="usage-spec-card-bar">
            <span></span><span></span><span></span>
            <em>mycli.usage.kdl</em>
          </div>
          <pre><code><span class="k">bin</span> <span class="s">"mycli"</span>
<span class="k">cmd</span> <span class="s">"deploy"</span> <span class="p">help=</span><span class="s">"Deploy a service"</span> {
    <span class="k">arg</span> <span class="s">"&lt;service&gt;"</span>
    <span class="k">flag</span> <span class="s">"--env &lt;env&gt;"</span> {
        <span class="k">choices</span> <span class="s">"staging"</span> <span class="s">"production"</span>
    }
}</code></pre>
        </div>
      </div>
    </div>
  </section>
</template>
