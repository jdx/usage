<script setup lang="ts">
// Measured numbers, not marketing: Rust figures from PLAN.md's gated perf
// report; Go figures from go/README.md. Both are cachegrind instruction
// counts for a cold parse against a shadow of mise's spec.
const rustRows = [
  { name: "usage", value: 50_900, label: "50.9k", note: "117× fewer", us: true },
  { name: "clap", value: 5_960_000, label: "5.96M", us: false },
];

const goRows = [
  { name: "usage-go", value: 2_700, label: "~2.7k", note: "~740× fewer", us: true },
  { name: "cobra", value: 2_008_880, label: "2.0M", us: false },
  { name: "urfave/cli v3", value: 5_591_321, label: "5.6M", us: false },
  { name: "kong", value: 57_889_084, label: "57.9M", us: false },
];

const rustMax = Math.max(...rustRows.map((r) => r.value));
const goMax = Math.max(...goRows.map((r) => r.value));

function width(value: number, max: number): string {
  return `${Math.max((value / max) * 100, 0.4)}%`;
}
</script>

<template>
  <section class="usage-bench">
    <p class="usage-hero-label">Benchmarks</p>
    <h2 class="usage-bench-title">Measured, not claimed.</h2>
    <p class="usage-bench-sub">
      Instructions to parse <code>mise use -g node@20</code> against a shadow of
      <a href="https://mise.jdx.dev">mise</a>'s CLI — 211 commands, 711 flags.
      Cachegrind, cold process, same method in both languages.
    </p>

    <div class="usage-bench-cards">
      <div class="usage-bench-card">
        <h3>Rust <span>vs clap</span></h3>
        <div class="usage-bench-rows">
          <div v-for="row in rustRows" :key="row.name" class="usage-bench-row">
            <span class="usage-bench-name">{{ row.name }}</span>
            <span class="usage-bench-track">
              <span
                class="usage-bench-bar"
                :class="{ us: row.us }"
                :style="{ width: width(row.value, rustMax) }"
              ></span>
              <span class="usage-bench-value"
                >{{ row.label }}<em v-if="row.note"> · {{ row.note }}</em></span
              >
            </span>
          </div>
        </div>
        <p class="usage-bench-foot">
          Wall clock, argv to parsed struct: <strong>2.1µs</strong> vs clap's 490µs.
          Heap allocations for a bare parse: <strong>zero</strong> vs 6,560.
        </p>
      </div>

      <div class="usage-bench-card">
        <h3>Go <span>vs cobra, urfave/cli, kong</span></h3>
        <div class="usage-bench-rows">
          <div v-for="row in goRows" :key="row.name" class="usage-bench-row">
            <span class="usage-bench-name">{{ row.name }}</span>
            <span class="usage-bench-track">
              <span
                class="usage-bench-bar"
                :class="{ us: row.us }"
                :style="{ width: width(row.value, goMax) }"
              ></span>
              <span class="usage-bench-value"
                >{{ row.label }}<em v-if="row.note"> · {{ row.note }}</em></span
              >
            </span>
          </div>
        </div>
        <p class="usage-bench-foot">
          Whole-process wall clock: <strong>1.1ms</strong> vs cobra's 1.8ms — of which
          0.95ms is Go runtime startup no parser can touch.
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
