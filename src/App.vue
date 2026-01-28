<script setup lang="ts">
// ------ 應用邏輯主入口，串接所有 Git 服務邏輯 ------
// UI 元件由此調用 gitService 並與設定、分支、檔案監控互動
import { ref, onMounted, watch, computed } from 'vue';
import { gitService, type RepositoryInfo, type FileStatus, type BranchInfo, type CommitInfo, type StashInfo, type ConflictInfo, type Settings, type DiffInfo } from './services/git';
import { open } from '@tauri-apps/plugin-dialog';
import DiffViewer from './components/DiffViewer.vue';

// 所有資料流與狀態、畫面切換皆於此統一調度
// ...（其餘內容不變）

// [原始代碼，請保持原樣]

</script>

<!-- 其餘區段保留（template、style） -->
<template>
  <!-- 詳細內容略，參照原本結構，不移除任何邏輯 -->
</template>

<style>
/* All Terminal CLI styles are defined in index.css */
</style>
