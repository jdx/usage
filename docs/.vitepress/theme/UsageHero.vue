<script setup lang="ts">
// Brand glyph paths from simple-icons (24x24, fill with currentColor)
const rustPath =
  "M23.8346 11.7033l-1.0073-.6236a13.7268 13.7268 0 00-.0283-.2936l.8656-.8069a.3483.3483 0 00-.1154-.578l-1.1066-.414a8.4958 8.4958 0 00-.087-.2856l.6904-.9587a.3462.3462 0 00-.2257-.5446l-1.1663-.1894a9.3574 9.3574 0 00-.1407-.2622l.49-1.0761a.3437.3437 0 00-.0274-.3361.3486.3486 0 00-.3006-.154l-1.1845.0416a6.7444 6.7444 0 00-.1873-.2268l.2723-1.153a.3472.3472 0 00-.417-.4172l-1.1532.2724a14.0183 14.0183 0 00-.2278-.1873l.0415-1.1845a.3442.3442 0 00-.49-.328l-1.076.491c-.0872-.0476-.1742-.0952-.2623-.1407l-.1903-1.1673A.3483.3483 0 0016.256.955l-.9597.6905a8.4867 8.4867 0 00-.2855-.086l-.414-1.1066a.3483.3483 0 00-.5781-.1154l-.8069.8666a9.2936 9.2936 0 00-.2936-.0284L12.2946.1683a.3462.3462 0 00-.5892 0l-.6236 1.0073a13.7383 13.7383 0 00-.2936.0284L9.9803.3374a.3462.3462 0 00-.578.1154l-.4141 1.1065c-.0962.0274-.1903.0567-.2855.086L7.744.955a.3483.3483 0 00-.5447.2258L7.009 2.348a9.3574 9.3574 0 00-.2622.1407l-1.0762-.491a.3462.3462 0 00-.49.328l.0416 1.1845a7.9826 7.9826 0 00-.2278.1873L3.8413 3.425a.3472.3472 0 00-.4171.4171l.2713 1.1531c-.0628.075-.1255.1509-.1863.2268l-1.1845-.0415a.3462.3462 0 00-.328.49l.491 1.0761a9.167 9.167 0 00-.1407.2622l-1.1662.1894a.3483.3483 0 00-.2258.5446l.6904.9587a13.303 13.303 0 00-.087.2855l-1.1065.414a.3483.3483 0 00-.1155.5781l.8656.807a9.2936 9.2936 0 00-.0283.2935l-1.0073.6236a.3442.3442 0 000 .5892l1.0073.6236c.008.0982.0182.1964.0283.2936l-.8656.8079a.3462.3462 0 00.1155.578l1.1065.4141c.0273.0962.0567.1914.087.2855l-.6904.9587a.3452.3452 0 00.2268.5447l1.1662.1893c.0456.088.0922.1751.1408.2622l-.491 1.0762a.3462.3462 0 00.328.49l1.1834-.0415c.0618.0769.1235.1528.1873.2277l-.2713 1.1541a.3462.3462 0 00.4171.4161l1.153-.2713c.075.0638.151.1255.2279.1863l-.0415 1.1845a.3442.3442 0 00.49.327l1.0761-.49c.087.0486.1741.0951.2622.1407l.1903 1.1662a.3483.3483 0 00.5447.2268l.9587-.6904a9.299 9.299 0 00.2855.087l.414 1.1066a.3452.3452 0 00.5781.1154l.8079-.8656c.0972.0111.1954.0203.2936.0294l.6236 1.0073a.3472.3472 0 00.5892 0l.6236-1.0073c.0982-.0091.1964-.0183.2936-.0294l.8069.8656a.3483.3483 0 00.578-.1154l.4141-1.1066a8.4626 8.4626 0 00.2855-.087l.9587.6904a.3452.3452 0 00.5447-.2268l.1903-1.1662c.088-.0456.1751-.0931.2622-.1407l1.0762.49a.3472.3472 0 00.49-.327l-.0415-1.1845a6.7267 6.7267 0 00.2267-.1863l1.1531.2713a.3472.3472 0 00.4171-.416l-.2713-1.1542c.0628-.0749.1255-.1508.1863-.2278l1.1845.0415a.3442.3442 0 00.328-.49l-.49-1.076c.0475-.0872.0951-.1742.1407-.2623l1.1662-.1893a.3483.3483 0 00.2258-.5447l-.6904-.9587.087-.2855 1.1066-.414a.3462.3462 0 00.1154-.5781l-.8656-.8079c.0101-.0972.0202-.1954.0283-.2936l1.0073-.6236a.3442.3442 0 000-.5892zm-6.7413 8.3551a.7138.7138 0 01.2986-1.396.714.714 0 11-.2997 1.396zm-.3422-2.3142a.649.649 0 00-.7715.5l-.3573 1.6685c-1.1035.501-2.3285.7795-3.6193.7795a8.7368 8.7368 0 01-3.6951-.814l-.3574-1.6684a.648.648 0 00-.7714-.499l-1.473.3158a8.7216 8.7216 0 01-.7613-.898h7.1676c.081 0 .1356-.0141.1356-.088v-2.536c0-.074-.0536-.0881-.1356-.0881h-2.0966v-1.6077h2.2677c.2065 0 1.1065.0587 1.394 1.2088.0901.3533.2875 1.5044.4232 1.8729.1346.413.6833 1.2381 1.2685 1.2381h3.5716a.7492.7492 0 00.1296-.0131 8.7874 8.7874 0 01-.8119.9526zM6.8369 20.024a.714.714 0 11-.2997-1.396.714.714 0 01.2997 1.396zM4.1177 8.9972a.7137.7137 0 11-1.304.5791.7137.7137 0 011.304-.579zm-.8352 1.9813l1.5347-.6824a.65.65 0 00.33-.8585l-.3158-.7147h1.2432v5.6025H3.5669a8.7753 8.7753 0 01-.2834-3.348zm6.7343-.5437V8.7836h2.9601c.153 0 1.0792.1772 1.0792.8697 0 .575-.7107.7815-1.2948.7815zm10.7574 1.4862c0 .2187-.008.4363-.0243.651h-.9c-.09 0-.1265.0586-.1265.1477v.413c0 .973-.5487 1.1846-1.0296 1.2382-.4576.0517-.9648-.1913-1.0275-.4717-.2704-1.5186-.7198-1.8436-1.4305-2.4034.8817-.5599 1.799-1.386 1.799-2.4915 0-1.1936-.819-1.9458-1.3769-2.3153-.7825-.5163-1.6491-.6195-1.883-.6195H5.4682a8.7651 8.7651 0 014.907-2.7699l1.0974 1.151a.648.648 0 00.9182.0213l1.227-1.1743a8.7753 8.7753 0 016.0044 4.2762l-.8403 1.8982a.652.652 0 00.33.8585l1.6178.7188c.0283.2875.0425.577.0425.8717zm-9.3006-9.5993a.7128.7128 0 11.984 1.0316.7137.7137 0 01-.984-1.0316zm8.3389 6.71a.7107.7107 0 01.9395-.3625.7137.7137 0 11-.9405.3635z";
const goPath =
  "M1.811 10.231c-.047 0-.058-.023-.035-.059l.246-.315c.023-.035.081-.058.128-.058h4.172c.046 0 .058.035.035.07l-.199.303c-.023.036-.082.07-.117.07zM.047 11.306c-.047 0-.059-.023-.035-.058l.245-.316c.023-.035.082-.058.129-.058h5.328c.047 0 .07.035.058.07l-.093.28c-.012.047-.058.07-.105.07zm2.828 1.075c-.047 0-.059-.035-.035-.07l.163-.292c.023-.035.07-.07.117-.07h2.337c.047 0 .07.035.07.082l-.023.28c0 .047-.047.082-.082.082zm12.129-2.36c-.736.187-1.239.327-1.963.514-.176.046-.187.058-.34-.117-.174-.199-.303-.327-.548-.444-.737-.362-1.45-.257-2.115.175-.795.514-1.204 1.274-1.192 2.22.011.935.654 1.706 1.577 1.835.795.105 1.46-.175 1.987-.77.105-.13.198-.27.315-.434H10.47c-.245 0-.304-.152-.222-.35.152-.362.432-.97.596-1.274a.315.315 0 01.292-.187h4.253c-.023.316-.023.631-.07.947a4.983 4.983 0 01-.958 2.29c-.841 1.11-1.94 1.8-3.33 1.986-1.145.152-2.209-.07-3.143-.77-.865-.655-1.356-1.52-1.484-2.595-.152-1.274.222-2.419.993-3.424.83-1.086 1.928-1.776 3.272-2.02 1.098-.2 2.15-.07 3.096.571.62.41 1.063.97 1.356 1.648.07.105.023.164-.117.2m3.868 6.461c-1.064-.024-2.034-.328-2.852-1.029a3.665 3.665 0 01-1.262-2.255c-.21-1.32.152-2.489.947-3.529.853-1.122 1.881-1.706 3.272-1.95 1.192-.21 2.314-.095 3.33.595.923.63 1.496 1.484 1.648 2.605.198 1.578-.257 2.863-1.344 3.962-.771.783-1.718 1.273-2.805 1.495-.315.06-.63.07-.934.106zm2.78-4.72c-.011-.153-.011-.27-.034-.387-.21-1.157-1.274-1.81-2.384-1.554-1.087.245-1.788.935-2.045 2.033-.21.912.234 1.835 1.075 2.21.643.28 1.285.244 1.905-.07.923-.48 1.425-1.228 1.484-2.233z";
const pythonPath =
  "M14.25.18l.9.2.73.26.59.3.45.32.34.34.25.34.16.33.1.3.04.26.02.2-.01.13V8.5l-.05.63-.13.55-.21.46-.26.38-.3.31-.33.25-.35.19-.35.14-.33.1-.3.07-.26.04-.21.02H8.77l-.69.05-.59.14-.5.22-.41.27-.33.32-.27.35-.2.36-.15.37-.1.35-.07.32-.04.27-.02.21v3.06H3.17l-.21-.03-.28-.07-.32-.12-.35-.18-.36-.26-.36-.36-.35-.46-.32-.59-.28-.73-.21-.88-.14-1.05-.05-1.23.06-1.22.16-1.04.24-.87.32-.71.36-.57.4-.44.42-.33.42-.24.4-.16.36-.1.32-.05.24-.01h.16l.06.01h8.16v-.83H6.18l-.01-2.75-.02-.37.05-.34.11-.31.17-.28.25-.26.31-.23.38-.2.44-.18.51-.15.58-.12.64-.1.71-.06.77-.04.84-.02 1.27.05zm-6.3 1.98l-.23.33-.08.41.08.41.23.34.33.22.41.09.41-.09.33-.22.23-.34.08-.41-.08-.41-.23-.33-.33-.22-.41-.09-.41.09zm13.09 3.95l.28.06.32.12.35.18.36.27.36.35.35.47.32.59.28.73.21.88.14 1.04.05 1.23-.06 1.23-.16 1.04-.24.86-.32.71-.36.57-.4.45-.42.33-.42.24-.4.16-.36.09-.32.05-.24.02-.16-.01h-8.22v.82h5.84l.01 2.76.02.36-.05.34-.11.31-.17.29-.25.25-.31.24-.38.2-.44.17-.51.15-.58.13-.64.09-.71.07-.77.04-.84.01-1.27-.04-1.07-.14-.9-.2-.73-.25-.59-.3-.45-.33-.34-.34-.25-.34-.16-.33-.1-.3-.04-.25-.02-.2.01-.13v-5.34l.05-.64.13-.54.21-.46.26-.38.3-.32.33-.24.35-.2.35-.14.33-.1.3-.06.26-.04.21-.02.13-.01h5.84l.69-.05.59-.14.5-.21.41-.28.33-.32.27-.35.2-.36.15-.36.1-.35.07-.32.04-.28.02-.21V6.07h2.09l.14.01zm-6.47 14.25l-.23.33-.08.41.08.41.23.33.33.23.41.08.41-.08.33-.23.23-.33.08-.41-.08-.41-.23-.33-.33-.23-.41-.08-.41.08z";
const javascriptPath =
  "M0 0h24v24H0V0zm22.034 18.276c-.175-1.095-.888-2.015-3.003-2.873-.736-.345-1.554-.585-1.797-1.14-.091-.33-.105-.51-.046-.705.15-.646.915-.84 1.515-.66.39.12.75.42.976.9 1.034-.676 1.034-.676 1.755-1.125-.27-.42-.404-.601-.586-.78-.63-.705-1.469-1.065-2.834-1.034l-.705.089c-.676.165-1.32.525-1.71 1.005-1.14 1.291-.811 3.541.569 4.471 1.365 1.02 3.361 1.244 3.616 2.205.24 1.17-.87 1.545-1.966 1.41-.811-.18-1.26-.586-1.755-1.336l-1.83 1.051c.21.48.45.689.81 1.109 1.74 1.756 6.09 1.666 6.871-1.004.029-.09.24-.705.074-1.65l.046.067zm-8.983-7.245h-2.248c0 1.938-.009 3.864-.009 5.805 0 1.232.063 2.363-.138 2.711-.33.689-1.18.601-1.566.48-.396-.196-.597-.466-.83-.855-.063-.105-.11-.196-.127-.196l-1.825 1.125c.305.63.75 1.172 1.324 1.517.855.51 2.004.675 3.207.405.783-.226 1.458-.691 1.811-1.411.51-.93.402-2.07.397-3.346.012-2.054 0-4.109 0-6.179l.004-.056z";
</script>

<template>
  <section class="usage-hero">
    <!-- Hero Content -->
    <div class="usage-hero-content">
      <!-- Left Column: Text -->
      <div class="usage-hero-text">
        <h1 class="usage-hero-title"><span class="accent">--</span>usage</h1>

        <p class="usage-hero-tagline">Define your CLI once.<span class="cursor"></span></p>

        <p class="usage-hero-desc">
          Usage is a toolkit for building command-line tools. Define your CLI's commands,
          flags, and args once in a KDL spec — get argument parsing, shell completions,
          <code>--help</code>, docs, and manpages from that one definition. Reference
          frameworks for Rust and Go build your CLI from the spec, with Python and
          JavaScript planned.
        </p>

        <p class="usage-hero-label">Frameworks</p>
        <div class="usage-hero-tiles">
          <a
            href="https://github.com/jdx/usage/tree/main/derive"
            class="usage-tile usage-tile-live"
          >
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path :d="rustPath" />
            </svg>
            <span>Rust</span>
            <div class="usage-tile-tooltip">
              <strong>Rust framework</strong>
              <p>Derive your CLI from Rust types — parsing, help, and completions generated from the usage spec.</p>
            </div>
          </a>
          <a
            href="https://github.com/jdx/usage/tree/main/go"
            class="usage-tile usage-tile-live"
          >
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path :d="goPath" />
            </svg>
            <span>Go</span>
            <div class="usage-tile-tooltip">
              <strong>Go framework</strong>
              <p>Build Go CLIs on the usage spec, with parsing behavior verified against the same conformance corpus as the Rust implementation.</p>
            </div>
          </a>
          <span class="usage-tile usage-tile-soon">
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path :d="pythonPath" />
            </svg>
            <span>Python</span>
            <span class="usage-tile-soon-pill">soon</span>
          </span>
          <span class="usage-tile usage-tile-soon">
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path :d="javascriptPath" />
            </svg>
            <span>JavaScript</span>
            <span class="usage-tile-soon-pill">soon</span>
          </span>
        </div>

        <p class="usage-hero-label">Spec &amp; tooling</p>
        <div class="usage-hero-tiles">
          <a href="/cli/completions" class="usage-tile usage-tile-feature">
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <polyline points="4 17 10 11 4 5" />
              <line x1="12" y1="19" x2="20" y2="19" />
            </svg>
            <span>Completions</span>
            <div class="usage-tile-tooltip">
              <strong>Shell completions</strong>
              <p>Tab completions for bash, zsh, fish, PowerShell, and nushell, generated from the spec.</p>
            </div>
          </a>
          <a href="/cli/markdown" class="usage-tile usage-tile-feature">
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
              <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
            </svg>
            <span>Docs &amp; manpages</span>
            <div class="usage-tile-tooltip">
              <strong>Auto-generated docs</strong>
              <p>--help output, markdown docs, and manpages, all generated from the same spec.</p>
            </div>
          </a>
          <a href="/cli/scripts" class="usage-tile usage-tile-feature">
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1="8" y1="13" x2="16" y2="13" />
              <line x1="8" y1="17" x2="13" y2="17" />
            </svg>
            <span>Scripts</span>
            <div class="usage-tile-tooltip">
              <strong>Bash scripts</strong>
              <p>Declare a script's interface in a USAGE comment and get arg parsing, validation, and completions.</p>
            </div>
          </a>
          <a href="/spec/" class="usage-tile usage-tile-feature">
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <polyline points="16 18 22 12 16 6" />
              <polyline points="8 6 2 12 8 18" />
            </svg>
            <span>KDL spec</span>
            <div class="usage-tile-tooltip">
              <strong>KDL specification</strong>
              <p>Define your CLI once in KDL. Completions, docs, and parsers are all generated from it.</p>
            </div>
          </a>
          <a href="/cli/" class="usage-tile usage-tile-feature">
            <svg class="usage-tile-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
            </svg>
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
            <linearGradient id="usage-scene-horizon" x1="0" y1="0" x2="360" y2="0" gradientUnits="userSpaceOnUse">
              <stop offset="0" stop-color="#ff2d95" stop-opacity="0" />
              <stop offset="0.3" stop-color="#ff2d95" stop-opacity="0.8" />
              <stop offset="0.7" stop-color="#ff2d95" stop-opacity="0.8" />
              <stop offset="1" stop-color="#ff2d95" stop-opacity="0" />
            </linearGradient>
            <linearGradient id="usage-grid-fade" x1="0" y1="0" x2="360" y2="0" gradientUnits="userSpaceOnUse">
              <stop offset="0" stop-color="#000" />
              <stop offset="0.25" stop-color="#fff" />
              <stop offset="0.75" stop-color="#fff" />
              <stop offset="1" stop-color="#000" />
            </linearGradient>
            <mask id="usage-grid-mask">
              <rect x="0" y="248" width="360" height="82" fill="url(#usage-grid-fade)" />
            </mask>
          </defs>

          <!-- ghost echoes -->
          <path d="M146 68 L348 68 L312 140 L348 212 L146 212 Z" fill="#01cdfe" opacity="0.55" />
          <path d="M131 56 L333 56 L297 128 L333 200 L131 200 Z" fill="#b967ff" opacity="0.55" />

          <!-- flag with dash cutouts -->
          <path d="M116 44 L318 44 L282 116 L318 188 L116 188 Z" fill="#ff2d95" />
          <rect x="148" y="105" width="50" height="22" rx="11" fill="#0d0221" />
          <rect x="216" y="105" width="50" height="22" rx="11" fill="#0d0221" />

          <!-- pole -->
          <line x1="108" y1="34" x2="108" y2="252" stroke="#f5f0ff" stroke-width="9" stroke-linecap="round" />
          <circle cx="108" cy="28" r="7" fill="#f5f0ff" />

          <!-- grid floor, fading at the scene edges -->
          <line x1="0" y1="252" x2="360" y2="252" stroke="url(#usage-scene-horizon)" stroke-width="2.5" />
          <g mask="url(#usage-grid-mask)" stroke="#01cdfe" stroke-width="1" opacity="0.45">
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
          <pre><code><span class="k">name</span> <span class="s">"mycli"</span>
<span class="k">flag</span> <span class="s">"-v --verbose"</span> <span class="p">help=</span><span class="s">"Enable verbose output"</span>
<span class="k">arg</span> <span class="s">"&lt;file&gt;"</span> <span class="p">help=</span><span class="s">"File to process"</span>
<span class="k">cmd</span> <span class="s">"deploy"</span> { <span class="k">flag</span> <span class="s">"--env &lt;env&gt;"</span> }</code></pre>
        </div>
      </div>
    </div>
  </section>
</template>
