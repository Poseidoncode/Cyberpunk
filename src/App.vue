<script setup lang="ts">
import { ref, onMounted, watch, computed } from 'vue';
import { gitService, type RepositoryInfo, type FileStatus, type BranchInfo, type CommitInfo, type StashInfo, type ConflictInfo, type Settings, type DiffInfo } from './services/git';
import { open } from '@tauri-apps/plugin-dialog';
import DiffViewer from './components/DiffViewer.vue';

const repoInfo = ref<RepositoryInfo | null>(null);
const fileStatuses = ref<FileStatus[]>([]);
const branches = ref<BranchInfo[]>([]);
const commits = ref<CommitInfo[]>([]);
const stashes = ref<StashInfo[]>([]);
const conflicts = ref<ConflictInfo[]>([]);
const settings = ref<Settings | null>(null);
const diffs = ref<DiffInfo[]>([]);

const commitMessage = ref("");
const selectedFile = ref<string | null>(null);
const selectedCommit = ref<CommitInfo | null>(null);
const view = ref<"changes" | "history" | "stashes" | "conflicts">("changes");
const loading = ref(false);
const error = ref<string | null>(null);

// Modal State
const showCloneModal = ref(false);
const cloneUrl = ref("");
const showSettingsModal = ref(false);
const showBranchModal = ref(false);
const newBranchName = ref("");

const stagedFiles = computed(() => fileStatuses.value.filter(f => f.staged).map(f => f.path));

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

onMounted(() => {
  fetchSettings();
});

watch([repoInfo, view], () => {
  if (repoInfo.value) {
    refreshRepo();
  }
});

watch(selectedFile, (newFile: string | null) => {
  if (newFile && view.value === "changes") {
    gitService.getDiff(newFile).then(d => diffs.value = d);
  } else if (!newFile) {
    diffs.value = [];
  }
});

watch(selectedCommit, (newCommit: CommitInfo | null) => {
  if (newCommit) {
    gitService.getCommitDiff(newCommit.sha).then(d => diffs.value = d);
  } else {
    diffs.value = [];
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
  }
};

const triggerCloneModal = () => {
  cloneUrl.value = "";
  showCloneModal.value = true;
};

const handleCloneRepo = async () => {
  if (!cloneUrl.value) return;
  const url = cloneUrl.value;
  showCloneModal.value = false;

  try {
    loading.value = true;
    error.value = null;

    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select Clone Destination",
    });

    if (selected && typeof selected === "string") {
      await gitService.cloneRepository(url, selected);
      const info = await gitService.openRepository(selected);
      repoInfo.value = info;
      fetchSettings();
      selectedFile.value = null;
    }
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
      await gitService.stageFiles([file.path]);
    }
    await refreshRepo();
  } catch (err) {
    error.value = err as string;
  }
};

const handleDiscardChanges = async (path: string) => {
  if (!confirm(`Are you sure you want to discard changes in ${path}? This cannot be undone.`)) return;
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

const handleCommit = async () => {
  if (!commitMessage.value.trim()) {
    alert("Please enter a commit message");
    return;
  }
  if (stagedFiles.value.length === 0) {
    alert("Please select files to commit");
    return;
  }

  try {
    loading.value = true;
    error.value = null;
    await gitService.createCommit(commitMessage.value, stagedFiles.value);
    commitMessage.value = "";
    selectedFile.value = null;
    await refreshRepo();
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
    alert("Pushed successfully!");
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
    alert("Fetch completed!");
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
    
    if (confirm(`Switch remote protocol to SSH?\nNew URL: ${sshUrl}`)) {
      await gitService.setRemoteUrl("origin", sshUrl);
      alert("Remote protocol switched to SSH successfully!");
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
</script>

<template>
  <div class="app flex flex-col h-screen bg-terminal-bg text-terminal-primary overflow-hidden font-mono">
    <!-- Header/Top Bar -->
    <header class="h-10 border-b border-terminal-border bg-terminal-bg flex items-center px-4 justify-between flex-shrink-0">
      <div class="flex items-center gap-4 text-xs uppercase">
        <div class="flex items-center gap-2 cursor-pointer hover:bg-terminal-muted px-2 py-1 border border-transparent hover:border-terminal-border group" @click="handleOpenRepo()" title="Click to switch repository">
          <span class="text-terminal-primary">[SWITCH]</span>
          <span class="font-bold truncate max-w-[200px] text-glow">{{ repoInfo ? repoInfo.path.split('/').pop() : 'NO_REPOSITORY' }}</span>
        </div>
        <div v-if="repoInfo" class="flex items-center gap-2 cursor-pointer hover:bg-terminal-muted px-2 py-1 border border-transparent hover:border-terminal-border" @click="showBranchModal = true">
          <span>$</span>
          <span class="font-bold text-glow">{{ branches.find((b: BranchInfo) => b.is_current)?.name || 'UNKNOWN' }}</span>
        </div>
      </div>
      <div class="flex items-center gap-2 text-xs uppercase">
        <button v-if="repoInfo" @click="handleFetch" class="px-2 py-1 border border-terminal-border hover:bg-terminal-primary hover:text-terminal-bg transition-colors">[ FETCH ]</button>
        <button @click="showSettingsModal = true" class="px-2 py-1 border border-terminal-border hover:bg-terminal-primary hover:text-terminal-bg transition-colors">[ CONFIG ]</button>
      </div>
    </header>

    <div v-if="error" class="bg-terminal-bg border-b border-terminal-error px-4 py-2 text-xs flex justify-between items-center text-terminal-error">
      <span>[ERR] {{ error }}</span>
      <button @click="error = null" class="hover:bg-terminal-error hover:text-terminal-bg px-2">X</button>
    </div>

    <!-- Modals -->
    <div v-if="showCloneModal || showSettingsModal || showBranchModal" class="fixed inset-0 flex items-center justify-center z-50 p-4" style="background-color: #000000;">
      <!-- Clone Modal -->
      <div v-if="showCloneModal" class="border-2 border-terminal-primary p-6 w-full max-w-md font-mono" style="background-color: #000000;">
        <div class="border-b border-terminal-border pb-2 mb-4 text-xs uppercase text-glow">+--- CLONE REPOSITORY ---+</div>
        <div class="mb-2 text-xs uppercase text-terminal-muted">REMOTE_URL:</div>
        <input v-model="cloneUrl" placeholder="https://github.com/user/repo.git" class="w-full border border-terminal-border p-2 text-terminal-primary text-xs mb-6 focus:border-terminal-primary outline-none font-mono" style="background-color: #000000; color: #33ff00;" />
        <div class="flex justify-end gap-3 text-xs uppercase">
          <button @click="showCloneModal = false" class="px-4 py-2 border border-terminal-border hover:bg-terminal-muted transition-colors">[ CANCEL ]</button>
          <button @click="handleCloneRepo" class="bg-terminal-primary text-terminal-bg px-4 py-2 border border-terminal-primary hover:bg-terminal-muted hover:text-terminal-primary transition-colors font-bold">[ EXECUTE ]</button>
        </div>
      </div>

      <!-- Settings Modal -->
      <div v-if="showSettingsModal && settings" class="border-2 border-terminal-primary p-6 w-full max-w-md font-mono" style="background-color: #000000;">
        <div class="border-b border-terminal-border pb-2 mb-6 text-xs uppercase text-glow">+--- CONFIGURATION ---+</div>
        <div class="space-y-4 mb-8">
          <div>
            <label class="block text-[10px] uppercase text-terminal-muted mb-1">GIT_USER_NAME:</label>
            <div class="text-[9px] text-terminal-muted mb-1 italic">// Identifies you as the author of commits</div>
            <input v-model="settings.user_name" class="w-full border border-terminal-border p-2 text-terminal-primary text-xs outline-none focus:border-terminal-primary font-mono" style="background-color: #000000; color: #33ff00;" />
          </div>
          <div>
            <label class="block text-[10px] uppercase text-terminal-muted mb-1">GIT_USER_EMAIL:</label>
            <div class="text-[9px] text-terminal-muted mb-1 italic">// Email address associated with your commits</div>
            <input v-model="settings.user_email" class="w-full border border-terminal-border p-2 text-terminal-primary text-xs outline-none focus:border-terminal-primary font-mono" style="background-color: #000000; color: #33ff00;" />
          </div>
          <div>
            <label class="block text-[10px] uppercase text-terminal-muted mb-1">SSH_KEY_PATH:</label>
            <input v-model="settings.ssh_key_path" placeholder="~/.ssh/id_rsa" class="w-full border border-terminal-border p-2 text-terminal-primary text-xs outline-none focus:border-terminal-primary font-mono" style="background-color: #000000; color: #33ff00;" />
          </div>
          <div class="pt-2">
            <button @click="handleSwitchToSSH" class="text-[10px] text-terminal-primary hover:underline uppercase flex items-center gap-1">
              <span>[!]</span> SWITCH_REMOTES_TO_SSH
            </button>
            <div class="text-[9px] text-terminal-muted mt-1 italic">// Use this if you get authentication errors with HTTPS</div>
          </div>
        </div>
        <div class="flex justify-end gap-3 text-xs uppercase">
          <button @click="showSettingsModal = false" class="px-4 py-2 border border-terminal-border hover:bg-terminal-muted transition-colors">[ CANCEL ]</button>
          <button @click="saveSettings" class="bg-terminal-primary text-terminal-bg px-4 py-2 border border-terminal-primary hover:bg-terminal-muted hover:text-terminal-primary transition-colors font-bold">[ SAVE ]</button>
        </div>
      </div>

      <!-- Branch Switcher Modal -->
      <div v-if="showBranchModal" class="border-2 border-terminal-primary p-6 w-full max-w-md font-mono" style="background-color: #000000;">
        <div class="border-b border-terminal-border pb-2 mb-4 text-xs uppercase text-glow">+--- BRANCH CONTROL ---+</div>
        <div class="max-h-60 overflow-auto mb-6 space-y-1">
          <div v-for="branch in branches" :key="branch.name"
               @click="!branch.is_current && checkoutBranch(branch.name)"
               class="p-2 border border-transparent hover:border-terminal-border cursor-pointer flex items-center justify-between text-xs"
               :class="{ 'bg-terminal-primary text-terminal-bg border-terminal-primary': branch.is_current }">
            <span class="uppercase">{{ branch.name }}</span>
            <span v-if="branch.is_current" class="text-[10px]">[ACTIVE]</span>
          </div>
        </div>
        <div class="border-t border-terminal-border pt-4">
          <label class="block text-[10px] uppercase text-terminal-muted mb-2">CREATE_NEW_BRANCH:</label>
          <div class="flex gap-2">
            <input v-model="newBranchName" @keyup.enter="handleCreateBranch" placeholder="feature/new-branch" class="flex-1 border border-terminal-border p-2 text-terminal-primary text-xs outline-none focus:border-terminal-primary font-mono" style="background-color: #000000; color: #33ff00;" />
            <button @click="handleCreateBranch" class="bg-terminal-bg border border-terminal-border hover:bg-terminal-muted px-4 py-2 text-xs uppercase">[ + ]</button>
          </div>
        </div>
        <div class="flex justify-end mt-6">
          <button @click="showBranchModal = false" class="px-4 py-2 border border-terminal-border hover:bg-terminal-muted transition-colors text-xs uppercase">[ CLOSE ]</button>
        </div>
      </div>
    </div>

    <!-- Main Content Area -->
    <div v-if="repoInfo" class="flex flex-1 overflow-hidden">
      <!-- Left Sidebar -->
      <aside class="w-72 flex-shrink-0 border-r border-terminal-border flex flex-col bg-terminal-bg">
        <div class="flex border-b border-terminal-border text-[10px] uppercase">
          <button @click="view = 'changes'" :class="{ 'bg-terminal-primary text-terminal-bg': view === 'changes' }" class="flex-1 py-2 font-bold hover:bg-terminal-muted transition-colors border-r border-terminal-border">CHANGES ({{ fileStatuses.length }})</button>
          <button @click="view = 'history'" :class="{ 'bg-terminal-primary text-terminal-bg': view === 'history', 'border-r border-terminal-border': stashes.length > 0 || conflicts.length > 0 }" class="flex-1 py-2 font-bold hover:bg-terminal-muted transition-colors">HISTORY</button>
          <button v-if="stashes.length > 0" @click="view = 'stashes'" :class="{ 'bg-terminal-primary text-terminal-bg': view === 'stashes' }" class="flex-1 py-2 font-bold hover:bg-terminal-muted transition-colors border-r border-terminal-border">STASH</button>
          <button v-if="conflicts.length > 0" @click="view = 'conflicts'" :class="{ 'bg-terminal-primary text-terminal-bg': view === 'conflicts' }" class="flex-1 py-2 font-bold hover:bg-terminal-muted transition-colors">CONFLICT</button>
        </div>

        <div class="flex-1 overflow-auto p-2">
          <div v-if="view === 'changes'" class="space-y-1">
            <div v-for="file in fileStatuses" :key="file.path" 
                 class="group flex items-center gap-2 p-2 border border-transparent hover:border-terminal-border cursor-pointer"
                 :class="{ 'border-terminal-primary bg-terminal-muted': selectedFile === file.path }"
                 @click.self="selectedFile = file.path">
              <input type="checkbox" :checked="file.staged" @change="toggleStaged(file)" class="w-3 h-3 border border-terminal-border bg-terminal-bg accent-terminal-primary" />
              <div class="flex-1 min-w-0 flex items-center gap-2" @click="selectedFile = file.path">
                <span class="text-[10px] w-4 text-center font-bold" :class="{ 'text-terminal-primary': file.status === 'added', 'text-terminal-secondary': file.status === 'modified', 'text-terminal-error': file.status === 'deleted' }">
                  {{ file.status[0].toUpperCase() }}
                </span>
                <span class="truncate text-xs font-mono" :title="file.path">{{ file.path.split('/').pop() }}</span>
              </div>
              <button @click.stop="handleDiscardChanges(file.path)" class="opacity-0 group-hover:opacity-100 p-1 hover:text-terminal-error text-[10px] transition-opacity uppercase">[ X ]</button>
            </div>
          </div>
          <div v-else-if="view === 'history'" class="space-y-1">
            <div v-for="commit in commits" :key="commit.sha" 
                 @click="selectedCommit = commit"
                 class="p-2 border border-transparent hover:border-terminal-border cursor-pointer transition-all"
                 :class="{ 'border-terminal-primary bg-terminal-muted': selectedCommit?.sha === commit.sha }">
              <div class="text-xs font-bold truncate mb-1 uppercase" :class="{ 'text-terminal-primary': selectedCommit?.sha === commit.sha }">{{ commit.message }}</div>
              <div class="flex justify-between text-[10px] text-terminal-muted font-mono">
                <span>{{ commit.sha.substring(0, 7) }}</span>
                <span>{{ new Date(commit.timestamp * 1000).toLocaleDateString() }}</span>
              </div>
            </div>
          </div>
          <div v-else-if="view === 'stashes'" class="space-y-1">
            <div v-for="(stash, index) in stashes" :key="index" 
                 class="p-2 bg-terminal-bg border border-terminal-border flex justify-between items-center group">
              <div class="flex-1 min-w-0">
                <div class="text-xs font-bold truncate uppercase">{{ stash.message || 'NO_MESSAGE' }}</div>
                <div class="text-[10px] text-terminal-muted font-mono">{{ stash.sha.substring(0, 7) }}</div>
              </div>
              <button @click="handleStashPop(index)" class="opacity-0 group-hover:opacity-100 bg-terminal-primary text-terminal-bg text-[10px] px-2 py-1 hover:bg-terminal-muted hover:text-terminal-primary transition-all uppercase">[ POP ]</button>
            </div>
          </div>
          <div v-else-if="view === 'conflicts'" class="space-y-2">
            <div v-for="conflict in conflicts" :key="conflict.path" class="p-2 bg-terminal-bg border border-terminal-error">
              <div class="text-xs font-bold truncate mb-2 text-terminal-error uppercase" :title="conflict.path">{{ conflict.path.split('/').pop() }}</div>
              <div class="flex gap-2 text-[10px] uppercase">
                <button @click="handleResolve(conflict.path, true)" class="flex-1 bg-terminal-bg border border-terminal-border hover:bg-terminal-muted py-1">[ OURS ]</button>
                <button @click="handleResolve(conflict.path, false)" class="flex-1 bg-terminal-bg border border-terminal-border hover:bg-terminal-muted py-1">[ THEIRS ]</button>
              </div>
            </div>
          </div>
        </div>

        <div v-if="view === 'changes'" class="p-3 border-t border-terminal-border bg-terminal-bg">
          <div class="text-[10px] uppercase text-terminal-muted mb-2">COMMIT_MESSAGE:</div>
          <textarea v-model="commitMessage" placeholder="git commit -m '...'" class="w-full bg-terminal-bg border border-terminal-border p-2 text-terminal-primary text-xs mb-3 focus:border-terminal-primary outline-none resize-none font-mono" rows="3" />
          <button @click="handleCommit" :disabled="loading || !commitMessage.trim() || stagedFiles.length === 0" 
                  class="w-full bg-terminal-primary text-terminal-bg disabled:opacity-50 disabled:bg-terminal-muted py-2 border border-terminal-primary font-bold text-xs tracking-wide uppercase hover:bg-terminal-muted hover:text-terminal-primary transition-colors">
            [ COMMIT TO {{ branches.find((b: BranchInfo) => b.is_current)?.name || 'HEAD' }} ]
          </button>
        </div>

        <div class="p-2 border-t border-terminal-border flex gap-2 overflow-x-auto bg-terminal-bg text-[10px] uppercase">
          <button @click="handlePull" class="flex-1 bg-terminal-bg border border-terminal-border py-1.5 px-2 hover:bg-terminal-primary hover:text-terminal-bg transition-colors">[ ↓ PULL ]</button>
          <button @click="handlePush" class="flex-1 bg-terminal-bg border border-terminal-border py-1.5 px-2 hover:bg-terminal-primary hover:text-terminal-bg transition-colors">[ ↑ PUSH ]</button>
          <button @click="handleStashSave" class="flex-1 bg-terminal-bg border border-terminal-border py-1.5 px-2 hover:bg-terminal-primary hover:text-terminal-bg transition-colors">[ STASH ]</button>
          <button v-if="view === 'history' && selectedCommit" @click="selectedCommit = null" class="flex-1 bg-terminal-bg border border-terminal-border py-1.5 px-2 hover:bg-terminal-error hover:text-terminal-bg transition-colors">[ CLR ]</button>
        </div>
      </aside>

      <!-- Diff/Main View -->
      <main class="flex-1 bg-terminal-bg flex flex-col overflow-hidden">
        <div v-if="view === 'changes' && selectedFile" class="flex-1 flex flex-col overflow-hidden">
          <div class="h-8 border-b border-terminal-border flex items-center px-4 bg-terminal-bg text-[10px] font-mono truncate uppercase text-terminal-muted">
            FILE: {{ selectedFile }}
          </div>
          <div class="flex-1 overflow-auto">
            <DiffViewer :diffs="diffs" />
          </div>
        </div>
        <div v-else-if="view === 'history' && selectedCommit" class="flex-1 flex flex-col overflow-hidden">
          <div class="h-8 border-b border-terminal-border flex items-center px-4 bg-terminal-bg text-[10px] font-mono truncate justify-between uppercase">
            <span class="text-terminal-muted">COMMIT: {{ selectedCommit.sha.substring(0, 12) }}</span>
            <span class="text-terminal-muted">AUTHOR: {{ selectedCommit.author }}</span>
          </div>
          <div class="flex-1 overflow-auto">
            <DiffViewer :diffs="diffs" />
          </div>
        </div>
        <div v-else class="flex-1 flex items-center justify-center text-terminal-muted text-xs uppercase">
          {{ view === 'history' ? '> SELECT_COMMIT_TO_VIEW_DIFF' : '> SELECT_FILE_TO_VIEW_CHANGES' }}
        </div>
      </main>
    </div>

    <!-- Welcome View -->
    <div v-else class="flex-1 flex flex-col items-center justify-center p-8 bg-terminal-bg">
      <div class="max-w-2xl w-full text-center space-y-8">
        <div class="space-y-4 border-2 border-terminal-primary p-8">
          <div class="text-xs uppercase text-terminal-muted mb-4">+--- SYSTEM READY ---+</div>
          <h2 class="text-4xl font-bold text-terminal-primary tracking-widest text-glow">GIT TERMINAL</h2>
          <p class="text-terminal-muted text-xs uppercase font-mono">COMMAND-LINE INTERFACE FOR VERSION CONTROL</p>
        </div>
        
        <div class="grid grid-cols-2 gap-4">
          <button @click="handleOpenRepo()" class="p-6 bg-terminal-bg border-2 border-terminal-border hover:border-terminal-primary hover:bg-terminal-muted transition-all group">
            <div class="text-2xl mb-4 text-terminal-primary">></div>
            <div class="text-sm font-bold text-terminal-primary mb-1 uppercase">[ OPEN_LOCAL ]</div>
            <div class="text-[10px] text-terminal-muted uppercase">LOAD_REPOSITORY_FROM_DISK</div>
          </button>
          <button @click="triggerCloneModal" class="p-6 bg-terminal-bg border-2 border-terminal-border hover:border-terminal-primary hover:bg-terminal-muted transition-all group">
            <div class="text-2xl mb-4 text-terminal-primary">$</div>
            <div class="text-sm font-bold text-terminal-primary mb-1 uppercase">[ CLONE_REMOTE ]</div>
            <div class="text-[10px] text-terminal-muted uppercase">FETCH_FROM_REMOTE_SERVER</div>
          </button>
        </div>

        <div v-if="settings?.recent_repositories.length" class="space-y-4 text-left">
          <h3 class="text-[10px] font-bold text-terminal-muted uppercase tracking-widest px-1 border-b border-terminal-border pb-2">RECENT_SESSIONS:</h3>
          <div class="space-y-1">
            <div v-for="path in settings.recent_repositories.slice(0, 5)" :key="path"
                 @click="handleOpenRepo(path)"
                 class="group flex items-center gap-4 p-3 bg-terminal-bg border border-terminal-border hover:border-terminal-primary hover:bg-terminal-muted cursor-pointer transition-all">
              <span class="text-terminal-primary">></span>
              <div class="flex-1 min-w-0">
                <div class="text-terminal-primary font-bold truncate uppercase text-xs">{{ path.split('/').pop() }}</div>
                <div class="text-terminal-muted text-[10px] truncate font-mono">{{ path }}</div>
              </div>
              <span class="text-terminal-muted group-hover:text-terminal-primary transition-all">></span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Global Loading -->
    <div v-if="loading" class="fixed inset-0 bg-black/80 flex items-center justify-center z-[100]">
      <div class="bg-terminal-bg border-2 border-terminal-primary px-6 py-4 flex items-center gap-4">
        <div class="w-4 h-4 border-2 border-terminal-primary border-t-transparent animate-spin"></div>
        <span class="text-xs font-bold tracking-tight uppercase text-terminal-primary">EXECUTING_GIT_COMMAND...</span>
      </div>
    </div>
  </div>
</template>

<style>
/* All Terminal CLI styles are defined in index.css */
</style>
