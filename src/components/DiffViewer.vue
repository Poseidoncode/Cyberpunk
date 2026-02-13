<script setup lang="ts">
import { DiffInfo } from "../services/git";

defineProps<{
  diffs: DiffInfo[];
}>();

const getLineColor = (line: string) => {
  if (line.startsWith('+')) return 'var(--success)';
  if (line.startsWith('-')) return 'var(--error)';
  if (line.startsWith('@@')) return 'var(--muted-foreground)';
  return 'var(--foreground)';
};

const getLineBg = (line: string) => {
  if (line.startsWith('+')) return 'rgba(16, 185, 129, 0.08)'; 
  if (line.startsWith('-')) return 'rgba(239, 68, 68, 0.08)';
  if (line.startsWith('@@')) return 'rgba(100, 116, 139, 0.05)';
  return 'transparent';
};

const parseLine = (line: string) => {
  if (!line) return '';
  const firstChar = line.charAt(0);
  if (firstChar === '+' || firstChar === '-' || firstChar === ' ') {
    return line.substring(1);
  }
  return line;
};

const MAX_LINES_PER_FILE = 500;

const getLines = (diffText: string) => {
  const lines = diffText.split('\n');
  if (lines.length > MAX_LINES_PER_FILE) {
    return {
      visible: lines.slice(0, MAX_LINES_PER_FILE),
      truncated: true,
      count: lines.length
    };
  }
  return {
    visible: lines,
    truncated: false,
    count: lines.length
  };
};
</script>

<template>
  <div v-if="diffs.length === 0" class="flex items-center justify-center p-16 text-muted-foreground text-sm italic">
    No differences to show
  </div>
  <div v-else class="diff-viewer bg-background select-text">
    <div v-for="(diff, i) in diffs" :key="i" class="diff-file border-b border-border last:border-b-0">
      <div class="diff-file-header bg-muted/50 backdrop-blur-sm sticky top-0 z-10 px-4 py-2.5 border-b border-border font-sans text-xs flex justify-between items-center shadow-sm">
        <div class="flex items-center gap-3">
          <span class="w-2 h-2 rounded-full" :class="{ 'bg-success': diff.additions > 0 && diff.deletions === 0, 'bg-error': diff.deletions > 0 && diff.additions === 0, 'bg-accent': diff.additions > 0 && diff.deletions > 0 }"></span>
          <span class="font-bold text-foreground tracking-tight">{{ diff.path }}</span>
        </div>
        <span class="text-[10px] font-mono flex items-center gap-2">
          <span class="text-success bg-success/10 px-1.5 py-0.5 rounded font-bold">+{{ diff.additions }}</span>
          <span class="text-error bg-error/10 px-1.5 py-0.5 rounded font-bold">-{{ diff.deletions }}</span>
        </span>
      </div>
      <div class="diff-content font-mono text-[11px] bg-card overflow-hidden">
        <div v-for="(line, j) in getLines(diff.diff_text).visible" :key="j"
             class="flex group hover:bg-muted/30 transition-colors border-l-4 border-transparent"
             :style="{ color: getLineColor(line), backgroundColor: getLineBg(line), borderLeftColor: line.startsWith('+') ? 'var(--success)' : (line.startsWith('-') ? 'var(--error)' : 'transparent') }">
          <span class="w-12 text-right pr-4 py-0.5 text-muted-foreground/40 select-none border-r border-border/10 group-hover:text-muted-foreground transition-colors">{{ j + 1 }}</span>
          <span class="flex-1 px-4 py-0.5 whitespace-pre-wrap break-all leading-relaxed">{{ parseLine(line) }}</span>
        </div>
        <div v-if="getLines(diff.diff_text).truncated" class="p-4 text-center bg-muted/20 text-muted-foreground text-[10px] italic border-t border-border/10">
          ... Showing only first {{ MAX_LINES_PER_FILE }} of {{ getLines(diff.diff_text).count }} lines. File too large to display fully.
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.diff-viewer {
  --success: #10B981;
  --error: #EF4444;
  --accent: #3b82f6;
}
</style>
