<script setup lang="ts">
// Measured numbers, not marketing: Rust figures from PLAN.md's gated perf
// report; Go figures from go/README.md. Wall time is parser overhead — what
// the framework adds on top of a process that parses nothing. The Go values
// subtract the ~0.95ms a do-nothing Go process costs, so they are
// deliberately approximate.
const rustRows = [
  { name: "usage-rs", value: 2.1, label: "2.1µs", note: "~230× less", us: true },
  { name: "clap", value: 490, label: "490µs", us: false },
];

const goRows = [
  { name: "usage-go", value: 0.15, label: "~0.15ms", note: "~6× less", us: true },
  { name: "urfave/cli v3", value: 0.75, label: "~0.75ms", us: false },
  { name: "cobra", value: 0.85, label: "~0.85ms", us: false },
  { name: "kong", value: 5.2, label: "~5.2ms", us: false },
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
    <h2 class="usage-bench-title">Measured, not claimed.</h2>
    <p class="usage-bench-sub">
      Parser overhead — wall time the framework adds on top of a process that parses
      nothing — for <code>mise use -g node@20</code> against a shadow of
      <a href="https://mise.jdx.dev">mise</a>'s CLI: 211 commands, 711 flags, cold.
    </p>

    <div class="usage-bench-cards">
      <div class="usage-bench-card">
        <h3>usage-rs <span>vs clap</span></h3>
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
          Most of clap's 490µs is building and validating its command tree before it
          can parse. Instructions: <strong>50.9k</strong> vs 5.96M. Heap allocations
          for a bare parse: <strong>zero</strong> vs 6,560.
        </p>
      </div>

      <div class="usage-bench-card">
        <h3>usage-go <span>vs cobra, urfave/cli, kong</span></h3>
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
          A do-nothing Go process costs ~0.95ms of runtime startup no parser can
          touch; these subtract it. Instructions: <strong>~2.7k</strong> vs cobra's
          2.0M, urfave/cli's 5.6M, kong's 57.9M.
        </p>
      </div>
    </div>

    <p class="usage-bench-method">
      Methodology and raw numbers:
      <a href="https://github.com/jdx/usage/blob/main/go/README.md">go/README.md</a> ·
      <a href="https://github.com/jdx/usage/blob/main/tasks/perf-shadow.sh">tasks/perf-shadow.sh</a>
    </p>
  </section>
</template>
