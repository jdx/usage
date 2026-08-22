<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";

const props = withDefaults(
  defineProps<{
    // Homepage shows both cards. Framework pages pass the language they are
    // documenting so the other card does not sit in the way of that page's
    // own install instructions.
    lang?: "all" | "rust" | "go";
    // Doc-column layout: the homepage padding and 1120px cap do not belong
    // inside a VitePress article.
    embedded?: boolean;
  }>(),
  { lang: "all", embedded: false },
);

const showRust = computed(() => props.lang !== "go");
const showGo = computed(() => props.lang !== "rust");
const idPrefix = computed(() => `bench-${props.lang}`);

// Rust wall clock from `time-sweep.rs`, which runs each parser repeatedly in one
// process and keeps the fastest of many short rounds — throughput rather than
// whole-process latency, because process startup cannot resolve a 200ns parse.
// Values in nanoseconds,
// quoted to two significant figures: across twenty-two runs on two machines the
// minima moved a few percent and their ratios by 10% — clap read 2,226x, 2,502x
// and 2,419x on three occasions — so the ratios carry a `~`.
//
// The instruction counts in the tooltip are the firm measure, from the `perf-pr`
// workflow's pinned runner running `tasks/perf-shadow.sh`: deterministic for a
// given binary, agreeing to within 0.15% between a developer machine and the
// runner, and genuinely one *cold* parse — `parse-n` is differenced against the
// same binary parsing nothing.
const rustRows = [
  { name: "usage-rs", value: 194, label: "190 ns", us: true },
  { name: "argh", value: 268, label: "270 ns", note: "1.4× more", us: false },
  { name: "clap", value: 479605, label: "480 µs", note: "~2,500× more", us: false },
  { name: "bpaf", value: 1597028, label: "1.6 ms", note: "~8,200× more", us: false },
];

// Go figures from go/README.md are whole-process wall time and subtract the
// ~950 µs a do-nothing Go process costs, so they are deliberately approximate.
//
// Ratios read the same way as the Rust card's: what each framework costs against
// usage-go, on the row that costs it. Whole multiples, because every figure here is
// a difference between two numbers already rounded to two significant figures — a
// ratio of those does not deserve a decimal place.
const goRows = [
  { name: "usage-go", value: 0.15, label: "~150 µs", us: true },
  { name: "urfave/cli v3", value: 0.75, label: "~750 µs", note: "~5× more", us: false },
  { name: "cobra", value: 0.85, label: "~850 µs", note: "~6× more", us: false },
  { name: "kong", value: 5.2, label: "~5.2 ms", note: "~35× more", us: false },
];

const rustMax = Math.max(...rustRows.map((r) => r.value));
const goMax = Math.max(...goRows.map((r) => r.value));

function width(value: number, max: number): string {
  return `${Math.max((value / max) * 100, 0.5)}%`;
}

// A tip is centred on the phrase it explains, and where that phrase sits on the line
// depends on where the text wrapped — so on a narrow card a tip can hang off the
// edge, and which edge it hangs off is not knowable when the CSS is written. Nudge
// it back inside as it opens. Measuring here rather than guessing in a media query
// is what lets the tip stay under its own phrase at every width.
const GUTTER = 8;

function place(hint: HTMLElement) {
  const tip = hint.querySelector(".usage-bench-tip") as HTMLElement | null;
  const card = hint.closest(".usage-bench-card") as HTMLElement | null;
  if (!tip || !card) return;

  const h = hint.getBoundingClientRect();
  const c = card.getBoundingClientRect();

  // Inside the card, and inside the window: at 320px the card is itself wider than
  // the screen, so the card alone is not the bound that matters there.
  const min = Math.max(c.left, 0) + GUTTER;
  const max = Math.min(c.right, window.innerWidth) - GUTTER;
  const room = max - min;

  // A tip too wide for that interval cannot be nudged into it — moving it to clear
  // one edge only pushes it past the other — so cap the width to the room there is
  // and let it wrap. This is why the width lives here and not only in the CSS, where
  // it would have to guess the same interval a second time.
  tip.style.maxWidth = "";
  if (tip.offsetWidth > room) tip.style.maxWidth = `${Math.floor(room)}px`;

  // Where the tip sits with no nudge: centred on the phrase, per the CSS. Derived
  // from layout rather than read off the tip's own rect, which reports wherever the
  // last nudge is mid-transition to and would clamp against the wrong position.
  const left = h.left + h.width / 2 - tip.offsetWidth / 2;
  const right = left + tip.offsetWidth;

  let shift = 0;
  if (left < min) shift = min - left;
  else if (right > max) shift = max - right;
  tip.style.setProperty("--tip-shift", `${Math.round(shift)}px`);
}

function clamp(event: Event) {
  place(event.currentTarget as HTMLElement);
}

// A tip open across a resize, a zoom, a rotation or the 860px reflow was measured
// against a layout that no longer exists. Whichever hint is still under the pointer
// or holding focus is the one showing a tip, so that is the one to measure again.
function replace() {
  const open = document.querySelector(
    ".usage-bench-hint:hover, .usage-bench-hint:focus",
  ) as HTMLElement | null;
  if (open) place(open);
}

onMounted(() => window.addEventListener("resize", replace, { passive: true }));
onBeforeUnmount(() => window.removeEventListener("resize", replace));
</script>

<template>
  <section class="usage-bench" :class="{ embedded: embedded }">
    <template v-if="!embedded">
      <p class="usage-hero-label">Benchmarks</p>
      <h2 class="usage-bench-title">Parser overhead</h2>
    </template>
    <p class="usage-bench-sub">
      What parsing <code>mise use -g node@20</code> costs each framework, against a shadow
      of <a href="https://mise.jdx.dev">mise</a>'s CLI: 211 commands, 711 flags.
    </p>

    <div
      class="usage-bench-cards"
      :class="{ 'usage-bench-cards-single': !(showRust && showGo) }"
    >
      <div v-if="showRust" class="usage-bench-card">
        <h3>usage-rs <span>vs clap, argh, bpaf</span></h3>
        <p class="usage-bench-metric">
          wall time,
          <span
            class="usage-bench-hint"
            tabindex="0"
            @mouseenter="clamp"
            @focusin="clamp"
            :aria-describedby="`${idPrefix}-tip-warmed`"
            >in-process parse throughput
            <span class="usage-bench-tip" :id="`${idPrefix}-tip-warmed`" role="tooltip">
              <strong>How this is measured</strong>
              <span>
                Each parser runs repeatedly inside one process; process startup is excluded,
                so these bars are not full CLI invocation times. The chart reports the fastest
                per-parse time from many short rounds. Minima and their ratios drift a few
                percent between runs and machines, hence the <code>~</code>. Instructions for
                one cold parse, which do not drift: 4,155 · 6,295 · 5.89M · 21.9M,
                agreeing across two machines to 0.15%.
              </span>
            </span>
          </span>
        </p>
        <div class="usage-bench-rows">
          <div v-for="row in rustRows" :key="row.name" class="usage-bench-row">
            <span class="usage-bench-name">{{ row.name }}</span>
            <span class="usage-bench-track">
              <span class="usage-bench-bararea">
                <span
                  class="usage-bench-bar"
                  :class="{ us: row.us }"
                  :style="{ width: width(row.value, rustMax) }"
                ></span>
              </span>
              <span class="usage-bench-value"
                >{{ row.label }}<em v-if="row.note"> · {{ row.note }}</em></span
              >
            </span>
          </div>
        </div>
        <p class="usage-bench-foot">
          clap and bpaf
          <span
            class="usage-bench-hint"
            tabindex="0"
            @mouseenter="clamp"
            @focusin="clamp"
            :aria-describedby="`${idPrefix}-tip-build`"
            >build a parser before they can use one
            <span class="usage-bench-tip" :id="`${idPrefix}-tip-build`" role="tooltip">
              <strong>Where their time goes</strong>
              <span>
                Most of clap's is constructing and validating its command tree. bpaf's is
                larger because it assembles a combinator tree per run as well, which
                reusing the parser across parses only halves.
              </span>
            </span></span
          >. Heap allocations for a bare parse: <strong>zero</strong>, against clap's 6,280.
          argh and bpaf also
          <span
            class="usage-bench-hint"
            tabindex="0"
            @mouseenter="clamp"
            @focusin="clamp"
            :aria-describedby="`${idPrefix}-tip-express`"
            >express less
            <span class="usage-bench-tip" :id="`${idPrefix}-tip-express`" role="tooltip">
              <strong>Missing from the argh and bpaf shadows</strong>
              <span>
                Aliases, hidden commands, global flags, and a positional beside a
                subcommand — the generator drops them, counts them, and prints the count
                when it runs.
              </span>
            </span></span
          >.
        </p>
      </div>

      <div v-if="showGo" class="usage-bench-card">
        <h3>usage-go <span>vs cobra, urfave/cli, kong</span></h3>
        <p class="usage-bench-metric">
          wall time,
          <span
            class="usage-bench-hint"
            tabindex="0"
            @mouseenter="clamp"
            @focusin="clamp"
            :aria-describedby="`${idPrefix}-tip-cold`"
            >one cold parse
            <span class="usage-bench-tip" :id="`${idPrefix}-tip-cold`" role="tooltip">
              <strong>How this is measured</strong>
              <span>
                Whole-process, with the ~950 µs of Go runtime startup a do-nothing process
                costs subtracted — approximate, and the reason the Rust card is timed
                in-process instead.
              </span>
            </span>
          </span>
        </p>
        <div class="usage-bench-rows">
          <div v-for="row in goRows" :key="row.name" class="usage-bench-row">
            <span class="usage-bench-name">{{ row.name }}</span>
            <span class="usage-bench-track">
              <span class="usage-bench-bararea">
                <span
                  class="usage-bench-bar"
                  :class="{ us: row.us }"
                  :style="{ width: width(row.value, goMax) }"
                ></span>
              </span>
              <span class="usage-bench-value"
                >{{ row.label }}<em v-if="row.note"> · {{ row.note }}</em></span
              >
            </span>
          </div>
        </div>
        <p class="usage-bench-foot">
          Instructions for the same parse: <strong>~2.7k</strong> vs cobra's 2.0M,
          urfave/cli's 5.6M, kong's 57.9M.
        </p>
      </div>
    </div>

    <p class="usage-bench-method">
      Methodology and raw numbers:
      <template v-if="showGo">
        <a href="https://github.com/jdx/usage/blob/main/go/README.md">go/README.md</a>
        <template v-if="showRust"> · </template>
      </template>
      <template v-if="showRust">
        <a href="https://github.com/jdx/usage/blob/main/tasks/perf-shadow.sh">tasks/perf-shadow.sh</a>
        ·
        <a
          href="https://github.com/jdx/usage/blob/main/benches/gate/src/bin/time-sweep.rs"
          >time-sweep.rs</a
        >
      </template>
    </p>
  </section>
</template>
