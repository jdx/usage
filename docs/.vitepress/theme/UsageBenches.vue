<script setup lang="ts">
// Rust wall clock from `time-sweep.rs`, which warms each parser and keeps the
// fastest of many short rounds — steady-state and in-process, because a
// whole-process measurement cannot resolve a 200ns parse. Values in nanoseconds,
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
// ~0.95ms a do-nothing Go process costs, so they are deliberately approximate.
//
// Ratios read the same way as the Rust card's: what each framework costs against
// usage-go, on the row that costs it. Whole multiples, because every figure here is
// a difference between two numbers already rounded to two significant figures — a
// ratio of those does not deserve a decimal place.
const goRows = [
  { name: "usage-go", value: 0.15, label: "~0.15ms", us: true },
  { name: "urfave/cli v3", value: 0.75, label: "~0.75ms", note: "~5× more", us: false },
  { name: "cobra", value: 0.85, label: "~0.85ms", note: "~6× more", us: false },
  { name: "kong", value: 5.2, label: "~5.2ms", note: "~35× more", us: false },
];

const rustMax = Math.max(...rustRows.map((r) => r.value));
const goMax = Math.max(...goRows.map((r) => r.value));

function width(value: number, max: number): string {
  return `${Math.max((value / max) * 100, 0.5)}%`;
}
</script>

<template>
  <section class="usage-bench">
    <p class="usage-hero-label">Benchmarks</p>
    <h2 class="usage-bench-title">Parser overhead</h2>
    <p class="usage-bench-sub">
      What parsing <code>mise use -g node@20</code> costs each framework, against a shadow
      of <a href="https://mise.jdx.dev">mise</a>'s CLI: 211 commands,
      711 flags. Every shadow is generated from the same spec by the same traversal, so
      what differs is the parser rather than one of them being written more carefully.
    </p>

    <div class="usage-bench-cards">
      <div class="usage-bench-card">
        <h3>usage-rs <span>vs clap, argh, bpaf</span></h3>
        <p class="usage-bench-metric">
          wall time,
          <span class="usage-bench-hint" tabindex="0" aria-describedby="bench-tip-warmed"
            >one warmed parse
            <span class="usage-bench-tip" id="bench-tip-warmed" role="tooltip">
              <strong>How this is measured</strong>
              <span>
                In-process and warmed — the fastest of many short rounds, since a fresh
                process cannot resolve a 200ns parse. Minima and their ratios drift a few
                percent between runs and machines, hence the <code>~</code>. Instructions
                for one cold parse, which do not drift: 4,155 · 6,295 · 5.89M · 21.9M,
                agreeing across two machines to 0.15%. For scale, starting a process costs
                ~1ms, so the first two bars are under anything a user feels.
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
          <span class="usage-bench-hint" tabindex="0" aria-describedby="bench-tip-build"
            >build a parser before they can use one
            <span class="usage-bench-tip" id="bench-tip-build" role="tooltip">
              <strong>Where their time goes</strong>
              <span>
                Most of clap's is constructing and validating its command tree. bpaf's is
                larger because it assembles a combinator tree per run as well, which
                reusing the parser across parses only halves.
              </span>
            </span></span
          >. Heap allocations for a bare parse: <strong>zero</strong>, against clap's 6,280.
          argh and bpaf also
          <span class="usage-bench-hint" tabindex="0" aria-describedby="bench-tip-express"
            >express less
            <span class="usage-bench-tip" id="bench-tip-express" role="tooltip">
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

      <div class="usage-bench-card">
        <h3>usage-go <span>vs cobra, urfave/cli, kong</span></h3>
        <p class="usage-bench-metric">
          wall time,
          <span class="usage-bench-hint" tabindex="0" aria-describedby="bench-tip-cold"
            >one cold parse
            <span class="usage-bench-tip" id="bench-tip-cold" role="tooltip">
              <strong>How this is measured</strong>
              <span>
                Whole-process, with the ~0.95ms of Go runtime startup a do-nothing process
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
      <a href="https://github.com/jdx/usage/blob/main/go/README.md">go/README.md</a> ·
      <a href="https://github.com/jdx/usage/blob/main/tasks/perf-shadow.sh">tasks/perf-shadow.sh</a>
      ·
      <a
        href="https://github.com/jdx/usage/blob/main/benches/gate/src/bin/time-sweep.rs"
        >time-sweep.rs</a
      >
    </p>
  </section>
</template>
