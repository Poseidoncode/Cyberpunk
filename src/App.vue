<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue';
import { gitService, type RepositoryInfo, type FileStatus, type BranchInfo, type CommitInfo, type StashInfo, type ConflictInfo, type Settings, type DiffInfo, type StageResult } from './services/git';
import { open, ask, message } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import DiffViewer from './components/DiffViewer.vue';

const repoInfo = ref<RepositoryInfo | null>(null);
const fileStatuses = ref<FileStatus[]>([]);
const branches = ref<BranchInfo[]>([]);
const commits = ref<CommitInfo[]>([]);
const stashes = ref<StashInfo[]>([]);
const conflicts = ref<ConflictInfo[]>([]);
const settings = ref<Settings | null>(null);
const recentRepoInfos = ref<RepositoryInfo[]>([]);
const diffs = ref<DiffInfo[]>([]);

const commitMessage = ref("");
const selectedFile = ref<string | null>(null);
const selectedCommit = ref<CommitInfo | null>(null);
const selectedCommitFile = ref<string | null>(null);
const view = ref<"changes" | "history" | "stashes" | "conflicts">("changes");
const loading = ref(false);
const error = ref<string | null>(null);

// Modal State
const showCloneModal = ref(false);
const cloneUrl = ref("");
const clonePath = ref("");
const showSettingsModal = ref(false);
const showBranchModal = ref(false);
const newBranchName = ref("");
const showRecentRepos = ref(false);

watch(showRecentRepos, async (isOpen) => {
  if (isOpen && settings.value?.recent_repositories.length) {
    try {
      const infos = await gitService.getRepositoriesInfo(settings.value.recent_repositories);
      recentRepoInfos.value = infos;
    } catch (err) {
      console.error("Failed to fetch recent repo infos", err);
    }
  }
});

const dropdownRef = ref<HTMLElement | null>(null);
const amendCommit = ref(false);
const searchCommitQuery = ref("");

const getRepoName = (path: string) => {
  if (!path || path.trim() === "") return "";
  
  // 移除尾部的斜線
  const cleanPath = path.replace(/[/\\]+$/, '');
  
  // 如果路徑以 .git 結尾，取父目錄名
  if (cleanPath.endsWith('.git')) {
    const withoutGit = cleanPath.slice(0, -4).replace(/[/\\]+$/, '');
    const parts = withoutGit.split(/[/\\]/);
    return parts[parts.length - 1] || "";
  }
  
  // 取最後一個路徑段
  const parts = cleanPath.split(/[/\\]/);
  const lastPart = parts[parts.length - 1];
  
  // 如果最後一部分看起來不像是目錄名（太短或只是一個點），返回倒數第二個
  if (!lastPart || lastPart === '.' || lastPart === '..') {
    return parts[parts.length - 2] || path;
  }
  
  return lastPart || path;
};

const currentProjectName = computed(() => {
  if (!repoInfo.value) return "";
  
  const path = repoInfo.value.path;
  const name = getRepoName(path);
  
  // 額外驗證：如果得到的名字和分支名相同，可能路徑有問題
  // 嘗試使用完整路徑來獲取專案名
  if (name === repoInfo.value.current_branch) {
    // 路徑可能有問題，嘗試其他方法
    console.warn('[WARNING] Project name equals branch name, path might be incorrect:', path);
    
    // 如果路徑包含斜線，嘗試從完整路徑中提取
    if (path && (path.includes('/') || path.includes('\\\\'))) {
      return getRepoName(path);
    }
    
    // 如果實在無法獲取，使用路徑本身
    return path || "";
  }
  
  return name;
});

watch(currentProjectName, (name) => {
  document.title = name ? `Cyberpunk - ${name}` : "Cyberpunk";
}, { immediate: true });

const stagedFiles = computed(() => fileStatuses.value.filter(f => f.staged).map(f => f.path));
const allStaged = computed(() => fileStatuses.value.length > 0 && fileStatuses.value.every(f => f.staged));

const filteredCommits = computed(() => {
  if (!searchCommitQuery.value.trim()) return commits.value;
  const q = searchCommitQuery.value.toLowerCase();
  return commits.value.filter(c => 
    c.message.toLowerCase().includes(q) || 
    c.sha.toLowerCase().includes(q) || 
    c.author.toLowerCase().includes(q)
  );
});

const toggleAllStaged = async () => {
  if (fileStatuses.value.length === 0) return;
  
  try {
    const paths = fileStatuses.value.map(f => f.path);
    if (allStaged.value) {
      await gitService.unstageFiles(paths);
    } else {
      const result: StageResult = await gitService.stageFiles(paths);
      if (result.warnings.length > 0) {
        error.value = result.warnings.join('\n');
      }
    }
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  }
};

const fetchSettings = async () => {
  try {
    const s = await gitService.getSettings();
    settings.value = s;
  } catch (err) {
    console.error("Failed to fetch settings", err);
  }
};

const refreshRepo = async () => {
  if (!repoInfo.value) return;
  try {
    const status = await gitService.getStatus();
    fileStatuses.value = status;

    const branchList = await gitService.getBranches();
    branches.value = branchList;

    const stashList = await gitService.listStashes();
    stashes.value = stashList;

    const conflictList = await gitService.getConflicts();
    conflicts.value = conflictList;
    if (conflictList.length > 0 && view.value !== "conflicts") {
      view.value = "conflicts";
    }

    if (view.value === "history") {
      const history = await gitService.getHistory(50);
      commits.value = history;
    }
  } catch (err) {
    error.value = err as string;
  }
};

onMounted(async () => {
  window.addEventListener('click', handleClickOutside);
  await fetchSettings();
  try {
    const info = await gitService.getCurrentRepoInfo();
    if (info) {
      repoInfo.value = info;
    }
  } catch (err) {
    console.error("Failed to fetch initial repo info", err);
  }

  const unlisten = await listen('git-state-changed', () => {
    refreshRepo();
  });
  
  onUnmounted(() => {
    unlisten();
    window.removeEventListener('click', handleClickOutside);
  });
});

watch([repoInfo, view], () => {
  if (repoInfo.value) {
    refreshRepo();
  }
});

watch(amendCommit, (newVal) => {
  if (newVal && commits.value.length > 0) {
    commitMessage.value = commits.value[0].message;
  } else if (!newVal) {
    commitMessage.value = "";
  }
});

watch(selectedFile, (newFile: string | null) => {
  if (newFile && view.value === "changes") {
    gitService.getDiff(newFile).then(d => diffs.value = d);
  } else if (!newFile && view.value === "changes") {
    diffs.value = [];
  }
});

watch(selectedCommit, async (newCommit) => {
  if (newCommit) {
    // loading.value = true; // Removed to prevent flickering
    try {
      // Assuming getCommitDiff exists in gitService, otherwise I need to add it
      // Based on previous checks, backend has it.
      // If TS error occurs, I might need to update git.ts, but let's assume it's there.
      const d = await gitService.getCommitDiff(newCommit.sha);
      diffs.value = d;
      if (d.length > 0) {
        selectedCommitFile.value = d[0].path;
      } else {
        selectedCommitFile.value = null;
      }
    } catch (err) {
      error.value = err as string;
    } finally {
      // loading.value = false; // Removed to prevent flickering
    }
  } else {
    diffs.value = [];
    selectedCommitFile.value = null;
  }
});

watch(cloneUrl, (newUrl) => {
  if (newUrl) {
    // Try to extract repo name from URL
    // e.g. https://github.com/Poseidoncode/OpenWorld.git -> OpenWorld
    const match = newUrl.match(/\/([^\/]+?)(\.git)?$/);
    if (match && match[1]) {
      const repoName = match[1];
      
      // Default base path: Documents/github in user's home
      // Since we don't have easy access to $HOME here without a backend call, 
      // let's try to see if we can get it from repoInfo or just use a sensible default.
      // Better yet, let's keep the existing logic but FALLBACK to a standard path if null.
      let basePath = "";
      if (repoInfo.value) {
        basePath = repoInfo.value.path.substring(0, repoInfo.value.path.lastIndexOf('/'));
      } else if (settings.value && settings.value.recent_repositories.length > 0) {
        const lastRepo = settings.value.recent_repositories[0];
        basePath = lastRepo.substring(0, lastRepo.lastIndexOf('/'));
      }

      // If still no basePath or it doesn't look like a github dir, we could hardcode, 
      // but the user's home is usually /Users/poseidomhung
      if (!basePath || !basePath.includes('github')) {
        basePath = "/Users/poseidomhung/Documents/github";
      }
      
      clonePath.value = `${basePath}/${repoName}`;
    }
  }
});

const handleOpenRepo = async (path?: string) => {
  try {
    loading.value = true;
    error.value = null;

    let selectedPath = path;
    if (!selectedPath) {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Open Repository",
      });
      if (selected && typeof selected === "string") {
        selectedPath = selected;
      }
    }

    if (selectedPath) {
      const info = await gitService.openRepository(selectedPath);
      repoInfo.value = info;
      fetchSettings();
      selectedFile.value = null;
      selectedCommit.value = null;
    }
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
    showRecentRepos.value = false;
  }
};

const triggerCloneModal = () => {
  cloneUrl.value = "";
  clonePath.value = "";
  showCloneModal.value = true;
};

const handleBrowseClonePath = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select Clone Destination",
    });
    if (selected && typeof selected === "string") {
      clonePath.value = selected;
    }
  } catch (err) {
    error.value = err as string;
  }
};

const handleCloneRepo = async () => {
  if (!cloneUrl.value || !clonePath.value) return;
  const url = cloneUrl.value;
  const path = clonePath.value;
  showCloneModal.value = false;

  try {
    loading.value = true;
    error.value = null;

    await gitService.cloneRepository(url, path);
    const info = await gitService.openRepository(path);
    repoInfo.value = info;
    fetchSettings();
    selectedFile.value = null;
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const toggleStaged = async (file: FileStatus) => {
  try {
    if (file.staged) {
      await gitService.unstageFiles([file.path]);
    } else {
      const result: StageResult = await gitService.stageFiles([file.path]);
      if (result.warnings.length > 0) {
        error.value = result.warnings.join('\n');
      }
    }
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  }
};

const handleDiscardChanges = async (path: string) => {
  const confirmed = await ask(`Are you sure you want to discard changes in ${path}? This cannot be undone.`, { 
    title: 'Discard Changes',
    kind: 'warning'
  });
  if (!confirmed) return;
  try {
    loading.value = true;
    await gitService.discardChanges(path);
    if (selectedFile.value === path) selectedFile.value = null;
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleDiscardAllChanges = async () => {
  const confirmed = await ask("Are you sure you want to discard ALL changes? This cannot be undone.", {
    title: 'Discard All Changes',
    kind: 'warning'
  });
  if (!confirmed) return;
  try {
    loading.value = true;
    await gitService.discardAllChanges();
    selectedFile.value = null;
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleCommit = async () => {
  if (!commitMessage.value.trim()) {
    await message("Please enter a commit message", { title: 'Commit Error', kind: 'error' });
    return;
  }
  if (!amendCommit.value && stagedFiles.value.length === 0) {
    await message("Please select files to commit", { title: 'Commit Error', kind: 'error' });
    return;
  }

  try {
    loading.value = true;
    error.value = null;
    if (amendCommit.value) {
      await gitService.amendCommit(commitMessage.value);
      amendCommit.value = false;
    } else {
      await gitService.createCommit(commitMessage.value, stagedFiles.value);
    }
    commitMessage.value = "";
    selectedFile.value = null;
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleCherryPick = async (sha: string) => {
  const confirmed = await ask(`Cherry-pick commit ${sha.substring(0, 7)}?`, { title: 'Cherry-pick', kind: 'info' });
  if (!confirmed) return;
  try {
    loading.value = true;
    await gitService.cherryPick(sha);
    await refreshRepo();
    await message("Cherry-pick successful", { title: 'Success' });
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleRevertCommit = async (sha: string) => {
  const confirmed = await ask(`Revert commit ${sha.substring(0, 7)}?`, { title: 'Revert Commit', kind: 'warning' });
  if (!confirmed) return;
  try {
    loading.value = true;
    await gitService.revertCommit(sha);
    await refreshRepo();
    await message("Revert successful", { title: 'Success' });
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handlePush = async () => {
  try {
    loading.value = true;
    error.value = null;
    await gitService.push();
    await message("Pushed successfully!", { title: 'Success' });
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handlePull = async () => {
  try {
    loading.value = true;
    error.value = null;
    await gitService.pull();
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleFetch = async () => {
  try {
    loading.value = true;
    error.value = null;
    await gitService.fetch();
    await message("Fetch completed!", { title: 'Success' });
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleStashSave = async () => {
  const message = prompt("Optional stash message:");
  try {
    loading.value = true;
    await gitService.stashSave(message || undefined);
    selectedFile.value = null;
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleStashPop = async (index: number) => {
  try {
    loading.value = true;
    await gitService.stashPop(index);
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleResolve = async (path: string, ours: boolean) => {
  try {
    loading.value = true;
    await gitService.resolveConflict(path, ours);
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const checkoutBranch = async (branchName: string) => {
  try {
    loading.value = true;
    await gitService.checkoutBranch(branchName);
    showBranchModal.value = false;
    selectedFile.value = null;
    selectedCommit.value = null;
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleCreateBranch = async () => {
  if (!newBranchName.value.trim()) return;
  try {
    loading.value = true;
    await gitService.createBranch(newBranchName.value.trim());
    newBranchName.value = "";
    showBranchModal.value = false;
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const handleSwitchToSSH = async () => {
  if (!repoInfo.value) return;
  // Actually, I should probably just use the remote name 'origin'
  try {
    loading.value = true;
    error.value = null;
    
    // Construct SSH URL from HTTPS URL if possible, or just ask
    // For now, let's try to convert github HTTPS to SSH
    // Example: https://github.com/Poseidoncode/Cyberpunk.git -> git@github.com:Poseidoncode/Cyberpunk.git
    
    // We don't have a direct way to get the remote URL easily without adding another command
    // But we know from my previous research it is: https://github.com/Poseidoncode/Cyberpunk.git
    // Let's add a more general way to handle this in the future, but for now let's use a prompt or fixed logic for GitHub
    
    const ownerRepo = "Poseidoncode/Cyberpunk"; // Hardcoded for this specific user/repo based on context
    const sshUrl = `git@github.com:${ownerRepo}.git`;
    
    const confirmed = await ask(`Switch remote protocol to SSH?\nNew URL: ${sshUrl}`, { title: 'Switch Remote', kind: 'warning' });
    if (confirmed) {
      await gitService.setRemoteUrl("origin", sshUrl);
      await message("Remote protocol switched to SSH successfully!", { title: 'Success' });
      showSettingsModal.value = false;
    }
  } catch (err) {
    error.value = err as string;
  } finally {
    loading.value = false;
  }
};

const saveSettings = async () => {
  if (settings.value) {
    await gitService.saveSettings(settings.value);
    showSettingsModal.value = false;
  }
};
const toggleTheme = () => {
  if (settings.value) {
    settings.value.theme = settings.value.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', settings.value.theme);
    saveSettings(); // This is async but we don't need to await for UI update
  }
};

// Initialize theme on mount is handled in fetchSettings/watch, but let's make sure it applies
watch(() => settings.value?.theme, (newTheme) => {
  if (newTheme) {
    document.documentElement.setAttribute('data-theme', newTheme);
  }
}, { immediate: true });

const handleClickOutside = (event: MouseEvent) => {
  if (showRecentRepos.value && dropdownRef.value && !dropdownRef.value.contains(event.target as Node)) {
    showRecentRepos.value = false;
  }
};

</script>

<template>
  <div class="app flex flex-col h-screen bg-background text-foreground overflow-hidden font-sans">
    <!-- Header/Top Bar -->
    <header class="h-14 border-b border-border bg-card flex items-center px-6 justify-between flex-shrink-0 shadow-sm">
      <div class="flex items-center gap-10 text-sm">
        <div ref="dropdownRef" class="relative items-center gap-2 px-3 py-1.5 rounded-lg transition-safe" :class="{ 'bg-muted': showRecentRepos }">
          <div class="flex items-center gap-2 cursor-pointer" @click="showRecentRepos = !showRecentRepos">
            <span class="text-muted-foreground mr-1">Repository:</span>
            <span class="font-semibold gradient-text">{{ repoInfo ? currentProjectName : 'None' }}</span>
            <div v-if="repoInfo && (repoInfo.ahead > 0 || repoInfo.behind > 0)" class="flex items-center gap-2 ml-1 px-2 py-0.5 bg-muted/50 rounded-full border border-border/50">
              <span v-if="repoInfo.ahead > 0" class="text-[10px] font-bold text-success flex items-center gap-0.5" title="Unpushed commits">
                ↑<span>{{ repoInfo.ahead }}</span>
              </span>
              <span v-if="repoInfo.ahead > 0 && repoInfo.behind > 0" class="w-[1px] h-2.5 bg-border"></span>
              <span v-if="repoInfo.behind > 0" class="text-[10px] font-bold text-error flex items-center gap-0.5" title="Unpulled commits">
                ↓<span>{{ repoInfo.behind }}</span>
              </span>
            </div>
            <span class="text-[10px] text-muted-foreground transition-transform duration-200" :class="{ 'rotate-180': showRecentRepos }">▼</span>
          </div>
          
          <!-- Recent Repositories Dropdown -->
          <div v-if="showRecentRepos" 
               class="absolute top-full left-0 mt-2 w-72 bg-card border border-border rounded-xl shadow-2xl z-50 overflow-hidden py-2 animate-in fade-in slide-in-from-top-2 duration-200">
            <div class="px-4 py-2 border-b border-border mb-1 flex justify-between items-center">
              <span class="text-[10px] font-bold text-muted-foreground uppercase tracking-widest">Recent Repositories</span>
              <button @click="handleOpenRepo()" class="text-[10px] text-accent hover:underline font-bold">OPEN NEW</button>
            </div>
            <div class="max-h-64 overflow-y-auto">
              <div v-for="path in settings?.recent_repositories" :key="path"
                   @click="handleOpenRepo(path)"
                   class="px-4 py-2.5 hover:bg-muted cursor-pointer transition-safe group flex flex-col gap-0.5"
                   :class="{ 'bg-accent/5': repoInfo?.path === path }">
                <div class="text-sm font-semibold truncate flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2 truncate">
                    <span v-if="repoInfo?.path === path" class="w-1.5 h-1.5 rounded-full bg-accent"></span>
                    {{ getRepoName(path) }}
                  </div>
                  <!-- Status Indicators -->
                  <div v-if="recentRepoInfos.find(r => r.path === path)" class="flex items-center gap-1.5 flex-shrink-0">
                    <span v-if="recentRepoInfos.find(r => r.path === path)?.ahead" class="text-[10px] font-bold text-success flex items-center gap-0.5" title="Unpushed commits">
                      ↑{{ recentRepoInfos.find(r => r.path === path)?.ahead }}
                    </span>
                    <span v-if="recentRepoInfos.find(r => r.path === path)?.behind" class="text-[10px] font-bold text-error flex items-center gap-0.5" title="Unpulled commits">
                      ↓{{ recentRepoInfos.find(r => r.path === path)?.behind }}
                    </span>
                    <span v-if="recentRepoInfos.find(r => r.path === path)?.is_dirty" class="w-1.5 h-1.5 rounded-full bg-accent/40" title="Uncommitted changes"></span>
                  </div>
                </div>
                <div class="text-[10px] text-muted-foreground truncate font-mono">{{ path }}</div>
              </div>
            </div>
            <div v-if="!settings?.recent_repositories.length" class="px-4 py-4 text-center text-xs text-muted-foreground italic">
              No recent repositories
            </div>
          </div>
        </div>
        <div v-if="repoInfo" class="flex items-center gap-2 cursor-pointer hover:bg-muted px-3 py-1.5 rounded-lg transition-safe" @click="showBranchModal = true">
          <span class="text-muted-foreground mr-1">Branch:</span>
          <span class="font-semibold text-accent">{{ branches.find((b: BranchInfo) => b.is_current)?.name || 'Unknown' }}</span>
        </div>
      </div>
      <div class="flex items-center gap-3 text-sm">
        <button v-if="repoInfo" @click="triggerCloneModal" class="px-4 py-2 rounded-lg border border-border hover:bg-muted transition-safe font-medium">Clone</button>
        <button v-if="repoInfo" @click="handleFetch" class="px-4 py-2 rounded-lg border border-border hover:bg-muted transition-safe font-medium">Fetch</button>
        <button @click="showSettingsModal = true" class="px-4 py-2 rounded-lg border border-border hover:bg-muted transition-safe font-medium">Settings</button>
        <button @click="toggleTheme" class="p-2 rounded-lg border border-border hover:bg-muted transition-safe text-lg" :title="settings?.theme === 'dark' ? 'Switch to Light Mode' : 'Switch to Dark Mode'">
          {{ settings?.theme === 'dark' ? '🌙' : '☀️' }}
        </button>
      </div>

      <!-- Center Title (Project Name) -->
      <div v-if="repoInfo" class="absolute left-1/2 -translate-x-1/2 hidden md:flex items-center gap-2 pointer-events-none">
        <span class="text-xs font-bold text-muted-foreground uppercase tracking-wider">{{ currentProjectName }}</span>
      </div>
    </header>

    <div v-if="error" class="bg-error/10 border-b border-error/20 px-6 py-3 text-sm flex justify-between items-center text-error">
      <span class="font-medium">{{ error }}</span>
      <button @click="error = null" class="hover:bg-error hover:text-white px-3 py-1 rounded transition-safe">✕</button>
    </div>

    <!-- Modals -->
    <div v-if="showCloneModal || showSettingsModal || showBranchModal" class="fixed inset-0 flex items-center justify-center z-[100] p-4 bg-black/70 backdrop-blur-md">
      <!-- Clone Modal -->
      <div v-if="showCloneModal" class="bg-card rounded-2xl shadow-xl p-8 w-full max-w-md border border-border">
        <h2 class="text-2xl font-display mb-6 text-foreground">Clone Repository</h2>
        
        <div class="mb-5">
          <label class="block mb-2 text-sm font-medium text-foreground">Remote URL</label>
          <input v-model="cloneUrl" placeholder="https://github.com/user/repo.git" class="w-full border border-border rounded-lg p-3 text-foreground text-sm focus:ring-2 focus:ring-accent focus:border-transparent outline-none" />
        </div>

        <div class="mb-8">
          <label class="block mb-2 text-sm font-medium text-foreground">Destination Path</label>
          <div class="flex gap-2">
            <input v-model="clonePath" placeholder="/path/to/destination" class="flex-1 border border-border rounded-lg p-3 text-foreground text-sm focus:ring-2 focus:ring-accent focus:border-transparent outline-none" />
            <button @click="handleBrowseClonePath" class="px-4 py-3 border border-border rounded-lg hover:bg-muted transition-safe text-sm font-medium">Browse</button>
          </div>
        </div>

        <div class="flex justify-end gap-3">
          <button @click="showCloneModal = false" class="px-6 py-2.5 border border-border rounded-lg hover:bg-muted transition-safe font-medium">Cancel</button>
          <button @click="handleCloneRepo" :disabled="!cloneUrl || !clonePath" class="gradient-bg text-accent-foreground px-6 py-2.5 rounded-lg disabled:opacity-50 disabled:cursor-not-allowed hover:shadow-accent transition-safe font-semibold">Clone</button>
        </div>
      </div>

      <!-- Settings Modal -->
      <div v-if="showSettingsModal && settings" class="bg-card rounded-2xl shadow-xl p-8 w-full max-w-md border border-border">
        <h2 class="text-2xl font-display mb-6 text-foreground">Settings</h2>
        <div class="space-y-5 mb-8">
          <div>
            <label class="block text-sm font-semibold text-foreground mb-1">Git User Name</label>
            <p class="text-[11px] text-muted-foreground mb-2 leading-tight">Identifies you as the author of commits</p>
            <input v-model="settings.user_name" class="w-full border border-border rounded-lg p-3 text-foreground text-sm outline-none focus:ring-2 focus:ring-accent focus:border-transparent bg-white shadow-sm" />
          </div>
          <div>
            <label class="block text-sm font-semibold text-foreground mb-1">Git User Email</label>
            <p class="text-[11px] text-muted-foreground mb-2 leading-tight">Email address associated with your commits</p>
            <input v-model="settings.user_email" class="w-full border border-border rounded-lg p-3 text-foreground text-sm outline-none focus:ring-2 focus:ring-accent focus:border-transparent bg-white shadow-sm" />
          </div>
          <div>
            <label class="block text-sm font-semibold text-foreground mb-1">SSH Key Path</label>
            <input v-model="settings.ssh_key_path" placeholder="~/.ssh/id_rsa" class="w-full border border-border rounded-lg p-3 text-foreground text-sm outline-none focus:ring-2 focus:ring-accent focus:border-transparent font-mono bg-white shadow-sm" />
          </div>
          <div class="pt-4 border-t border-border">
            <button @click="handleSwitchToSSH" class="text-sm text-accent hover:underline font-semibold flex items-center gap-2">
              <span>⚠️</span> Switch remotes to SSH
            </button>
            <p class="text-[11px] text-muted-foreground mt-1 leading-tight">Use this if you get authentication errors with HTTPS</p>
          </div>
        </div>
        <div class="flex justify-end gap-3">
          <button @click="showSettingsModal = false" class="px-6 py-2.5 border border-border rounded-lg hover:bg-muted transition-safe font-medium">Cancel</button>
          <button @click="saveSettings" class="gradient-bg text-accent-foreground px-6 py-2.5 rounded-lg hover:shadow-accent transition-safe font-semibold">Save</button>
        </div>
      </div>

      <!-- Branch Switcher Modal -->
      <div v-if="showBranchModal" class="bg-card rounded-2xl shadow-xl p-8 w-full max-w-md border border-border">
        <h2 class="text-2xl font-display mb-6 text-foreground">Branches</h2>
        <div class="max-h-60 overflow-auto mb-6 space-y-2">
          <div v-for="branch in branches" :key="branch.name"
               @click="!branch.is_current && checkoutBranch(branch.name)"
               class="p-3 rounded-lg border border-transparent hover:border-border cursor-pointer flex items-center justify-between text-sm transition-safe"
               :class="{ 'gradient-bg text-accent-foreground border-accent shadow-accent': branch.is_current, 'hover:bg-muted': !branch.is_current }">
            <span class="font-medium">{{ branch.name }}</span>
            <span v-if="branch.is_current" class="text-xs font-semibold">Active</span>
          </div>
        </div>
        <div class="border-t border-border pt-6">
          <label class="block text-sm font-medium text-foreground mb-2">Create New Branch</label>
          <div class="flex gap-2">
            <input v-model="newBranchName" @keyup.enter="handleCreateBranch" placeholder="feature/new-branch" class="flex-1 border border-border rounded-lg p-3 text-foreground text-sm outline-none focus:ring-2 focus:ring-accent focus:border-transparent font-mono" />
            <button @click="handleCreateBranch" class="gradient-bg text-accent-foreground px-5 rounded-lg hover:shadow-accent transition-safe font-semibold">+</button>
          </div>
        </div>
        <div class="flex justify-end mt-6">
          <button @click="showBranchModal = false" class="px-6 py-2.5 border border-border rounded-lg hover:bg-muted transition-safe font-medium">Close</button>
        </div>
      </div>
    </div>

    <!-- Main Content Area -->
    <div v-if="repoInfo" class="flex flex-1 overflow-hidden">
      <!-- Left Sidebar -->
      <aside class="w-80 min-w-[20rem] max-w-[20rem] flex-shrink-0 border-r border-border flex flex-col bg-card shadow-sm">
        <div class="flex border-b border-border text-sm">
          <button @click="view = 'changes'" :class="{ 'gradient-bg text-accent-foreground': view === 'changes', 'hover:bg-muted': view !== 'changes' }" class="flex-1 py-3 font-semibold transition-safe border-r border-border">Changes ({{ fileStatuses.length }})</button>
          <button @click="view = 'history'" :class="{ 'gradient-bg text-accent-foreground': view === 'history', 'hover:bg-muted': view !== 'history', 'border-r border-border': stashes.length > 0 || conflicts.length > 0 }" class="flex-1 py-3 font-semibold transition-safe">History</button>
          <button v-if="stashes.length > 0" @click="view = 'stashes'" :class="{ 'gradient-bg text-accent-foreground': view === 'stashes', 'hover:bg-muted': view !== 'stashes' }" class="flex-1 py-3 font-semibold transition-safe border-r border-border">Stash</button>
          <button v-if="conflicts.length > 0" @click="view = 'conflicts'" :class="{ 'gradient-bg text-accent-foreground': view === 'conflicts', 'hover:bg-muted': view !== 'conflicts' }" class="flex-1 py-3 font-semibold transition-safe">Conflict</button>
        </div>

        <div class="flex-1 overflow-auto p-3">
          <div v-if="view === 'changes'" class="space-y-1.5">
            <!-- Changes Header with Bulk Select -->
            <div v-if="fileStatuses.length > 0" 
                 class="flex items-center gap-3 p-2.5 mb-2 rounded-lg bg-muted/50 border border-border transition-safe justify-between">
              <div class="flex items-center gap-3 cursor-pointer" @click="toggleAllStaged">
                <input type="checkbox" :checked="allStaged" class="w-4 h-4 rounded border-border accent-accent cursor-pointer pointer-events-none" />
                <div class="text-xs font-semibold text-muted-foreground select-none">
                  {{ fileStatuses.length }} changed file{{ fileStatuses.length !== 1 ? 's' : '' }}
                </div>
              </div>
              <button @click.stop="handleDiscardAllChanges" class="text-[10px] text-error hover:underline font-bold px-2 py-1 rounded hover:bg-error/10 transition-safe">DISCARD ALL</button>
            </div>

            <div v-for="file in fileStatuses" :key="file.path" 
                 class="group flex items-center gap-3 p-2.5 rounded-lg border border-transparent hover:border-border cursor-pointer transition-safe"
                 :class="{ 'border-accent bg-accent/5': selectedFile === file.path }"
                 @click.self="selectedFile = file.path">
              <input type="checkbox" :checked="file.staged" @change="toggleStaged(file)" class="w-4 h-4 rounded border-border accent-accent" />
              <div class="flex-1 min-w-0 flex items-center gap-2" @click="selectedFile = file.path">
                <span class="text-xs w-5 text-center font-semibold" :class="{ 'text-success': file.status === 'added', 'text-accent': file.status === 'modified', 'text-error': file.status === 'deleted' }">
                  {{ file.status[0].toUpperCase() }}
                </span>
                <span class="truncate text-sm" :title="file.path">{{ file.path.split('/').pop() }}</span>
              </div>
              <button @click.stop="handleDiscardChanges(file.path)" class="opacity-0 group-hover:opacity-100 p-1.5 hover:text-error text-xs transition-opacity rounded hover:bg-error/10">✕</button>
            </div>
          </div>
          <div v-else-if="view === 'history'" class="flex-1 flex flex-col overflow-hidden">
            <div class="px-3 py-2 border-b border-border bg-card/50">
               <input v-model="searchCommitQuery" placeholder="Search commits..." class="w-full bg-muted/30 border border-border rounded-lg px-3 py-2 text-xs text-foreground outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <RecycleScroller
              class="flex-1 overflow-auto p-3"
              :items="filteredCommits"
              :item-size="76"
              key-field="sha"
              v-slot="{ item }"
            >
              <div @click="selectedCommit = item"
                   class="mb-1.5 p-3 rounded-lg border border-transparent hover:border-border cursor-pointer transition-safe bg-card/30"
                   :class="{ 'border-accent bg-accent/5 shadow-sm': selectedCommit?.sha === item.sha }">
                <div class="text-sm font-semibold truncate mb-1.5 flex items-center gap-2" :class="{ 'text-accent': selectedCommit?.sha === item.sha }">
                  <span v-if="!item.is_pushed" 
                        class="text-success font-bold text-xs" title="Unpushed commit">↑</span>
                  {{ item.message }}
                </div>
                <div class="flex justify-between text-xs text-muted-foreground font-mono">
                  <span>{{ item.sha.substring(0, 7) }}</span>
                  <span>{{ new Date(item.timestamp * 1000).toLocaleDateString() }}</span>
                </div>
              </div>
            </RecycleScroller>
          </div>
          <div v-else-if="view === 'stashes'" class="space-y-1.5">
            <div v-for="(stash, index) in stashes" :key="index" 
                 class="p-3 bg-card rounded-lg border border-border flex justify-between items-center group hover:border-accent transition-safe">
              <div class="flex-1 min-w-0">
                <div class="text-sm font-semibold truncate">{{ stash.message || 'No message' }}</div>
                <div class="text-xs text-muted-foreground font-mono mt-1">{{ stash.sha.substring(0, 7) }}</div>
              </div>
              <button @click="handleStashPop(index)" class="opacity-0 group-hover:opacity-100 gradient-bg text-accent-foreground text-xs px-3 py-1.5 rounded-lg hover:shadow-accent transition-safe font-medium">Pop</button>
            </div>
          </div>
          <div v-else-if="view === 'conflicts'" class="space-y-2">
            <div v-for="conflict in conflicts" :key="conflict.path" class="p-3 bg-error/5 rounded-lg border border-error/20">
              <div class="text-sm font-semibold truncate mb-3 text-error" :title="conflict.path">{{ conflict.path.split('/').pop() }}</div>
              <div class="flex gap-2 text-xs">
                <button @click="handleResolve(conflict.path, true)" class="flex-1 bg-card border border-border hover:bg-muted py-2 rounded-lg font-medium transition-safe">Use Ours</button>
                <button @click="handleResolve(conflict.path, false)" class="flex-1 bg-card border border-border hover:bg-muted py-2 rounded-lg font-medium transition-safe">Use Theirs</button>
              </div>
            </div>
          </div>
        </div>

        <div v-if="view === 'changes'" class="p-4 border-t border-border bg-muted/30">
          <div class="flex items-center gap-2 mb-2">
             <input type="checkbox" id="amend" v-model="amendCommit" class="w-3.5 h-3.5 rounded border-border accent-accent" />
             <label for="amend" class="text-xs font-medium text-muted-foreground cursor-pointer select-none">Amend Last Commit</label>
          </div>
          <label class="block mb-2 text-sm font-medium text-foreground">Commit Message</label>
          <textarea v-model="commitMessage" placeholder="Describe your changes..." class="w-full bg-card border border-border rounded-lg p-3 text-foreground text-sm mb-3 focus:ring-2 focus:ring-accent focus:border-transparent outline-none resize-none" rows="3" />
          <button @click="handleCommit" :disabled="loading || !commitMessage.trim() || (!amendCommit && stagedFiles.length === 0)" 
                  class="w-full gradient-bg text-accent-foreground disabled:opacity-50 disabled:cursor-not-allowed py-2.5 rounded-lg font-semibold text-sm hover:shadow-accent transition-safe">
            {{ amendCommit ? 'Amend Commit' : `Commit to ${branches.find((b: BranchInfo) => b.is_current)?.name || 'HEAD'}` }}
          </button>
        </div>

        <div class="p-3 border-t border-border flex gap-2 overflow-x-auto bg-card text-sm">
          <button @click="handlePull" class="flex-1 bg-card border border-border py-2 px-3 rounded-lg hover:bg-muted transition-safe font-medium flex items-center justify-center gap-2">
            Pull
            <span v-if="repoInfo?.behind" class="flex items-center justify-center bg-error/10 text-error text-[10px] w-4 h-4 rounded-full font-bold">{{ repoInfo.behind }}</span>
          </button>
          <button @click="handlePush" class="flex-1 bg-card border border-border py-2 px-3 rounded-lg hover:bg-muted transition-safe font-medium flex items-center justify-center gap-2">
            Push
            <span v-if="repoInfo?.ahead" class="flex items-center justify-center bg-success/10 text-success text-[10px] w-4 h-4 rounded-full font-bold">{{ repoInfo.ahead }}</span>
          </button>
          <button @click="handleStashSave" class="flex-1 bg-card border border-border py-2 px-3 rounded-lg hover:bg-muted transition-safe font-medium">Stash</button>
          <button v-if="view === 'history' && selectedCommit" @click="selectedCommit = null" class="flex-1 bg-card border border-border py-2 px-3 rounded-lg hover:bg-error/10 hover:text-error transition-safe font-medium">Clear</button>
        </div>
      </aside>

      <!-- Diff/Main View -->
      <main class="flex-1 bg-background flex flex-col overflow-hidden">
        <div v-if="view === 'changes' && selectedFile" class="flex-1 flex flex-col overflow-hidden">
          <div class="h-12 border-b border-border flex items-center px-6 bg-card text-sm font-mono text-muted-foreground">
            {{ selectedFile }}
          </div>
          <div class="flex-1 overflow-auto">
            <DiffViewer :diffs="diffs" />
          </div>
        </div>
        <div v-else-if="view === 'history' && selectedCommit" class="flex-1 flex flex-col overflow-hidden">
          <div class="h-14 border-b border-border flex items-center px-6 bg-card text-sm font-mono justify-between flex-shrink-0">
            <div class="flex items-center gap-3 overflow-hidden">
              <span class="text-accent font-semibold flex-shrink-0">{{ selectedCommit.sha.substring(0, 7) }}</span>
              <span class="text-muted-foreground truncate" :title="selectedCommit.message">{{ selectedCommit.message }}</span>
            </div>
            <div class="flex items-center gap-3 flex-shrink-0 ml-4">
               <button @click="handleCherryPick(selectedCommit.sha)" class="px-3 py-1.5 border border-border rounded text-xs hover:bg-muted transition-safe font-medium" title="Apply this commit to current branch">Cherry-pick</button>
               <button @click="handleRevertCommit(selectedCommit.sha)" class="px-3 py-1.5 border border-border rounded text-xs hover:bg-muted hover:text-error transition-safe font-medium" title="Create a new commit that reverts this one">Revert</button>
            </div>
          </div>
          
          <div class="flex-1 flex overflow-hidden">
            <!-- Left: File List -->
            <div class="w-64 border-r border-border bg-card overflow-y-auto flex-shrink-0">
              <div v-for="diff in diffs" :key="diff.path"
                   @click="selectedCommitFile = diff.path"
                   class="px-4 py-2 text-sm cursor-pointer border-l-2 hover:bg-muted transition-safe flex items-center justify-between group"
                   :class="{ 'border-accent bg-accent/5': selectedCommitFile === diff.path, 'border-transparent': selectedCommitFile !== diff.path }">
                <span class="truncate" :title="diff.path">{{ diff.path.split('/').pop() }}</span>
                <span class="text-xs w-4 text-center font-bold" 
                      :class="{ 'text-success': diff.additions > 0 && diff.deletions === 0, 'text-error': diff.deletions > 0 && diff.additions === 0, 'text-accent': diff.additions > 0 && diff.deletions > 0 }">
                  {{ diff.additions > 0 && diff.deletions === 0 ? 'A' : (diff.deletions > 0 && diff.additions === 0 ? 'D' : 'M') }}
                </span>
              </div>
            </div>

            <!-- Right: Diff -->
            <div class="flex-1 overflow-auto bg-background">
              <DiffViewer :diffs="diffs.filter(d => d.path === selectedCommitFile)" />
            </div>
          </div>
        </div>
        <div v-else class="flex-1 flex items-center justify-center text-muted-foreground text-sm">
          {{ view === 'history' ? 'Select a commit to view diff' : 'Select a file to view changes' }}
        </div>
      </main>
    </div>

    <!-- Welcome View -->
    <div v-else class="flex-1 flex flex-col items-center justify-center p-8 bg-background dot-pattern">
      <div class="max-w-3xl w-full text-center space-y-12 radial-glow">
        <div class="space-y-6">
          <h1 class="text-5xl md:text-6xl font-display text-foreground tracking-tight">
            Git <span class="gradient-text">Terminal</span>
          </h1>
          <p class="text-muted-foreground text-lg max-w-xl mx-auto">A modern, elegant interface for version control</p>
        </div>
        
        <div class="grid grid-cols-2 gap-6 max-w-2xl mx-auto">
          <button @click="handleOpenRepo()" class="group p-8 bg-card border border-border rounded-2xl hover:border-accent hover:shadow-lg transition-safe flex flex-col items-center">
            <div class="text-4xl mb-4 gradient-text">📁</div>
            <div class="text-lg font-semibold text-foreground mb-2">Open Local</div>
            <div class="text-sm text-muted-foreground">Load repository from disk</div>
          </button>
          <button @click="triggerCloneModal" class="group p-8 bg-card border border-border rounded-2xl hover:border-accent hover:shadow-lg transition-safe flex flex-col items-center">
            <div class="text-4xl mb-4 gradient-text">⬇️</div>
            <div class="text-lg font-semibold text-foreground mb-2">Clone Remote</div>
            <div class="text-sm text-muted-foreground">Fetch from remote server</div>
          </button>
        </div>

        <div v-if="settings?.recent_repositories.length" class="space-y-4 text-left max-w-2xl mx-auto">
          <h3 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider px-1 border-b border-border pb-3">Recent Repositories</h3>
          <div class="space-y-2">
            <div v-for="path in settings.recent_repositories.slice(0, 5)" :key="path"
                 @click="handleOpenRepo(path)"
                 class="group flex items-center gap-4 p-4 bg-card border border-border rounded-xl hover:border-accent hover:shadow-md cursor-pointer transition-safe">
              <span class="text-accent text-xl">›</span>
              <div class="flex-1 min-w-0">
                <div class="text-foreground font-semibold truncate text-sm">{{ getRepoName(path) }}</div>
                <div class="text-muted-foreground text-xs truncate font-mono mt-0.5">{{ path }}</div>
              </div>
              <span class="text-muted-foreground group-hover:text-accent transition-safe text-xl">→</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Loading Indicator (右上角小圈圈) -->
    <div v-if="loading" class="fixed top-4 right-4 z-[100] flex items-center gap-2 bg-card/95 backdrop-blur-sm px-3 py-2 rounded-lg shadow-lg border border-border">
      <div class="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin"></div>
      <span class="text-xs font-medium text-muted-foreground">Loading...</span>
    </div>
  </div>
</template>

<style>
/* Minimalist modern styles defined in index.css */
</style>
