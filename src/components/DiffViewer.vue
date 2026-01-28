<script setup lang="ts">
import { DiffInfo } from "../services/git";

defineProps<{
  diffs: DiffInfo[];
}>();

const getLineColor = (line: string) => {
  if (line.startsWith('+')) return '#33ff00'; // Terminal green for additions
  if (line.startsWith('-')) return '#ff3333'; // Terminal red for deletions
  if (line.startsWith('@@')) return '#ffb000'; // Terminal amber for metadata
  return '#33ff00'; // Default terminal green
};
</script>

<template>
  <div v-if="diffs.length === 0" class="empty-state p-8 text-center text-terminal-muted text-xs uppercase">
    > NO_DIFFERENCES_TO_SHOW
  </div>
  <div v-else class="diff-viewer">
    <div v-for="(diff, i) in diffs" :key="i" class="diff-file border-b border-terminal-border">
      <div class="diff-file-header bg-terminal-bg p-2 border-b border-terminal-border font-mono text-[10px] uppercase flex justify-between">
        <span class="text-terminal-primary">FILE: {{ diff.path }}</span>
        <span class="text-terminal-muted">
          +{{ diff.additions }} -{{ diff.deletions }}
        </span>
      </div>
      <pre class="diff-content p-4 font-mono text-[10px] overflow-auto bg-terminal-bg">
        <div v-for="(line, j) in diff.diff_text.split('\n')" :key="j"
             :style="{ color: getLineColor(line), whiteSpace: 'pre-wrap' }">
          {{ line }}
        </div>
      </pre>
    </div>
  </div>
</template>
