<script setup lang="ts">
// Rust figures from the `perf-pr` workflow's pinned runner, which runs
// `tasks/perf-shadow.sh`.
//
// The bars are instruction counts, because that is the measure that holds still:
// deterministic for a given binary, and a developer machine and the runner agreed
// on them to within 0.15%. They are also genuinely one *cold* parse — `parse-n` is differenced
// against the same binary parsing nothing, so the figure is a single parse in a
// fresh process.
//
// Wall clock lives in the footnote instead, and is deliberately vaguer. It comes
// from `time-sweep.rs`, which warms each parser and keeps the fastest of many
// short rounds, so it is steady-state rather than cold. Across twenty-two runs on
// two machines the minima moved a few percent and their ratios by 10% — clap read
// 2,226x, 2,502x and 2,419x on three occasions — which is a range worth quoting
// loosely and not a number worth putting on a bar.
const rustRows = [
  { name: "usage-rs", value: 4155, label: "4,155", us: true },
  { name: "argh", value: 6295, label: "6,295", note: "1.5× more", us: false },
  { name: "clap", value: 5894561, label: "5.89M", note: "1,418× more", us: false },
  { name: "bpaf", value: 21917918, label: "21.9M", note: "5,275× more", us: false },
];

// Go figures from go/README.md are whole-process wall time and subtract the
// ~0.95ms a do-nothing Go process costs, so they are deliberately approximate.
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
        <p class="usage-bench-metric">instructions, one cold parse</p>
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
          usage-rs and argh are the two bars you cannot see, and neither is a number anyone
          could feel — a process costs ~1ms to start. The gap that matters is to the two
          that build a parser before they can use one: most of clap's is constructing and
          validating its command tree, and bpaf's is larger because it assembles a
          combinator tree per run, which reusing the parser across parses only halves.
          Instructions rather than time because they hold still: deterministic per binary,
          agreeing to within 0.15% across two machines, and genuinely one parse in a fresh
          process. In time, warmed and on one machine, the same four are roughly
          <strong>0.2µs</strong> · 0.3µs · 480µs · 1.6ms — loosely, since those minima and
          their ratios move several percent between runs and between machines. Heap allocations for a
          bare parse: <strong>zero</strong> vs clap's 6,280. What argh and bpaf cannot
          express — aliases, hidden commands, global flags, a positional beside a
          subcommand — is counted by the generator and printed when it runs.
        </p>
      </div>

      <div class="usage-bench-card">
        <h3>usage-go <span>vs cobra, urfave/cli, kong</span></h3>
        <p class="usage-bench-metric">wall time, one cold parse</p>
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
      ·
      <a
        href="https://github.com/jdx/usage/blob/main/benches/gate/src/bin/time-sweep.rs"
        >time-sweep.rs</a
      >
    </p>
  </section>
</template>
