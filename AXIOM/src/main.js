/**
 * AXIOM Browser - Frontend Application
 *
 * Per PRD Section 7: "Rust owns all state. WebView is stateless."
 * This UI layer only renders state received from the Rust core.
 */

const invoke = window?.__TAURI__?.core?.invoke;

// ============================================
// State
// ============================================

let currentTabs = [];
let currentSession = null;
let activeTabId = null;
let lastBounds = null;
let boundsSyncHandle = null;
let currentTheme = null;
let currentBookmarks = [];
let currentDownloads = [];
const downloadStateById = new Map();
const tabLastInteractionAt = new Map();
let tabHousekeepingHandle = null;
let readerTypography = { fontSize: 18, maxWidth: 760 };
let readerSourceTabId = null;
const TAB_IDLE_FREEZE_MS = 5 * 60 * 1000;
const TAB_IDLE_DISCARD_MS = 30 * 60 * 1000;
let bookmarksBarVisible = true;
let selectedBookmarkUrl = null;
let draggingTabId = null;
let draggingDidDrop = false;

async function invokeCommand(command, args) {
  const result = await invoke(command, args);

  if (!result || typeof result !== 'object' || !('success' in result)) {
    return result;
  }

  if (!result.success) {
    throw new Error(result.error || `Command failed: ${command}`);
  }

  return result.data;
}

const SEARCH_ENGINES = {
  google: 'https://www.google.com/search?q=%s',
  bing: 'https://www.bing.com/search?q=%s',
  duckduckgo: 'https://duckduckgo.com/?q=%s',
};

const RELOAD_ICON_SVG = `
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
    <path d="M13.5 8a5.5 5.5 0 11-1.5-3.8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
    <path d="M13 2v3h-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
  </svg>
`;

const STOP_ICON_SVG = `
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
    <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
  </svg>
`;
// Note: Webviews are managed by the Rust backend, not frontend

// ============================================
// DOM Elements
// ============================================

const elements = {
  sessionName: null,
  tabList: null,
  addressBar: null,
  addressSuggestions: null,
  backBtn: null,
  forwardBtn: null,
  reloadBtn: null,
  readerBtn: null,
  newTabBtn: null,
  sessionsBtn: null,
  historyBtn: null,
  downloadsBtn: null,
  downloadsModal: null,
  downloadsList: null,
  settingsBtn: null,
  themeToggleBtn: null,
  toastContainer: null,
  sessionModal: null,
  sessionList: null,
  newSessionName: null,
  createSessionBtn: null,
  historyModal: null,
  historyList: null,
  historySearch: null,
  historyClearRange: null,
  historyClearStart: null,
  historyClearEnd: null,
  clearHistoryBtn: null,
  settingsModal: null,
  searchEngineSelect: null,
  manageBookmarksBtn: null,
  bookmarksModal: null,
  bookmarksSearch: null,
  bookmarksFolderFilter: null,
  bookmarksNewBtn: null,
  bookmarksImportFile: null,
  bookmarksExportBtn: null,
  bookmarksManagerList: null,
  bookmarksEditor: null,
  bookmarkEditTitle: null,
  bookmarkEditUrl: null,
  bookmarkEditFolder: null,
  bookmarkDeleteBtn: null,
  bookmarksBar: null,
  bookmarksList: null,
  addBookmarkBtn: null,
  bookmarksToggle: null,
  autofillToggle: null,
  autofillName: null,
  autofillEmail: null,
  passwordSaveToggle: null,
  emptyState: null,
  webviewContainer: null,
  tabPlaceholder: null,
  tabPlaceholderTitle: null,
  tabPlaceholderUrl: null,
  tabPlaceholderRestoreBtn: null,
  readerOverlay: null,
  readerDomain: null,
  readerTitle: null,
  readerContent: null,
  readerFontDecreaseBtn: null,
  readerFontIncreaseBtn: null,
  readerWidthDecreaseBtn: null,
  readerWidthIncreaseBtn: null,
  readerCloseBtn: null,
};

// ============================================
// Initialization
// ============================================

window.addEventListener('DOMContentLoaded', async () => {
  if (typeof invoke !== 'function') {
    showFatalError(
      'Tauri IPC unavailable. The UI cannot talk to the Rust core.',
      'Verify `app.withGlobalTauri` is enabled and the window/webview has a matching capability.'
    );
    return;
  }

  // Cache DOM elements
  elements.sessionName = document.getElementById('session-name');
  elements.tabList = document.getElementById('tab-list');
  elements.addressBar = document.getElementById('address-bar');
  elements.addressSuggestions = document.getElementById('address-suggestions');
  elements.backBtn = document.getElementById('back-btn');
  elements.forwardBtn = document.getElementById('forward-btn');
  elements.reloadBtn = document.getElementById('reload-btn');
  elements.readerBtn = document.getElementById('reader-btn');
  elements.newTabBtn = document.getElementById('new-tab-btn');
  elements.sessionsBtn = document.getElementById('sessions-btn');
  elements.historyBtn = document.getElementById('history-btn');
  elements.downloadsBtn = document.getElementById('downloads-btn');
  elements.downloadsModal = document.getElementById('downloads-modal');
  elements.downloadsList = document.getElementById('downloads-list');
  elements.settingsBtn = document.getElementById('settings-btn');
  elements.themeToggleBtn = document.getElementById('theme-toggle-btn');        
  elements.toastContainer = document.getElementById('toast-container');
  elements.sessionModal = document.getElementById('session-modal');
  elements.sessionList = document.getElementById('session-list');
  elements.newSessionName = document.getElementById('new-session-name');        
  elements.createSessionBtn = document.getElementById('create-session-btn');
  elements.historyModal = document.getElementById('history-modal');
  elements.historyList = document.getElementById('history-list');
  elements.historySearch = document.getElementById('history-search');
  elements.historyClearRange = document.getElementById('history-clear-range');
  elements.historyClearStart = document.getElementById('history-clear-start');
  elements.historyClearEnd = document.getElementById('history-clear-end');
  elements.clearHistoryBtn = document.getElementById('clear-history-btn');
  elements.settingsModal = document.getElementById('settings-modal');
  elements.searchEngineSelect = document.getElementById('search-engine-select');
  elements.manageBookmarksBtn = document.getElementById('manage-bookmarks-btn');
  elements.bookmarksModal = document.getElementById('bookmarks-modal');
  elements.bookmarksSearch = document.getElementById('bookmarks-search');
  elements.bookmarksFolderFilter = document.getElementById('bookmarks-folder-filter');
  elements.bookmarksNewBtn = document.getElementById('bookmarks-new-btn');
  elements.bookmarksImportFile = document.getElementById('bookmarks-import-file');
  elements.bookmarksExportBtn = document.getElementById('bookmarks-export-btn');
  elements.bookmarksManagerList = document.getElementById('bookmarks-manager-list');
  elements.bookmarksEditor = document.getElementById('bookmarks-editor');
  elements.bookmarkEditTitle = document.getElementById('bookmark-edit-title');
  elements.bookmarkEditUrl = document.getElementById('bookmark-edit-url');
  elements.bookmarkEditFolder = document.getElementById('bookmark-edit-folder');
  elements.bookmarkDeleteBtn = document.getElementById('bookmark-delete-btn');
  elements.bookmarksBar = document.getElementById('bookmarks-bar');
  elements.bookmarksList = document.getElementById('bookmarks-list');
  elements.addBookmarkBtn = document.getElementById('add-bookmark-btn');
  elements.bookmarksToggle = document.getElementById('bookmarks-bar-toggle');
  elements.autofillToggle = document.getElementById('autofill-toggle');
  elements.autofillName = document.getElementById('autofill-name');
  elements.autofillEmail = document.getElementById('autofill-email');
  elements.passwordSaveToggle = document.getElementById('password-save-toggle');
  elements.emptyState = document.getElementById('empty-state');
  elements.webviewContainer = document.getElementById('webview-container');
  elements.tabPlaceholder = document.getElementById('tab-placeholder');
  elements.tabPlaceholderTitle = document.getElementById('tab-placeholder-title');
  elements.tabPlaceholderUrl = document.getElementById('tab-placeholder-url');
  elements.tabPlaceholderRestoreBtn = document.getElementById('tab-placeholder-restore');
  elements.readerOverlay = document.getElementById('reader-overlay');
  elements.readerDomain = document.getElementById('reader-domain');
  elements.readerTitle = document.getElementById('reader-title');
  elements.readerContent = document.getElementById('reader-content');
  elements.readerFontDecreaseBtn = document.getElementById('reader-font-decrease');
  elements.readerFontIncreaseBtn = document.getElementById('reader-font-increase');
  elements.readerWidthDecreaseBtn = document.getElementById('reader-width-decrease');
  elements.readerWidthIncreaseBtn = document.getElementById('reader-width-increase');
  elements.readerCloseBtn = document.getElementById('reader-close-btn');

  try {
    await invoke('frontend_ready');
  } catch (error) {
    showFatalError('Failed to initialize IPC with Rust core.', String(error));
    return;
  }

  const listen = window?.__TAURI__?.event?.listen;
  if (typeof listen === 'function') {
    try {
      await listen('tabs-updated', () => refreshTabs());
      await listen('download-updated', (event) => handleDownloadUpdated(event.payload));
      await listen('new-window-requested', (event) => handleNewWindowRequested(event.payload));
      await listen('navigation-blocked', (event) => {
        const url = typeof event.payload === 'string' ? event.payload : String(event.payload || '');
        if (!url) return;
        showToast({ title: 'Blocked by tracking protection', message: url, timeout: 6000 });
      });
    } catch (error) {
      console.warn('Failed to listen for tab updates:', error);
    }
  }

  // Setup event listeners
  setupEventListeners();

  // Sync native webview bounds before creating content webviews
  scheduleBoundsSync();

  // Load settings and theme
  await loadSettings();
  loadReaderTypography();
  applyReaderTypography();
  refreshFilterListsInBackground();

  // Load initial state
  await loadInitialState();
  startTabHousekeeping();
});

async function loadInitialState() {
  try {
    // Load active session
    const sessionResult = await invoke('get_active_session');
    if (sessionResult.success) {
      currentSession = sessionResult.data;
      updateSessionDisplay();
    }

    // Load tabs
    await refreshTabs();
  } catch (error) {
    console.error('Failed to load initial state:', error);
  }
}

// ============================================
// Event Listeners
// ============================================

function setupEventListeners() {
  // Navigation buttons
  elements.backBtn.addEventListener('click', navigateBack);
  elements.forwardBtn.addEventListener('click', navigateForward);
  elements.reloadBtn.addEventListener('click', handleReloadButton);
  if (elements.readerBtn) {
    elements.readerBtn.addEventListener('click', toggleReaderMode);
  }

  // New tab button
  elements.newTabBtn.addEventListener('click', createNewTab);

  // Theme toggle
  elements.themeToggleBtn.addEventListener('click', toggleTheme);

  // Address bar
  elements.addressBar.addEventListener('keydown', handleAddressBarKeydown);
  elements.addressBar.addEventListener('input', handleAddressBarInput);
  elements.addressBar.addEventListener('focus', handleAddressBarFocus);
  elements.addressBar.addEventListener('blur', handleAddressBarBlur);

  // Sessions button
  elements.sessionsBtn.addEventListener('click', openSessionModal);

  // History button
  if (elements.historyBtn) {
    elements.historyBtn.addEventListener('click', () => openHistoryModal());
  }

  // Downloads button
  if (elements.downloadsBtn) {
    elements.downloadsBtn.addEventListener('click', () => openDownloadsModal());
  }

  // Settings button
  elements.settingsBtn.addEventListener('click', openSettingsModal);

  // Session modal
  elements.sessionModal.querySelector('.modal-backdrop').addEventListener('click', closeSessionModal);
  elements.sessionModal.querySelector('.modal-close').addEventListener('click', closeSessionModal);
  elements.createSessionBtn.addEventListener('click', createNewSession);
  elements.newSessionName.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') createNewSession();
  });

  // History modal
  if (elements.historyModal) {
    elements.historyModal.querySelector('.modal-backdrop').addEventListener('click', closeHistoryModal);
    elements.historyModal.querySelector('.modal-close').addEventListener('click', closeHistoryModal);
    elements.historySearch.addEventListener('input', handleHistorySearchInput);
    elements.historyClearRange.addEventListener('change', handleHistoryClearRangeChange);
    elements.clearHistoryBtn.addEventListener('click', clearHistoryFromUI);
  }

  // Downloads modal
  if (elements.downloadsModal) {
    elements.downloadsModal.querySelector('.modal-backdrop').addEventListener('click', closeDownloadsModal);
    elements.downloadsModal.querySelector('.modal-close').addEventListener('click', closeDownloadsModal);
  }

  // Settings modal
  elements.settingsModal.querySelector('.modal-backdrop').addEventListener('click', closeSettingsModal);
  elements.settingsModal.querySelector('.modal-close').addEventListener('click', closeSettingsModal);
  elements.searchEngineSelect.addEventListener('change', handleSearchEngineChange);
  elements.bookmarksToggle.addEventListener('change', handleBookmarksToggle);
  if (elements.autofillToggle) {
    elements.autofillToggle.addEventListener('change', handleAutofillToggle);
  }
  if (elements.autofillName) {
    elements.autofillName.addEventListener('change', handleAutofillProfileChange);
  }
  if (elements.autofillEmail) {
    elements.autofillEmail.addEventListener('change', handleAutofillProfileChange);
  }
  if (elements.passwordSaveToggle) {
    elements.passwordSaveToggle.addEventListener('change', handlePasswordSaveToggle);
  }
  elements.addBookmarkBtn.addEventListener('click', addBookmarkFromActiveTab);
  if (elements.manageBookmarksBtn) {
    elements.manageBookmarksBtn.addEventListener('click', openBookmarksModal);
  }

  if (elements.bookmarksModal) {
    elements.bookmarksModal.querySelector('.modal-backdrop').addEventListener('click', closeBookmarksModal);
    elements.bookmarksModal.querySelector('.modal-close').addEventListener('click', closeBookmarksModal);
    elements.bookmarksSearch.addEventListener('input', renderBookmarksManager);
    elements.bookmarksFolderFilter.addEventListener('change', renderBookmarksManager);
    elements.bookmarksNewBtn.addEventListener('click', startNewBookmark);
    elements.bookmarksExportBtn.addEventListener('click', exportBookmarksHtml);
    elements.bookmarksImportFile.addEventListener('change', importBookmarksFromFile);
    elements.bookmarksEditor.addEventListener('submit', saveBookmarkEdits);
    elements.bookmarkDeleteBtn.addEventListener('click', deleteSelectedBookmark);
  }

  // Keyboard shortcuts
  document.addEventListener('keydown', handleGlobalKeydown);

  // Keep native webviews aligned with layout
  window.addEventListener('resize', scheduleBoundsSync);

  // Tab drag-and-drop reorder
  elements.tabList.addEventListener('dragover', (e) => e.preventDefault());     
  elements.tabList.addEventListener('drop', handleTabListDrop);

  if (elements.tabPlaceholderRestoreBtn) {
    elements.tabPlaceholderRestoreBtn.addEventListener('click', restoreDiscardedActiveTab);
  }

  if (elements.readerCloseBtn) {
    elements.readerCloseBtn.addEventListener('click', closeReaderMode);
  }
  if (elements.readerFontDecreaseBtn) {
    elements.readerFontDecreaseBtn.addEventListener('click', () => adjustReaderTypography({ fontSizeDelta: -2 }));
  }
  if (elements.readerFontIncreaseBtn) {
    elements.readerFontIncreaseBtn.addEventListener('click', () => adjustReaderTypography({ fontSizeDelta: 2 }));
  }
  if (elements.readerWidthDecreaseBtn) {
    elements.readerWidthDecreaseBtn.addEventListener('click', () => adjustReaderTypography({ widthDelta: -60 }));
  }
  if (elements.readerWidthIncreaseBtn) {
    elements.readerWidthIncreaseBtn.addEventListener('click', () => adjustReaderTypography({ widthDelta: 60 }));
  }
}

function handleGlobalKeydown(e) {
  const key = (e.key || '').toLowerCase();
  const accel = e.ctrlKey || e.metaKey;
  const target = e.target;
  const isEditable =
    target
    && (target.tagName === 'INPUT'
      || target.tagName === 'TEXTAREA'
      || target.isContentEditable);

  if (accel && key === 't') {
    e.preventDefault();
    createNewTab();
    return;
  }

  if (accel && !e.shiftKey && key === 'n') {
    e.preventDefault();
    createNewWindow();
    return;
  }

  if (accel && e.shiftKey && key === 't') {
    e.preventDefault();
    restoreLastClosedTab();
    return;
  }

  if (accel && key === 'w') {
    e.preventDefault();
    if (activeTabId) closeTab(activeTabId);
    return;
  }

  if (accel && !e.shiftKey && key === 'r') {
    e.preventDefault();
    reloadPage();
    return;
  }

  if (accel && e.shiftKey && key === 'r') {
    e.preventDefault();
    forceReloadPage();
    return;
  }

  if (accel && key === 'l') {
    e.preventDefault();
    elements.addressBar.focus();
    elements.addressBar.select();
    return;
  }

  if (accel && key === 'j') {
    e.preventDefault();
    openDownloadsModal();
    return;
  }

  if (accel && e.altKey && key === 'r') {
    e.preventDefault();
    toggleReaderMode();
    return;
  }

  if (e.ctrlKey && key === 'h') {
    e.preventDefault();
    openHistoryModal();
    return;
  }

  if (accel && key === 'tab') {
    e.preventDefault();
    cycleTabs(e.shiftKey ? -1 : 1);
    return;
  }

  if (e.altKey && key === 'arrowleft') {
    e.preventDefault();
    navigateBack();
    return;
  }

  if (e.altKey && key === 'arrowright') {
    e.preventDefault();
    navigateForward();
    return;
  }

  if (isEditable) return;

  if (key === 'f11') {
    e.preventDefault();
    invokeCommand('toggle_fullscreen').catch((error) => {
      console.warn('Failed to toggle fullscreen:', error);
    });
    return;
  }

  if (key === 'escape') {
    if (isReaderModeOpen()) {
      closeReaderMode();
      return;
    }

    const hadModal =
      (elements.sessionModal && !elements.sessionModal.classList.contains('hidden'))
      || (elements.historyModal && !elements.historyModal.classList.contains('hidden'))
      || (elements.downloadsModal && !elements.downloadsModal.classList.contains('hidden'))
      || (elements.settingsModal && !elements.settingsModal.classList.contains('hidden'))
      || (elements.bookmarksModal && !elements.bookmarksModal.classList.contains('hidden'));

    closeSessionModal();
    closeHistoryModal();
    closeDownloadsModal();
    closeSettingsModal();
    closeBookmarksModal();
    elements.addressSuggestions.classList.add('hidden');

    if (!hadModal) {
      stopLoading();
    }
  }
}

// ============================================
// Settings & Theme
// ============================================

async function loadSettings() {
  try {
    const result = await invoke('get_settings');
    if (result.success) {
      const {
        search_engine: template,
        theme,
        bookmarks_bar_visible: barVisible,
        autofill_enabled: autofillEnabled,
        autofill_name: autofillName,
        autofill_email: autofillEmail,
        password_save_prompt_enabled: passwordSaveEnabled,
      } = result.data;
      const engineId = getEngineIdFromTemplate(template);
      elements.searchEngineSelect.value = engineId;

      const initialTheme = theme || getSystemTheme();
      applyTheme(initialTheme);
      const resolvedVisible = typeof barVisible === 'boolean' ? barVisible : true;
      elements.bookmarksToggle.checked = resolvedVisible;
      applyBookmarksVisibility(resolvedVisible);
      if (elements.autofillToggle) {
        elements.autofillToggle.checked = typeof autofillEnabled === 'boolean' ? autofillEnabled : true;
      }
      if (elements.autofillName) {
        elements.autofillName.value = typeof autofillName === 'string' ? autofillName : '';
      }
      if (elements.autofillEmail) {
        elements.autofillEmail.value = typeof autofillEmail === 'string' ? autofillEmail : '';
      }
      if (elements.passwordSaveToggle) {
        elements.passwordSaveToggle.checked = typeof passwordSaveEnabled === 'boolean' ? passwordSaveEnabled : false;
      }
      await loadBookmarks();
      return;
    }
  } catch (error) {
    console.error('Failed to load settings:', error);
  }

  elements.searchEngineSelect.value = 'duckduckgo';
  applyTheme(getSystemTheme());
  elements.bookmarksToggle.checked = true;
  applyBookmarksVisibility(true);
  if (elements.autofillToggle) {
    elements.autofillToggle.checked = true;
  }
  if (elements.autofillName) {
    elements.autofillName.value = '';
  }
  if (elements.autofillEmail) {
    elements.autofillEmail.value = '';
  }
  if (elements.passwordSaveToggle) {
    elements.passwordSaveToggle.checked = false;
  }
  await loadBookmarks();
}

function refreshFilterListsInBackground(force = false) {
  invokeCommand('refresh_filter_lists', { force })
    .then((status) => {
      if (!status || status.updated !== true) return;
      showToast({
        title: 'Tracking lists updated',
        message: `${status.blocked_domains || 0} blocked domains loaded`,
        timeout: 4000,
      });
    })
    .catch((error) => {
      console.warn('Failed to refresh filter lists:', error);
    });
}

function getSystemTheme() {
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

function applyTheme(theme) {
  currentTheme = theme;
  document.documentElement.dataset.theme = theme;
  updateThemeToggleIcon(theme);
}

function updateThemeToggleIcon(theme) {
  if (!elements.themeToggleBtn) return;

  if (theme === 'light') {
    elements.themeToggleBtn.innerHTML = `
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
        <path d="M9.5 1.5a5.5 5.5 0 10 5 8.3A6.5 6.5 0 019.5 1.5z" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
      </svg>
    `;
    elements.themeToggleBtn.title = 'Switch to dark theme';
  } else {
    elements.themeToggleBtn.innerHTML = `
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
        <path d="M8 2.5V1M8 15v-1.5M3.05 3.05l-.9-.9M13.85 13.85l-.9-.9M1 8h1.5M13.5 8H15M3.05 12.95l-.9.9M13.85 2.15l-.9.9" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
        <circle cx="8" cy="8" r="3.5" stroke="currentColor" stroke-width="1.3"/>
      </svg>
    `;
    elements.themeToggleBtn.title = 'Switch to light theme';
  }
}

async function toggleTheme() {
  const nextTheme = currentTheme === 'light' ? 'dark' : 'light';
  applyTheme(nextTheme);

  try {
    await invoke('set_theme', { theme: nextTheme });
  } catch (error) {
    console.error('Failed to persist theme:', error);
  }
}

function getEngineIdFromTemplate(template) {
  const entry = Object.entries(SEARCH_ENGINES).find(([, value]) => value === template);
  return entry ? entry[0] : 'duckduckgo';
}

async function handleSearchEngineChange() {
  const engine = elements.searchEngineSelect.value;

  try {
    await invoke('set_search_engine', { engine });
  } catch (error) {
    console.error('Failed to update search engine:', error);
  }
}

async function handleAutofillToggle() {
  if (!elements.autofillToggle) return;
  const enabled = Boolean(elements.autofillToggle.checked);
  try {
    await invoke('set_autofill_enabled', { enabled });
  } catch (error) {
    console.error('Failed to persist autofill setting:', error);
  }
}

async function handleAutofillProfileChange() {
  const name = elements.autofillName ? elements.autofillName.value.trim() : '';
  const email = elements.autofillEmail ? elements.autofillEmail.value.trim() : '';
  try {
    await invoke('set_autofill_profile', {
      name: name || null,
      email: email || null,
    });
  } catch (error) {
    console.error('Failed to persist autofill profile:', error);
  }
}

async function handlePasswordSaveToggle() {
  if (!elements.passwordSaveToggle) return;
  const enabled = Boolean(elements.passwordSaveToggle.checked);
  try {
    await invoke('set_password_save_prompt_enabled', { enabled });
  } catch (error) {
    console.error('Failed to persist password prompt setting:', error);
  }
}

function openSettingsModal() {
  elements.settingsModal.classList.remove('hidden');
  elements.searchEngineSelect.focus();
}

function closeSettingsModal() {
  elements.settingsModal.classList.add('hidden');
}

function updateNavigationButtons() {
  const hasActiveTab = Boolean(activeTabId);
  const activeTab = getActiveTab();
  elements.backBtn.disabled = !hasActiveTab;
  elements.forwardBtn.disabled = !hasActiveTab;
  elements.reloadBtn.disabled = !hasActiveTab;
  if (elements.readerBtn) {
    const canReader = Boolean(activeTab && activeTab.url && activeTab.url !== 'about:blank');
    elements.readerBtn.disabled = !canReader;
  }

  const loading = Boolean(activeTab && activeTab.is_loading);
  elements.reloadBtn.innerHTML = loading ? STOP_ICON_SVG : RELOAD_ICON_SVG;
  elements.reloadBtn.title = loading ? 'Stop loading' : 'Reload';
}

function handleReloadButton() {
  const tab = getActiveTab();
  if (tab && tab.is_loading) {
    stopLoading();
  } else {
    reloadPage();
  }
}

async function navigateBack() {
  if (!activeTabId) return;

  try {
    await invokeCommand('webview_back', { tab_id: activeTabId });
  } catch (error) {
    console.error('Failed to navigate back:', error);
  }
}

async function navigateForward() {
  if (!activeTabId) return;

  try {
    await invokeCommand('webview_forward', { tab_id: activeTabId });
  } catch (error) {
    console.error('Failed to navigate forward:', error);
  }
}

async function reloadPage() {
  if (!activeTabId) return;

  try {
    await invokeCommand('reload_webview', { tab_id: activeTabId });
  } catch (error) {
    console.error('Failed to reload page:', error);
  }
}

async function forceReloadPage() {
  if (!activeTabId) return;

  try {
    await invokeCommand('force_reload_webview', { tab_id: activeTabId });
  } catch (error) {
    console.error('Failed to force reload page:', error);
  }
}

async function stopLoading() {
  if (!activeTabId) return;

  try {
    await invokeCommand('stop_webview_loading', { tab_id: activeTabId });
  } catch (error) {
    console.error('Failed to stop loading:', error);
  }
}

// ============================================
// Reader Mode
// ============================================

function loadReaderTypography() {
  try {
    const raw = localStorage.getItem('axiom_reader_typography');
    if (!raw) return;
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === 'object') {
      const fontSize = Number(parsed.fontSize);
      const maxWidth = Number(parsed.maxWidth);
      if (Number.isFinite(fontSize)) readerTypography.fontSize = fontSize;
      if (Number.isFinite(maxWidth)) readerTypography.maxWidth = maxWidth;
    }
  } catch (error) {
    console.warn('Failed to load reader typography:', error);
  }
}

function persistReaderTypography() {
  try {
    localStorage.setItem('axiom_reader_typography', JSON.stringify(readerTypography));
  } catch (error) {
    console.warn('Failed to persist reader typography:', error);
  }
}

function applyReaderTypography() {
  if (!elements.readerOverlay) return;
  const fontSize = Math.round(readerTypography.fontSize);
  const maxWidth = Math.round(readerTypography.maxWidth);
  elements.readerOverlay.style.setProperty('--reader-font-size', `${fontSize}px`);
  elements.readerOverlay.style.setProperty('--reader-max-width', `${maxWidth}px`);
}

function adjustReaderTypography({ fontSizeDelta = 0, widthDelta = 0 }) {
  const nextFont = Math.min(28, Math.max(14, Number(readerTypography.fontSize) + fontSizeDelta));
  const nextWidth = Math.min(980, Math.max(520, Number(readerTypography.maxWidth) + widthDelta));

  readerTypography = { fontSize: nextFont, maxWidth: nextWidth };
  applyReaderTypography();
  persistReaderTypography();
}

function isReaderModeOpen() {
  return Boolean(elements.readerOverlay && !elements.readerOverlay.classList.contains('hidden'));
}

async function toggleReaderMode() {
  if (isReaderModeOpen()) {
    closeReaderMode();
    return;
  }
  await openReaderMode();
}

async function openReaderMode() {
  if (!elements.readerOverlay || !elements.readerContent) return;

  const tab = getActiveTab();
  if (!tab || !tab.url || tab.url === 'about:blank') {
    showToast({ title: 'Reader mode unavailable', message: 'No active page to extract.' });
    return;
  }

  readerSourceTabId = tab.id || activeTabId;
  elements.readerOverlay.classList.remove('hidden');
  applyReaderTypography();
  elements.addressSuggestions.classList.add('hidden');
  elements.readerCloseBtn?.focus();

  if (elements.readerDomain) {
    try {
      const parsed = new URL(tab.url);
      elements.readerDomain.textContent = parsed.host;
    } catch {
      elements.readerDomain.textContent = '';
    }
  }
  if (elements.readerTitle) elements.readerTitle.textContent = tab.title || tab.url;
  elements.readerContent.innerHTML = '<p>Loading reader view…</p>';

  try {
    const result = await invokeCommand('extract_reader', { url: tab.url });
    if (elements.readerDomain) {
      try {
        const parsed = new URL(result.url);
        elements.readerDomain.textContent = parsed.host;
      } catch {
        elements.readerDomain.textContent = '';
      }
    }
    if (elements.readerTitle) {
      const byline = result.byline ? ` · ${result.byline}` : '';
      elements.readerTitle.textContent = `${result.title}${byline}`;
    }
    elements.readerContent.innerHTML = result.content_html || '<p>No content.</p>';
  } catch (error) {
    console.error('Reader mode extraction failed:', error);
    showToast({ title: 'Reader mode failed', message: String(error), timeout: 8000 });
    closeReaderMode();
  }
}

function closeReaderMode({ restoreFocus = true } = {}) {
  if (!elements.readerOverlay || !elements.readerContent) return;
  elements.readerOverlay.classList.add('hidden');
  elements.readerContent.innerHTML = '';
  readerSourceTabId = null;
  if (restoreFocus) {
    elements.readerBtn?.focus();
  }
}

// ============================================
// Bookmarks
// ============================================

function applyBookmarksVisibility(visible) {
  bookmarksBarVisible = visible;
  if (!elements.bookmarksBar) return;

  if (visible) {
    elements.bookmarksBar.classList.remove('hidden');
  } else {
    elements.bookmarksBar.classList.add('hidden');
  }

  scheduleBoundsSync();
}

async function handleBookmarksToggle() {
  const visible = elements.bookmarksToggle.checked;
  applyBookmarksVisibility(visible);

  try {
    await invoke('set_bookmarks_bar_visibility', { visible });
  } catch (error) {
    console.error('Failed to persist bookmarks bar visibility:', error);
  }
}

async function loadBookmarks() {
  try {
    const result = await invoke('get_bookmarks');
    if (result.success) {
      currentBookmarks = Array.isArray(result.data) ? result.data : [];
      renderBookmarks();
    }
  } catch (error) {
    console.error('Failed to load bookmarks:', error);
  }
}

function renderBookmarks() {
  if (!elements.bookmarksList) return;

  elements.bookmarksList.innerHTML = '';

  if (currentBookmarks.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'bookmarks-empty';
    empty.textContent = 'No bookmarks yet';
    elements.bookmarksList.appendChild(empty);
    return;
  }

    currentBookmarks.forEach((bookmark) => {
      const item = document.createElement('button');
    item.className = 'bookmark-item';
    item.type = 'button';
    item.dataset.url = bookmark.url;
    item.title = bookmark.url;

    const title = document.createElement('span');
    title.className = 'bookmark-title';
    title.textContent = bookmark.title || bookmark.url;

      item.appendChild(title);
      item.addEventListener('click', (e) => {
        openUrlWithDisposition(bookmark.url, dispositionFromPointerEvent(e));
      });
      item.addEventListener('auxclick', (e) => {
        if (e.button !== 1) return;
        e.preventDefault();
        openUrlWithDisposition(bookmark.url, 'new_background_tab');
      });

      elements.bookmarksList.appendChild(item);
    });
  }

async function openBookmarksModal() {
  if (!elements.bookmarksModal) return;

  closeSettingsModal();
  elements.bookmarksModal.classList.remove('hidden');
  elements.bookmarksSearch.value = '';
  selectedBookmarkUrl = null;

  try {
    await loadBookmarks();
    await refreshBookmarksFolderFilter();
    startNewBookmark();
    renderBookmarksManager();
  } catch (error) {
    console.error('Failed to open bookmarks manager:', error);
  } finally {
    elements.bookmarksSearch.focus();
  }
}

function closeBookmarksModal() {
  if (!elements.bookmarksModal) return;
  elements.bookmarksModal.classList.add('hidden');
  selectedBookmarkUrl = null;
}

async function refreshBookmarksFolderFilter() {
  if (!elements.bookmarksFolderFilter) return;

  let folders = [];
  try {
    folders = await invokeCommand('get_bookmark_folders');
  } catch (error) {
    folders = [];
  }

  const currentValue = elements.bookmarksFolderFilter.value || 'all';
  elements.bookmarksFolderFilter.innerHTML = '';

  const addOption = (value, label) => {
    const opt = document.createElement('option');
    opt.value = value;
    opt.textContent = label;
    elements.bookmarksFolderFilter.appendChild(opt);
  };

  addOption('all', 'All');
  addOption('__root__', 'No folder');
  folders.forEach((folder) => addOption(folder, folder));

  elements.bookmarksFolderFilter.value = currentValue;
}

function renderBookmarksManager() {
  if (!elements.bookmarksManagerList) return;

  const query = (elements.bookmarksSearch.value || '').trim().toLowerCase();
  const folderFilter = elements.bookmarksFolderFilter.value || 'all';

  const filtered = currentBookmarks.filter((b) => {
    const title = (b.title || '').toLowerCase();
    const url = (b.url || '').toLowerCase();
    const folder = (b.folder || '').toString();

    const matchesQuery = !query || title.includes(query) || url.includes(query) || folder.toLowerCase().includes(query);
    if (!matchesQuery) return false;

    if (folderFilter === 'all') return true;
    if (folderFilter === '__root__') return !b.folder;
    return b.folder === folderFilter || (b.folder && b.folder.startsWith(`${folderFilter}/`));
  });

  elements.bookmarksManagerList.innerHTML = '';

  if (!filtered.length) {
    const empty = document.createElement('div');
    empty.className = 'bookmarks-empty';
    empty.textContent = 'No bookmarks';
    elements.bookmarksManagerList.appendChild(empty);
    return;
  }

  filtered.forEach((bookmark) => {
    const item = document.createElement('button');
    item.type = 'button';
    item.className = 'bookmarks-manager-item';
    item.dataset.url = bookmark.url;
    if (selectedBookmarkUrl && bookmark.url === selectedBookmarkUrl) {
      item.classList.add('active');
    }

    const text = document.createElement('div');
    text.className = 'bookmarks-manager-item-text';

    const title = document.createElement('div');
    title.className = 'bookmarks-manager-item-title';
    title.textContent = bookmark.title || bookmark.url;

    const url = document.createElement('div');
    url.className = 'bookmarks-manager-item-url';
    url.textContent = bookmark.folder ? `${bookmark.folder} · ${bookmark.url}` : bookmark.url;

    text.appendChild(title);
    text.appendChild(url);
    item.appendChild(text);

    item.addEventListener('click', () => {
      selectBookmarkForEditing(bookmark.url);
    });
    item.addEventListener('dblclick', () => {
      openUrlWithDisposition(bookmark.url, 'current_tab');
      closeBookmarksModal();
    });
    item.addEventListener('auxclick', (e) => {
      if (e.button !== 1) return;
      e.preventDefault();
      openUrlWithDisposition(bookmark.url, 'new_background_tab');
      closeBookmarksModal();
    });

    elements.bookmarksManagerList.appendChild(item);
  });
}

function selectBookmarkForEditing(url) {
  const bookmark = currentBookmarks.find((b) => b.url === url);
  if (!bookmark) return;
  selectedBookmarkUrl = bookmark.url;
  elements.bookmarkEditTitle.value = bookmark.title || '';
  elements.bookmarkEditUrl.value = bookmark.url || '';
  elements.bookmarkEditFolder.value = bookmark.folder || '';
  renderBookmarksManager();
}

function startNewBookmark() {
  selectedBookmarkUrl = null;
  elements.bookmarkEditTitle.value = '';
  elements.bookmarkEditUrl.value = '';
  elements.bookmarkEditFolder.value = '';
  renderBookmarksManager();
}

async function saveBookmarkEdits(e) {
  e.preventDefault();

  const title = (elements.bookmarkEditTitle.value || '').trim();
  const url = (elements.bookmarkEditUrl.value || '').trim();
  const folderRaw = (elements.bookmarkEditFolder.value || '').trim();
  const folder = folderRaw ? folderRaw : null;

  try {
    const bookmarks = selectedBookmarkUrl
      ? await invokeCommand('update_bookmark', { old_url: selectedBookmarkUrl, title, url, folder })
      : await invokeCommand('add_bookmark', { title, url, folder });

    currentBookmarks = Array.isArray(bookmarks) ? bookmarks : [];
    renderBookmarks();
    await refreshBookmarksFolderFilter();
    selectedBookmarkUrl = url;
    renderBookmarksManager();
  } catch (error) {
    console.error('Failed to save bookmark:', error);
  }
}

async function deleteSelectedBookmark() {
  if (!selectedBookmarkUrl) return;

  try {
    const bookmarks = await invokeCommand('remove_bookmark', { url: selectedBookmarkUrl });
    currentBookmarks = Array.isArray(bookmarks) ? bookmarks : [];
    renderBookmarks();
    await refreshBookmarksFolderFilter();
    startNewBookmark();
  } catch (error) {
    console.error('Failed to delete bookmark:', error);
  }
}

async function exportBookmarksHtml() {
  try {
    const html = await invokeCommand('export_bookmarks_html');
    const blob = new Blob([html], { type: 'text/html;charset=utf-8' });
    const href = URL.createObjectURL(blob);

    const a = document.createElement('a');
    a.href = href;
    a.download = 'axiom_bookmarks.html';
    a.click();

    URL.revokeObjectURL(href);
  } catch (error) {
    console.error('Failed to export bookmarks:', error);
  }
}

async function importBookmarksFromFile() {
  const file = elements.bookmarksImportFile.files && elements.bookmarksImportFile.files[0];
  if (!file) return;

  try {
    const html = await file.text();
    const bookmarks = await invokeCommand('import_bookmarks_html', { html });
    currentBookmarks = Array.isArray(bookmarks) ? bookmarks : [];
    renderBookmarks();
    await refreshBookmarksFolderFilter();
    renderBookmarksManager();
  } catch (error) {
    console.error('Failed to import bookmarks:', error);
  } finally {
    elements.bookmarksImportFile.value = '';
  }
}

function getActiveTab() {
  return currentTabs.find((tab) => tab.id === activeTabId) || null;
}     

function updateBookmarkActions() {
  if (!elements.addBookmarkBtn) return;
  const activeTab = getActiveTab();
  const canBookmark = activeTab && activeTab.url && activeTab.url !== 'about:blank';
  elements.addBookmarkBtn.disabled = !canBookmark;
}

async function addBookmarkFromActiveTab() {
  const activeTab = getActiveTab();
  if (!activeTab || !activeTab.url || activeTab.url === 'about:blank') return;

  const title = activeTab.title || activeTab.url;
  const url = activeTab.url;

  try {
    const result = await invoke('add_bookmark', { title, url });
    if (result.success) {
      currentBookmarks = result.data;
      renderBookmarks();
    }
  } catch (error) {
    console.error('Failed to add bookmark:', error);
  }
}

async function openBookmark(url) {
  if (!url) return;
  await openUrlWithDisposition(url, 'current_tab');
}

async function openUrlWithDisposition(url, disposition) {
  if (!url) return;

  switch (disposition) {
    case 'new_window':
      await openUrlInNewWindow(url);
      return;
    case 'new_background_tab':
      await openUrlInNewTab(url, true);
      return;
    case 'new_foreground_tab':
      await openUrlInNewTab(url, false);
      return;
    default:
      await navigateToUrl(url);
  }
}

async function openUrlInNewWindow(url) {
  try {
    await invokeCommand('open_url_in_new_window', { url });
  } catch (error) {
    console.error('Failed to open URL in new window:', error);
  }
}

async function openUrlInNewTab(url, background) {
  const command = background ? 'create_tab_background' : 'create_tab';

  try {
    const result = await invoke(command, { url });
    if (!result || !result.success || !result.data) return;

    const tab = result.data;
    if (!background) {
      activeTabId = tab.id;
      await ensureActiveWebview(tab);
      await refreshTabs();
      return;
    }

    await ensureWebview(tab);
    await refreshTabs();
  } catch (error) {
    console.error('Failed to open URL in new tab:', error);
  }
}

function dispositionFromPointerEvent(e) {
  if (!e) return 'current_tab';
  const accel = e.ctrlKey || e.metaKey;

  if (e.shiftKey) return 'new_window';
  if (e.altKey) return 'new_foreground_tab';
  if (accel || e.button === 1) return 'new_background_tab';
  return 'current_tab';
}

function hostForUrl(url) {
  try {
    return new URL(url).host.toLowerCase();
  } catch {
    return '';
  }
}

async function navigateToUrl(url) {
  try {
    const cleanedUrl = await invokeCommand('clean_url', { url });
    const blocked = await invokeCommand('should_block_url', { url: cleanedUrl });
    if (blocked) {
      showToast({
        title: 'Blocked by tracking protection',
        message: cleanedUrl,
        timeout: 6000,
      });
      return;
    }

    const probe = await invokeCommand('probe_url', { url: cleanedUrl });
    if (probe && probe.ok === false) {
      const kind = String(probe.error_kind || 'network').toLowerCase();
      if (kind === 'invalid_url') {
        showToast({
          title: 'Invalid URL',
          message: cleanedUrl,
          timeout: 8000,
        });
        return;
      }
      const title =
        kind === 'dns'
          ? 'DNS error'
          : kind === 'tls'
            ? 'SSL error'
            : kind === 'timeout'
              ? 'Connection timed out'
              : 'Network error';

      showToast({
        title,
        message: cleanedUrl,
        actions: [
          { label: 'Retry', kind: 'primary', onClick: () => openUrlWithDisposition(cleanedUrl, 'current_tab') },
          { label: 'Dismiss', kind: 'secondary' },
        ],
        timeout: 0,
      });
    }

    const targetUrl = probe?.final_url || cleanedUrl;

    if (activeTabId) {
      try {
        await invoke('activate_tab', { tab_id: activeTabId });
      } catch (error) {
        console.warn('Failed to restore active tab:', error);
      }

      const current = getActiveTab();
      const beforeHost = hostForUrl(current?.url || '');
      const afterHost = hostForUrl(targetUrl);
      if (beforeHost && afterHost && beforeHost !== afterHost) {
        try {
          await invokeCommand('close_webview', { tab_id: activeTabId });
        } catch (error) {
          console.warn('Failed to re-partition webview storage:', error);
        }
      }
      await invokeCommand('navigate_tab', { tab_id: activeTabId, url: targetUrl });
      await ensureWebview({ id: activeTabId, url: targetUrl });
      await invokeCommand('navigate_webview', { tab_id: activeTabId, url: targetUrl });
      await refreshTabs();
      return;
    }

    const tabResult = await invoke('create_tab', { url: targetUrl });
    if (tabResult.success) {
      const tab = tabResult.data;
      activeTabId = tab.id;
      await ensureActiveWebview(tab);
      await refreshTabs();
    }
  } catch (error) {
    console.error('Failed to navigate to URL:', error);
    showToast({
      title: 'Navigation failed',
      message: error?.message || String(error || 'Failed to navigate'),
      actions: [
        { label: 'Retry', kind: 'primary', onClick: () => navigateToUrl(url) },
        { label: 'Dismiss', kind: 'secondary' },
      ],
      timeout: 0,
    });
  }
}

// ============================================
// Tab Management
// ============================================

async function refreshTabs() {
  try {
    // Get active tab
    const activeResult = await invoke('get_active_tab');
    let activeTab = null;
    if (activeResult.success && activeResult.data) {
      activeTab = activeResult.data;
      activeTabId = activeTab.id;
      updateAddressBar(activeTab.url);
    } else {
      activeTabId = null;
      updateAddressBar('');
    }

    if (isReaderModeOpen() && readerSourceTabId && activeTabId && readerSourceTabId !== activeTabId) {
      closeReaderMode({ restoreFocus: false });
    }

    const result = await invoke('get_tabs');
    if (result.success) {
      currentTabs = result.data;
      renderTabs();
    }

    if (!activeTab && currentTabs.length > 0) {
      const fallback = await invoke('activate_tab', { tab_id: currentTabs[0].id });
      if (fallback.success && fallback.data) {
        activeTab = fallback.data;
        activeTabId = activeTab.id;
        updateAddressBar(activeTab.url);
      }
    }

    updateEmptyState();
    updateNavigationButtons();
    await syncWebviewBounds();
    await ensureActiveWebview(activeTab);
    updateBookmarkActions();

    const now = Date.now();
    for (const tab of currentTabs) {
      if (tab && tab.id && !tabLastInteractionAt.has(tab.id)) {
        tabLastInteractionAt.set(tab.id, now);
      }
    }
    if (activeTabId) {
      tabLastInteractionAt.set(activeTabId, now);
    }
  } catch (error) {
    console.error('Failed to refresh tabs:', error);
  }
}

function startTabHousekeeping() {
  if (tabHousekeepingHandle) return;

  tabHousekeepingHandle = setInterval(() => {
    tabHousekeepingTick().catch((error) => {
      console.error('Tab housekeeping failed:', error);
    });
  }, 30_000);
}

async function tabHousekeepingTick() {
  if (!Array.isArray(currentTabs) || currentTabs.length === 0) return;

  const now = Date.now();
  let changed = false;

  for (const tab of currentTabs) {
    if (!tab || !tab.id) continue;
    if (tab.id === activeTabId) continue;

    const last = tabLastInteractionAt.get(tab.id);
    if (!last) continue;

    const idleMs = now - last;
    if (tab.state === 'background' && idleMs >= TAB_IDLE_FREEZE_MS) {
      await freezeAndUnloadTab(tab.id);
      tabLastInteractionAt.set(tab.id, now);
      changed = true;
      continue;
    }

    if (tab.state === 'frozen' && idleMs >= TAB_IDLE_DISCARD_MS) {
      await discardAndUnloadTab(tab.id);
      tabLastInteractionAt.set(tab.id, now);
      changed = true;
    }
  }

  if (changed) {
    await refreshTabs();
  }
}

async function freezeAndUnloadTab(tabId) {
  try {
    await invokeCommand('close_webview', { tab_id: tabId });
  } catch (error) {
    console.warn('Failed to close webview for freeze:', error);
  }

  try {
    await invokeCommand('freeze_tab', { tab_id: tabId });
  } catch (error) {
    console.warn('Failed to freeze tab:', error);
  }
}

async function discardAndUnloadTab(tabId) {
  try {
    await invokeCommand('close_webview', { tab_id: tabId });
  } catch (error) {
    console.warn('Failed to close webview for discard:', error);
  }

  try {
    await invokeCommand('discard_tab', { tab_id: tabId });
  } catch (error) {
    console.warn('Failed to discard tab:', error);
  }
}

function cycleTabs(direction) {
  if (!Array.isArray(currentTabs) || currentTabs.length === 0) return;

  const currentIndex = currentTabs.findIndex((t) => t.id === activeTabId);
  const startIndex = currentIndex >= 0 ? currentIndex : 0;
  const nextIndex = (startIndex + direction + currentTabs.length) % currentTabs.length;
  const next = currentTabs[nextIndex];
  if (!next || next.id === activeTabId) return;

  activateTab(next.id);
}

function renderTabs() {
  elements.tabList.innerHTML = '';

  currentTabs.forEach((tab) => {
    const tabEl = createTabElement(tab);
    elements.tabList.appendChild(tabEl);
  });

  updateEmptyState();
}

function createTabElement(tab) {
  const div = document.createElement('div');
  div.className = 'tab-item';
  div.dataset.tabId = tab.id;
  div.draggable = true;

  // Add state classes
  if (tab.id === activeTabId) {
    div.classList.add('active');
  }
  if (tab.is_loading) {
    div.classList.add('loading');
  }
  if (tab.state === 'frozen') {
    div.classList.add('frozen');
  }
  if (tab.state === 'discarded') {
    div.classList.add('discarded');
  }

  // Favicon
  const favicon = document.createElement('div');
  favicon.className = 'tab-favicon';
  if (tab.favicon_url) {
    const img = document.createElement('img');
    img.src = tab.favicon_url;
    img.onerror = () => {
      img.remove();
      favicon.innerHTML = getDefaultFaviconSvg();
    };
    favicon.appendChild(img);
  } else {
    favicon.innerHTML = getDefaultFaviconSvg();
  }

  // Title
  const title = document.createElement('span');
  title.className = 'tab-title';
  title.textContent = tab.title || tab.url || 'New Tab';

  // Close button
  const closeBtn = document.createElement('button');
  closeBtn.className = 'tab-close';
  closeBtn.innerHTML = `
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
      <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
    </svg>
  `;
  closeBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    closeTab(tab.id);
  });

  div.appendChild(favicon);
  div.appendChild(title);
  div.appendChild(closeBtn);

  // Click to activate
  div.addEventListener('click', () => activateTab(tab.id));

  div.addEventListener('dragstart', (e) => handleTabDragStart(e, tab.id));
  div.addEventListener('dragover', (e) => handleTabDragOver(e, tab.id));
  div.addEventListener('drop', (e) => handleTabDrop(e, tab.id));
  div.addEventListener('dragend', handleTabDragEnd);

  return div;
}

function getDefaultFaviconSvg() {
  return `
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
      <circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.5"/>
    </svg>
  `;
}

async function createNewWindow() {
  try {
    await invokeCommand('create_window');
  } catch (error) {
    console.error('Failed to create window:', error);
  }
}

async function createNewTab() {
  try {
    const result = await invoke('create_tab', { url: 'about:blank' });
    if (result.success) {
      const tab = result.data;
      activeTabId = tab.id;

      // Create native webview for this tab
      await ensureActiveWebview(tab);

      await refreshTabs();
      elements.addressBar.focus();
      elements.addressBar.select();
    }
  } catch (error) {
    console.error('Failed to create tab:', error);
  }
}

async function closeTab(tabId) {
  try {
    // Close native webview
    await invokeCommand('close_webview', { tab_id: tabId });

    const result = await invoke('close_tab', { tab_id: tabId });
    if (result.success) {
      await refreshTabs();
    }
  } catch (error) {
    console.error('Failed to close tab:', error);
  }
}

async function detachTabToNewWindow(tabId) {
  if (!tabId) return;

  try {
    await invokeCommand('close_webview', { tab_id: tabId });
    await invokeCommand('detach_tab_to_new_window', { tab_id: tabId });
    await refreshTabs();
  } catch (error) {
    console.error('Failed to detach tab to new window:', error);
  }
}

async function restoreLastClosedTab() {
  try {
    const result = await invoke('restore_last_closed_tab');
    if (result.success && result.data) {
      const tab = result.data;
      activeTabId = tab.id;
      await ensureActiveWebview(tab);
      await refreshTabs();
    }
  } catch (error) {
    console.error('Failed to restore last closed tab:', error);
  }
}

async function activateTab(tabId) {
  try {
    const result = await invoke('activate_tab', { tab_id: tabId });
    if (result.success) {
      activeTabId = tabId;

      await refreshTabs();
    }
  } catch (error) {
    console.error('Failed to activate tab:', error);
  }
}

function handleTabDragStart(e, tabId) {
  draggingTabId = tabId;
  draggingDidDrop = false;
  document.body.classList.add('dragging');

  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/tab-id', tabId);
  }
}

function handleTabDragOver(e, overTabId) {
  e.preventDefault();
  if (!draggingTabId || draggingTabId === overTabId) return;

  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'move';
  }

  const el = e.currentTarget;
  if (el && el.classList) {
    el.classList.add('drag-over');
  }
}

function handleTabDrop(e, overTabId) {
  e.preventDefault();
  const tabId = draggingTabId || e.dataTransfer?.getData('text/tab-id');
  if (!tabId || tabId === overTabId) return;
  draggingDidDrop = true;

  const newIndex = currentTabs.findIndex((t) => t.id === overTabId);
  if (newIndex >= 0) {
    reorderTab(tabId, newIndex);
  }
}

function handleTabListDrop(e) {
  e.preventDefault();
  if (!draggingTabId) return;
  draggingDidDrop = true;

  const onTab = e.target && e.target.closest ? e.target.closest('.tab-item') : null;
  if (onTab) return;

  reorderTab(draggingTabId, currentTabs.length);
}

function handleTabDragEnd(e) {
  const tabId = draggingTabId;
  const didDrop = draggingDidDrop;
  const isOutsideWindow = e
    && (e.clientX < 0 || e.clientX > window.innerWidth || e.clientY < 0 || e.clientY > window.innerHeight);

  draggingTabId = null;
  draggingDidDrop = false;
  document.body.classList.remove('dragging');
  document.querySelectorAll('.tab-item.drag-over').forEach((el) => {
    el.classList.remove('drag-over');
  });

  if (!didDrop && isOutsideWindow && tabId) {
    detachTabToNewWindow(tabId);
  }
}

async function reorderTab(tabId, newIndex) {
  try {
    const result = await invoke('reorder_tab', { tab_id: tabId, new_index: newIndex });
    if (result.success) {
      await refreshTabs();
    }
  } catch (error) {
    console.error('Failed to reorder tab:', error);
  }
}

function updateEmptyState() {
  if (currentTabs.length === 0) {
    elements.emptyState.classList.remove('hidden');
  } else {
    elements.emptyState.classList.add('hidden');
  }
}

function showTabPlaceholder(tab) {
  if (!elements.tabPlaceholder) return;

  if (elements.tabPlaceholderTitle) {
    elements.tabPlaceholderTitle.textContent = tab.state === 'discarded' ? 'Tab discarded' : 'Tab unavailable';
  }
  if (elements.tabPlaceholderUrl) {
    elements.tabPlaceholderUrl.textContent = tab.url || '';
  }

  elements.tabPlaceholder.classList.remove('hidden');
}

function hideTabPlaceholder() {
  if (!elements.tabPlaceholder) return;
  elements.tabPlaceholder.classList.add('hidden');
}

async function restoreDiscardedActiveTab() {
  const tab = getActiveTab();
  if (!tab || !tab.id) return;

  try {
    await invoke('activate_tab', { tab_id: tab.id });
    hideTabPlaceholder();
    await refreshTabs();
  } catch (error) {
    console.error('Failed to restore discarded tab:', error);
  }
}

async function ensureWebview(tab) {
  if (!tab) return;

  const url = tab.url || 'about:blank';
  await invokeCommand('create_webview', { tab_id: tab.id, url });
}

async function ensureActiveWebview(tab) {
  if (!tab) return;

  if (tab.state === 'discarded') {
    showTabPlaceholder(tab);
    try {
      await invokeCommand('close_webview', { tab_id: tab.id });
    } catch (error) {
      console.warn('Failed to close discarded webview:', error);
    }
    return;
  }

  hideTabPlaceholder();
  try {
    await ensureWebview(tab);
  } catch (error) {
    console.error('Failed to ensure webview:', error);
    showToast({
      title: 'Tab failed to initialize',
      message: error?.message || String(error || 'Failed to create webview'),
      actions: [
        { label: 'Retry', kind: 'primary', onClick: () => ensureActiveWebview(tab) },
        { label: 'Dismiss', kind: 'secondary' },
      ],
      timeout: 0,
    });
    return;
  }
  try {
    await invokeCommand('show_webview', { tab_id: tab.id });
  } catch (error) {
    console.error('Failed to show webview:', error);
    showToast({
      title: 'Tab failed to display',
      message: error?.message || String(error || 'Failed to show webview'),
      actions: [
        { label: 'Retry', kind: 'primary', onClick: () => ensureActiveWebview(tab) },
        { label: 'Dismiss', kind: 'secondary' },
      ],
      timeout: 0,
    });
  }
}

function scheduleBoundsSync() {
  if (boundsSyncHandle) return;

  boundsSyncHandle = window.requestAnimationFrame(async () => {
    boundsSyncHandle = null;
    await syncWebviewBounds();
  });
}

async function syncWebviewBounds() {
  if (!elements.webviewContainer) return;

  const rect = elements.webviewContainer.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return;

  const bounds = {
    x: rect.left,
    y: rect.top,
    width: rect.width,
    height: rect.height,
  };

  if (
    lastBounds
    && bounds.x === lastBounds.x
    && bounds.y === lastBounds.y
    && bounds.width === lastBounds.width
    && bounds.height === lastBounds.height
  ) {
    return;
  }

  lastBounds = { ...bounds };

  try {
    await invokeCommand('update_all_webview_bounds', bounds);
  } catch (error) {
    console.error('Failed to update webview bounds:', error);
  }
}

// ============================================
// Address Bar
// ============================================

function updateAddressBar(url) {
  if (url && url !== 'about:blank') {
    elements.addressBar.value = url;
  } else {
    elements.addressBar.value = '';
  }
}

async function handleAddressBarKeydown(e) {
  if (e.key === 'Enter') {
    e.preventDefault();
    const accel = e.ctrlKey || e.metaKey;
    const disposition = e.shiftKey
      ? 'new_window'
      : (e.altKey && accel)
        ? 'new_background_tab'
        : e.altKey
          ? 'new_foreground_tab'
          : 'current_tab';
    await navigateToInput(disposition);
  }
}

async function handleAddressBarInput() {
  const input = elements.addressBar.value.trim();

  if (input.length < 2) {
    elements.addressSuggestions.classList.add('hidden');
    return;
  }

  // Check for command mode
  if (input.startsWith('@')) {
    await showCommandSuggestions(input);
  } else {
    await showHistorySuggestions(input);
  }
}

function handleAddressBarFocus() {
  elements.addressBar.select();
}

function handleAddressBarBlur() {
  // Delay to allow click on suggestions
  setTimeout(() => {
    elements.addressSuggestions.classList.add('hidden');
  }, 200);
}

async function navigateToInput(disposition = 'current_tab') {
  const input = elements.addressBar.value.trim();
  if (!input) return;

  try {
    // Resolve input using backend
    const result = await invoke('resolve_input', { input });
    if (!result.success) return;

    const resolution = result.data;

    if (resolution.type === 'Navigate' || resolution.type === 'Search') {
      await openUrlWithDisposition(resolution.value, disposition);
    } else if (resolution.type === 'Command') {
      await handleCommand(resolution.value);
    }

    elements.addressSuggestions.classList.add('hidden');
  } catch (error) {
    console.error('Navigation failed:', error);
  }
}

async function showCommandSuggestions(input) {
  const trimmed = input.trim();
  const [prefix, ...rest] = trimmed.split(/\s+/);
  const query = rest.join(' ').trim();
  const cmd = (prefix || '').toLowerCase();

  if (cmd === '@tabs') {
    const q = query.toLowerCase();
    const matches = (Array.isArray(currentTabs) ? currentTabs : [])
      .filter((tab) => {
        if (!q) return true;
        const title = (tab.title || '').toLowerCase();
        const url = (tab.url || '').toLowerCase();
        return title.includes(q) || url.includes(q);
      })
      .slice(0, 8);

    if (!matches.length) {
      elements.addressSuggestions.classList.add('hidden');
      return;
    }

    elements.addressSuggestions.innerHTML = matches
      .map(
        (tab) => `
        <div class="suggestion-item" data-tab-id="${tab.id}">
          <div class="suggestion-icon">▣</div>
          <div class="suggestion-text">
            <div class="suggestion-title">${escapeHtml(tab.title || tab.url || 'Tab')}</div>
            <div class="suggestion-url">${escapeHtml(tab.url || '')}</div>
          </div>
        </div>
      `
      )
      .join('');

    elements.addressSuggestions.classList.remove('hidden');

    elements.addressSuggestions.querySelectorAll('.suggestion-item').forEach((el) => {
      el.addEventListener('click', () => {
        const tabId = el.dataset.tabId;
        if (tabId) activateTab(tabId);
        elements.addressSuggestions.classList.add('hidden');
      });
    });

    return;
  }

  if (cmd === '@sessions') {
    try {
      const result = await invoke('get_sessions');
      if (!result.success) {
        elements.addressSuggestions.classList.add('hidden');
        return;
      }

      const q = query.toLowerCase();
      const sessions = Array.isArray(result.data) ? result.data : [];
      const matches = sessions
        .filter((session) => !q || String(session.name || '').toLowerCase().includes(q))
        .slice(0, 8);

      if (!matches.length) {
        elements.addressSuggestions.classList.add('hidden');
        return;
      }

      elements.addressSuggestions.innerHTML = matches
        .map(
          (session) => `
          <div class="suggestion-item" data-session-id="${session.id}">
            <div class="suggestion-icon">@</div>
            <div class="suggestion-text">
              <div class="suggestion-title">${escapeHtml(session.name || 'Session')}</div>
              <div class="suggestion-url">${Number(session.tab_count || 0)} tabs</div>
            </div>
          </div>
        `
        )
        .join('');

      elements.addressSuggestions.classList.remove('hidden');
      elements.addressSuggestions.querySelectorAll('.suggestion-item').forEach((el) => {
        el.addEventListener('click', async () => {
          const sessionId = el.dataset.sessionId;
          if (sessionId) await switchSession(sessionId);
          elements.addressSuggestions.classList.add('hidden');
        });
      });
    } catch (error) {
      console.error('Failed to load sessions:', error);
      elements.addressSuggestions.classList.add('hidden');
    }

    return;
  }

  if (cmd === '@history') {
    const label = query ? `Search history for "${query}"` : 'Open history';
    elements.addressSuggestions.innerHTML = `
      <div class="suggestion-item" data-action="open-history">
        <div class="suggestion-icon">@</div>
        <div class="suggestion-text">
          <div class="suggestion-title">${escapeHtml(label)}</div>
          <div class="suggestion-url">History</div>
        </div>
      </div>
    `;

    elements.addressSuggestions.classList.remove('hidden');
    const item = elements.addressSuggestions.querySelector('.suggestion-item');
    if (item) {
      item.addEventListener('click', () => {
        openHistoryModal(query || '');
        elements.addressSuggestions.classList.add('hidden');
      });
    }
    return;
  }

  const commands = [
    { prefix: '@tabs', description: 'Search open tabs' },
    { prefix: '@history', description: 'Search history' },
    { prefix: '@sessions', description: 'Switch session' },
  ];

  const filtered = commands.filter((cmd) =>
    cmd.prefix.toLowerCase().includes((prefix || input).toLowerCase())
  );

  if (filtered.length === 0) {
    elements.addressSuggestions.classList.add('hidden');
    return;
  }

  elements.addressSuggestions.innerHTML = filtered
    .map(
      (cmd) => `
      <div class="suggestion-item" data-command="${cmd.prefix}">
        <div class="suggestion-icon">@</div>
        <div class="suggestion-text">
          <div class="suggestion-title">${cmd.prefix}</div>
          <div class="suggestion-url">${cmd.description}</div>
        </div>
      </div>
    `
    )
    .join('');

  elements.addressSuggestions.classList.remove('hidden');

  // Add click handlers
  elements.addressSuggestions.querySelectorAll('.suggestion-item').forEach((el) => {
    el.addEventListener('click', () => {
      elements.addressBar.value = el.dataset.command + ' ';
      elements.addressBar.focus();
      elements.addressSuggestions.classList.add('hidden');
    });
  });
}

async function showHistorySuggestions(query) {
  try {
    const result = await invoke('search_history', { query });
    if (!result.success || result.data.length === 0) {
      elements.addressSuggestions.classList.add('hidden');
      return;
    }

    elements.addressSuggestions.innerHTML = result.data
      .slice(0, 8)
      .map(
        (entry) => `
        <div class="suggestion-item" data-url="${entry.url}">
          <div class="suggestion-icon">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.5"/>
              <path d="M8 4v4l2 2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
          </div>
          <div class="suggestion-text">
            <div class="suggestion-title">${escapeHtml(entry.title || entry.url)}</div>
            <div class="suggestion-url">${escapeHtml(entry.url)}</div>
          </div>
        </div>
      `
      )
      .join('');

    elements.addressSuggestions.classList.remove('hidden');

      // Add click handlers
      elements.addressSuggestions.querySelectorAll('.suggestion-item').forEach((el) => {
        el.addEventListener('click', (e) => {
          e.preventDefault();
          openUrlWithDisposition(el.dataset.url, dispositionFromPointerEvent(e));
          elements.addressSuggestions.classList.add('hidden');
        });
        el.addEventListener('auxclick', (e) => {
          if (e.button !== 1) return;
          e.preventDefault();
          openUrlWithDisposition(el.dataset.url, 'new_background_tab');
          elements.addressSuggestions.classList.add('hidden');
        });
      });
  } catch (error) {
    console.error('Failed to search history:', error);
  }
}

async function handleCommand(command) {
  const commandType = String(command?.command_type || '').toLowerCase();
  const query = String(command?.query || '').trim();

  if (commandType === 'tabs') {
    const q = query.toLowerCase();
    const matches = (Array.isArray(currentTabs) ? currentTabs : []).filter((tab) => {
      if (!q) return false;
      const title = (tab.title || '').toLowerCase();
      const url = (tab.url || '').toLowerCase();
      return title.includes(q) || url.includes(q);
    });

    if (!matches.length) {
      showToast({
        title: 'No matching tabs',
        message: query ? `@tabs ${query}` : 'Type @tabs <query>',
        timeout: 2500,
      });
      return;
    }

    activateTab(matches[0].id);
    return;
  }

  if (commandType === 'history') {
    openHistoryModal(query);
    return;
  }

  if (commandType === 'sessions') {
    openSessionModal();
  }
}

// ============================================
// Session Management
// ============================================

function updateSessionDisplay() {
  if (currentSession) {
    elements.sessionName.textContent = currentSession.name;
  }
}

async function openSessionModal() {
  try {
    const result = await invoke('get_sessions');
    if (result.success) {
      renderSessionList(result.data);
    }
    elements.sessionModal.classList.remove('hidden');
    elements.newSessionName.focus();
  } catch (error) {
    console.error('Failed to load sessions:', error);
  }
}

function closeSessionModal() {
  elements.sessionModal.classList.add('hidden');
  elements.newSessionName.value = '';
}

function renderSessionList(sessions) {
  elements.sessionList.innerHTML = sessions
    .map(
      (session) => `
      <div class="session-item ${session.is_active ? 'active' : ''}" data-session-id="${session.id}">
        <span class="session-item-name">${escapeHtml(session.name)}</span>
        <span class="session-item-count">${session.tab_count} tabs</span>
      </div>
    `
    )
    .join('');

  // Add click handlers
  elements.sessionList.querySelectorAll('.session-item').forEach((el) => {
    el.addEventListener('click', () => switchSession(el.dataset.sessionId));
  });
}

async function createNewSession() {
  const name = elements.newSessionName.value.trim();
  if (!name) return;

  try {
    const result = await invoke('create_session', { name });
    if (result.success) {
      elements.newSessionName.value = '';
      await switchSession(result.data.id);
    }
  } catch (error) {
    console.error('Failed to create session:', error);
  }
}

  async function switchSession(sessionId) {
  try {
    const result = await invoke('switch_session', { session_id: sessionId });
    if (result.success) {
      currentSession = result.data;
      updateSessionDisplay();
      await refreshTabs();
      closeSessionModal();
    }
  } catch (error) {
    console.error('Failed to switch session:', error);
  }
  }

// ============================================
// History
// ============================================

let historySearchHandle = null;

async function openHistoryModal(initialQuery = '') {
  if (!elements.historyModal) return;

  elements.historyModal.classList.remove('hidden');
  handleHistoryClearRangeChange();
  elements.historySearch.value = initialQuery;
  elements.historyList.innerHTML = '';

  try {
    await loadHistoryList(initialQuery);
  } finally {
    elements.historySearch.focus();
  }
}

function closeHistoryModal() {
  if (!elements.historyModal) return;
  elements.historyModal.classList.add('hidden');
}

// ============================================
// Downloads
// ============================================

function handleNewWindowRequested(payload) {
  const url = typeof payload === 'string' ? payload : payload?.url;
  if (!url) return;

  (async () => {
    let cleaned = url;
    try {
      cleaned = await invokeCommand('clean_url', { url });
    } catch {
      cleaned = url;
    }

    try {
      const blocked = await invokeCommand('should_block_url', { url: cleaned });
      if (blocked) {
        showToast({ title: 'Popup blocked', message: cleaned, timeout: 6000 });
        return;
      }
    } catch {}

    showToast({
      title: 'Popup requested',
      message: cleaned,
      actions: [
        { label: 'Open tab', kind: 'primary', onClick: () => openUrlWithDisposition(cleaned, 'new_foreground_tab') },
        { label: 'Background tab', kind: 'secondary', onClick: () => openUrlWithDisposition(cleaned, 'new_background_tab') },
        { label: 'Open window', kind: 'secondary', onClick: () => openUrlWithDisposition(cleaned, 'new_window') },
        { label: 'Dismiss', kind: 'secondary' },
      ],
      timeout: 0,
    });
  })().catch((error) => console.error('Popup handler failed:', error));
}

async function openDownloadsModal() {
  if (!elements.downloadsModal || !elements.downloadsList) return;

  elements.downloadsModal.classList.remove('hidden');
  elements.downloadsList.innerHTML = '';
  await refreshDownloads();
}

function closeDownloadsModal() {
  if (!elements.downloadsModal) return;
  elements.downloadsModal.classList.add('hidden');
}

function isDownloadsModalOpen() {
  return Boolean(elements.downloadsModal && !elements.downloadsModal.classList.contains('hidden'));
}

async function refreshDownloads() {
  if (!elements.downloadsList) return;

  try {
    const downloads = await invokeCommand('list_downloads');
    currentDownloads = Array.isArray(downloads) ? downloads : [];

    downloadStateById.clear();
    for (const download of currentDownloads) {
      if (download && download.id && download.state) {
        downloadStateById.set(download.id, download.state);
      }
    }

    renderDownloadsList();
  } catch (error) {
    console.error('Failed to refresh downloads:', error);
  }
}

function handleDownloadUpdated(download) {
  if (!download || !download.id) return;

  const previousState = downloadStateById.get(download.id);
  downloadStateById.set(download.id, download.state);
  upsertDownload(download);

  if (isDownloadsModalOpen()) {
    renderDownloadsList();
  }

  if (!previousState && download.state === 'pending') {
    showDownloadConsentToast(download);
    return;
  }

  if (previousState !== download.state && download.state === 'completed') {
    showToast({
      title: 'Download completed',
      message: download.file_name || 'Completed',
      actions: [
        {
          label: 'Show',
          kind: 'primary',
          onClick: () => revealDownload(download.id),
        },
        { label: 'Dismiss', kind: 'secondary' },
      ],
      timeout: 8000,
    });
    return;
  }

  if (previousState !== download.state && download.state === 'failed') {
    showToast({
      title: 'Download failed',
      message: download.file_name || 'Failed',
      actions: [
        {
          label: download.can_resume ? 'Resume' : 'Retry',
          kind: 'primary',
          onClick: () => (download.can_resume ? resumeDownload(download.id) : startDownload(download.id)),
        },
        { label: 'Dismiss', kind: 'secondary' },
      ],
      timeout: 10000,
    });
  }
}

function upsertDownload(download) {
  const index = currentDownloads.findIndex((d) => d.id === download.id);
  if (index >= 0) {
    currentDownloads[index] = download;
  } else {
    currentDownloads.unshift(download);
  }

  currentDownloads.sort((a, b) => (b.created_at || '').localeCompare(a.created_at || ''));
}

function renderDownloadsList() {
  if (!elements.downloadsList) return;

  elements.downloadsList.innerHTML = '';

  if (!currentDownloads.length) {
    const empty = document.createElement('div');
    empty.className = 'downloads-empty';
    empty.textContent = 'No downloads yet.';
    elements.downloadsList.appendChild(empty);
    return;
  }

  for (const download of currentDownloads) {
    elements.downloadsList.appendChild(createDownloadElement(download));
  }
}

function createDownloadElement(download) {
  const row = document.createElement('div');
  row.className = 'download-item';

  const icon = document.createElement('div');
  icon.className = 'download-item-icon';
  icon.innerHTML = `
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
      <path d="M8 2v7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      <path d="M5 7l3 3 3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M3 13.5h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
    </svg>
  `;

  const main = document.createElement('div');
  main.className = 'download-item-main';

  const title = document.createElement('div');
  title.className = 'download-item-title';

  const titleText = document.createElement('span');
  titleText.textContent = download.file_name || 'download';
  title.appendChild(titleText);

  const badge = createRiskBadge(download);
  if (badge) title.appendChild(badge);

  const subtitle = document.createElement('div');
  subtitle.className = 'download-item-subtitle';
  subtitle.textContent = download.url || '';

  const progress = document.createElement('div');
  progress.className = 'download-progress';
  const bar = document.createElement('div');
  bar.className = 'download-progress-bar';
  bar.style.width = `${Math.max(0, Math.min(100, Number(download.progress || 0)))}%`;
  progress.appendChild(bar);

  const metaRow = document.createElement('div');
  metaRow.className = 'download-meta-row';

  const metaLeft = document.createElement('div');
  metaLeft.className = 'download-meta-left';

  const state = document.createElement('span');
  state.className = 'download-state';
  state.textContent = prettyDownloadState(download);

  const bytes = document.createElement('span');
  bytes.className = 'download-bytes';
  bytes.textContent = formatDownloadBytes(download);

  metaLeft.appendChild(state);
  metaLeft.appendChild(bytes);

  const actions = document.createElement('div');
  actions.className = 'download-actions';
  for (const btn of createDownloadActionButtons(download)) {
    actions.appendChild(btn);
  }

  metaRow.appendChild(metaLeft);
  metaRow.appendChild(actions);

  main.appendChild(title);
  main.appendChild(subtitle);
  if (download.state === 'downloading' || download.state === 'paused' || download.state === 'pending') {
    main.appendChild(progress);
  }
  main.appendChild(metaRow);

  row.appendChild(icon);
  row.appendChild(main);
  return row;
}

function createRiskBadge(download) {
  if (!download.needs_warning) return null;

  const badge = document.createElement('span');
  badge.className = 'badge';

  if (download.risk_level === 'dangerous') {
    badge.classList.add('badge-dangerous');
    badge.textContent = 'Danger';
    return badge;
  }

  badge.classList.add('badge-warning');
  badge.textContent = 'Warning';
  return badge;
}

function prettyDownloadState(download) {
  const state = String(download.state || '').toLowerCase();
  if (state === 'pending') return 'Pending';
  if (state === 'downloading') return 'Downloading';
  if (state === 'paused') return 'Paused';
  if (state === 'completed') return 'Completed';
  if (state === 'failed') return 'Failed';
  if (state === 'cancelled') return 'Cancelled';
  return state || 'Unknown';
}

function formatDownloadBytes(download) {
  const downloaded = formatBytes(Number(download.downloaded_bytes || 0));
  if (download.total_bytes) {
    const total = formatBytes(Number(download.total_bytes));
    return `${downloaded} / ${total}`;
  }
  return downloaded;
}

function createDownloadActionButtons(download) {
  const buttons = [];
  const state = String(download.state || '').toLowerCase();

  if (state === 'pending') {
    buttons.push(makeDownloadActionButton('Start', () => startDownload(download.id), 'primary'));
    buttons.push(makeDownloadActionButton('Cancel', () => cancelDownload(download.id), 'secondary'));
  } else if (state === 'downloading') {
    buttons.push(makeDownloadActionButton('Pause', () => pauseDownload(download.id), 'secondary'));
    buttons.push(makeDownloadActionButton('Cancel', () => cancelDownload(download.id), 'secondary'));
  } else if (state === 'paused') {
    buttons.push(makeDownloadActionButton('Resume', () => resumeDownload(download.id), 'primary'));
    buttons.push(makeDownloadActionButton('Cancel', () => cancelDownload(download.id), 'secondary'));
  } else if (state === 'failed') {
    buttons.push(makeDownloadActionButton(download.can_resume ? 'Resume' : 'Retry', () => (download.can_resume ? resumeDownload(download.id) : startDownload(download.id)), 'primary'));
    buttons.push(makeDownloadActionButton('Cancel', () => cancelDownload(download.id), 'secondary'));
  } else if (state === 'completed') {
    buttons.push(makeDownloadActionButton('Show', () => revealDownload(download.id), 'primary'));
  }

  return buttons;
}

function makeDownloadActionButton(label, onClick, kind) {
  const button = document.createElement('button');
  button.className = 'download-action-btn';
  if (kind === 'primary') {
    button.style.borderColor = 'rgba(110, 158, 255, 0.35)';
  }
  button.textContent = label;
  button.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    onClick();
  });
  return button;
}

async function startDownload(downloadId) {
  try {
    const updated = await invokeCommand('start_download', { download_id: downloadId });
    handleDownloadUpdated(updated);
  } catch (error) {
    console.error('Failed to start download:', error);
  }
}

async function pauseDownload(downloadId) {
  try {
    const updated = await invokeCommand('pause_download', { download_id: downloadId });
    handleDownloadUpdated(updated);
  } catch (error) {
    console.error('Failed to pause download:', error);
  }
}

async function resumeDownload(downloadId) {
  try {
    const updated = await invokeCommand('resume_download', { download_id: downloadId });
    handleDownloadUpdated(updated);
  } catch (error) {
    console.error('Failed to resume download:', error);
  }
}

async function cancelDownload(downloadId) {
  try {
    const updated = await invokeCommand('cancel_download', { download_id: downloadId });
    handleDownloadUpdated(updated);
  } catch (error) {
    console.error('Failed to cancel download:', error);
  }
}

async function revealDownload(downloadId) {
  try {
    await invokeCommand('reveal_download', { download_id: downloadId });
  } catch (error) {
    console.error('Failed to reveal download:', error);
  }
}

function showDownloadConsentToast(download) {
  const risk = download.risk_level === 'dangerous' ? 'Danger' : download.risk_level === 'warning' ? 'Warning' : 'Safe';
  const title = download.needs_warning ? `Download requested (${risk})` : 'Download requested';

  showToast({
    title,
    message: download.file_name || download.url || 'Download',
    actions: [
      { label: 'Start', kind: 'primary', onClick: () => startDownload(download.id) },
      { label: 'Cancel', kind: 'secondary', onClick: () => cancelDownload(download.id) },
    ],
    timeout: 0,
  });
}

function handleHistoryClearRangeChange() {
  const range = elements.historyClearRange.value;
  const isCustom = range === 'custom';
  elements.historyClearStart.disabled = !isCustom;
  elements.historyClearEnd.disabled = !isCustom;
  elements.historyClearStart.style.display = isCustom ? '' : 'none';
  elements.historyClearEnd.style.display = isCustom ? '' : 'none';

  if (!isCustom) {
    elements.historyClearStart.value = '';
    elements.historyClearEnd.value = '';
  }
}

function handleHistorySearchInput() {
  if (historySearchHandle) clearTimeout(historySearchHandle);
  historySearchHandle = setTimeout(async () => {
    historySearchHandle = null;
    await loadHistoryList(elements.historySearch.value.trim());
  }, 150);
}

async function clearHistoryFromUI() {
  const range = elements.historyClearRange.value;
  const now = new Date();

  let start = null;
  let end = null;

  if (range === 'custom') {
    const startValue = elements.historyClearStart.value;
    const endValue = elements.historyClearEnd.value;
    start = startValue ? new Date(startValue).toISOString() : null;
    end = endValue ? new Date(endValue).toISOString() : null;
  } else if (range !== 'all') {
    const deltaMs = {
      '15m': 15 * 60 * 1000,
      '1h': 60 * 60 * 1000,
      '24h': 24 * 60 * 60 * 1000,
      '7d': 7 * 24 * 60 * 60 * 1000,
      '30d': 30 * 24 * 60 * 60 * 1000,
    }[range];

    if (deltaMs) {
      start = new Date(now.getTime() - deltaMs).toISOString();
      end = now.toISOString();
    }
  }

  try {
    await invokeCommand('clear_history_range', { start, end });
    await loadHistoryList(elements.historySearch.value.trim());
  } catch (error) {
    console.error('Failed to clear history:', error);
  }
}

async function loadHistoryList(query) {
  try {
    const result = query
      ? await invoke('search_history', { query })
      : await invoke('get_recent_history');

    if (!result || !result.success) return;

    const entries = Array.isArray(result.data) ? result.data : [];
    renderHistoryList(entries);
  } catch (error) {
    console.error('Failed to load history:', error);
  }
}

function renderHistoryList(entries) {
  elements.historyList.innerHTML = '';

  if (!entries.length) {
    const empty = document.createElement('div');
    empty.className = 'bookmarks-empty';
    empty.textContent = 'No history entries';
    elements.historyList.appendChild(empty);
    return;
  }

  entries.forEach((entry) => {
    const item = document.createElement('button');
    item.type = 'button';
    item.className = 'history-item';
    item.dataset.url = entry.url;

    const text = document.createElement('div');
    text.className = 'history-item-text';

    const title = document.createElement('div');
    title.className = 'history-item-title';
    title.textContent = entry.title || entry.url;

    const url = document.createElement('div');
    url.className = 'history-item-url';
    url.textContent = entry.url;

    text.appendChild(title);
    text.appendChild(url);

    const meta = document.createElement('div');
    meta.className = 'history-item-meta';
    const when = entry.visited_at ? formatRelativeTime(entry.visited_at) : '';
    const count = typeof entry.visit_count === 'number' ? entry.visit_count : 0;
    meta.textContent = [when, count ? `${count}×` : ''].filter(Boolean).join(' · ');

    item.appendChild(text);
    item.appendChild(meta);

    item.addEventListener('click', (e) => {
      openUrlWithDisposition(entry.url, dispositionFromPointerEvent(e));
      closeHistoryModal();
    });
    item.addEventListener('auxclick', (e) => {
      if (e.button !== 1) return;
      e.preventDefault();
      openUrlWithDisposition(entry.url, 'new_background_tab');
      closeHistoryModal();
    });

    elements.historyList.appendChild(item);
  });
}

function formatRelativeTime(iso) {
  const dt = new Date(iso);
  if (Number.isNaN(dt.getTime())) return '';

  const delta = Date.now() - dt.getTime();
  const seconds = Math.floor(delta / 1000);
  if (seconds < 60) return `${seconds}s ago`;

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;

  return dt.toLocaleDateString();
}

  // ============================================
  // Utilities
  // ============================================

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function formatBytes(bytes) {
  const value = Number(bytes);
  if (!Number.isFinite(value) || value <= 0) return '0 B';

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let unitIndex = 0;
  let size = value;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  const precision = unitIndex === 0 ? 0 : size < 10 ? 2 : size < 100 ? 1 : 0;
  return `${size.toFixed(precision)} ${units[unitIndex]}`;
}

function showToast({ title, message, actions = [], timeout = 6000 }) {
  if (!elements.toastContainer) return;

  const toast = document.createElement('div');
  toast.className = 'toast';

  const titleEl = document.createElement('div');
  titleEl.className = 'toast-title';
  titleEl.textContent = title || 'Notification';

  const messageEl = document.createElement('div');
  messageEl.className = 'toast-message';
  messageEl.textContent = message || '';

  const actionsEl = document.createElement('div');
  actionsEl.className = 'toast-actions';

  for (const action of actions) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = action.kind === 'primary' ? 'btn-primary' : 'btn-secondary';
    button.textContent = action.label || 'OK';
    button.addEventListener('click', async () => {
      try {
        if (typeof action.onClick === 'function') {
          await action.onClick();
        }
      } catch (error) {
        console.error('Toast action failed:', error);
      } finally {
        toast.remove();
      }
    });
    actionsEl.appendChild(button);
  }

  toast.appendChild(titleEl);
  if (message) toast.appendChild(messageEl);
  if (actions.length) toast.appendChild(actionsEl);

  elements.toastContainer.appendChild(toast);

  while (elements.toastContainer.children.length > 4) {
    elements.toastContainer.firstElementChild?.remove();
  }

  if (timeout && timeout > 0) {
    setTimeout(() => toast.remove(), timeout);
  }
}

function showFatalError(title, details) {
  const existing = document.getElementById('fatal-error');
  if (existing) existing.remove();

  const wrapper = document.createElement('div');
  wrapper.id = 'fatal-error';
  wrapper.className = 'fatal-error';

  const safeTitle = escapeHtml(title || 'Error');
  const safeDetails = details ? escapeHtml(details) : '';

  wrapper.innerHTML = `
    <div class="fatal-error-card">
      <div class="fatal-error-title">${safeTitle}</div>
      ${safeDetails ? `<div class="fatal-error-details">${safeDetails}</div>` : ''}
    </div>
  `;

  document.body.appendChild(wrapper);
}
