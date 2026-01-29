<script setup lang="ts">
import { DiffInfo } from "../services/git";

defineProps<{
  diffs: DiffInfo[];
}>();

const getLineColor = (line: string) => {
  if (line.startsWith('+')) return '#10B981'; // Emerald for additions
  if (line.startsWith('-')) return '#EF4444'; // Red for deletions
  if (line.startsWith('@@')) return '#64748B'; // Slate for metadata
  return '#0F172A'; // Foreground for context
};

const getLineBg = (line: string) => {
  if (line.startsWith('+')) return 'rgba(16, 185, 129, 0.05)'; // Light emerald bg
  if (line.startsWith('-')) return 'rgba(239, 68, 68, 0.05)'; // Light red bg
  return 'transparent';
};
</script>

<template>
  <div v-if="diffs.length === 0" class="flex items-center justify-center p-16 text-muted-foreground text-sm">
    No differences to show
  </div>
  <div v-else class="diff-viewer bg-background">
    <div v-for="(diff, i) in diffs" :key="i" class="diff-file border-b border-border last:border-b-0">
      <div class="diff-file-header bg-muted px-4 py-3 border-b border-border font-sans text-xs flex justify-between items-center">
        <span class="font-semibold text-foreground">{{ diff.path }}</span>
        <span class="text-muted-foreground font-mono">
          <span class="text-success">+{{ diff.additions }}</span>
          <span class="mx-2">·</span>
          <span class="text-error">-{{ diff.deletions }}</span>
        </span>
      </div>
      <pre class="diff-content p-4 font-mono text-xs overflow-auto bg-card"><div v-for="(line, j) in diff.diff_text.split('\n')" :key="j"
             class="px-2 py-0.5 leading-relaxed"
             :style="{ color: getLineColor(line), backgroundColor: getLineBg(line) }">{{ line }}</div></pre>
    </div>
  </div>
</template>
