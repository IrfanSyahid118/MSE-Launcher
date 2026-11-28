const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event

const STRINGS = {
  id: {
    installTitle: 'Pasang MSE Launcher',
    installSubtitle: 'Pilih mode instalasi dan folder tujuan sesuai kebutuhan Anda.',
    stdTitle: 'Instalasi Penuh',
    stdBadge: 'Disarankan',
    stdDesc: 'Pasang ke folder aplikasi lokal lengkap dengan shortcut dan uninstaller.',
    portTitle: 'Mode Portabel',
    portBadge: 'Portabel',
    portDesc: 'Ekstrak langsung ke folder pilihan Anda lengkap dengan penanda portable.txt.',
    pathLabelInstall: 'Folder Instalasi',
    pathLabelPortable: 'Folder Tujuan Portabel',
    browse: 'Pilih Folder...',
    hintInstall: 'Akan membuat uninstall.exe agar pembaruan otomatis tetap berjalan.',
    hintPortable: 'Anda bebas memilih folder mana saja (mis. USB, Desktop, atau drive lain).',
    desktopShortcut: 'Buat shortcut di Desktop',
    startShortcut: 'Buat shortcut di Start Menu',
    cancel: 'Batal',
    startInstall: 'Pasang Sekarang',
    startPortable: 'Ekstrak Sekarang',
    noPayload:
      'File installer ini tidak berisi paket aplikasi (mode stub/pengembangan). Jalankan skrip pengemasan untuk menyematkan MSE Launcher.exe.',
    uninstallTitle: 'Hapus MSE Launcher',
    uninstallSubtitle: 'Program, shortcut, dan uninstaller di folder ini akan dihapus dari perangkat Anda.',
    uninstallDirLabel: 'Lokasi Terpasang',
    removeDataLabel: 'Hapus juga data launcher (instance, akun, pengaturan)',
    removeDataHint: 'Biarkan tidak dicentang jika Anda hanya ingin memasang ulang atau memperbarui.',
    startUninstall: 'Hapus Sekarang',
    progressInstallTitle: 'Sedang Memasang MSE Launcher...',
    progressPortableTitle: 'Sedang Mengekstrak Mode Portabel...',
    progressUninstallTitle: 'Sedang Menghapus MSE Launcher...',
    stages: {
      closing: 'Menutup MSE Launcher yang sedang berjalan...',
      extracting: 'Mengekstrak berkas aplikasi...',
      uninstaller: 'Menyiapkan uninstaller...',
      portable: 'Menulis konfigurasi portabel...',
      shortcuts: 'Membuat shortcut...',
      registry: 'Memperbarui entri Windows...',
      removing: 'Menghapus berkas aplikasi...',
      done: 'Selesai!',
    },
    doneInstallTitle: 'MSE Launcher Siap Digunakan',
    doneInstallSub: 'Aplikasi berhasil dipasang di folder tujuan Anda.',
    donePortableTitle: 'Ekstrak Portabel Selesai',
    donePortableSub: 'MSE Launcher Portable siap dijalankan dari folder pilihan Anda.',
    doneUninstallTitle: 'Penghapusan Selesai',
    doneUninstallSub: 'MSE Launcher telah dihapus dari sistem Anda.',
    launchAfter: 'Buka MSE Launcher sekarang',
    finish: 'Selesai',
    errorTitle: 'Terjadi Kesalahan',
    back: 'Kembali',
    close: 'Tutup',
    folderDialogTitle: 'Pilih Folder Tujuan',
  },
  en: {
    installTitle: 'Install MSE Launcher',
    installSubtitle: 'Choose your installation mode and destination folder.',
    stdTitle: 'Standard Install',
    stdBadge: 'Recommended',
    stdDesc: 'Installs locally with shortcuts and an uninstaller.',
    portTitle: 'Portable Mode',
    portBadge: 'Portable',
    portDesc: 'Extracts directly to any folder you choose with a portable.txt marker.',
    pathLabelInstall: 'Install Location',
    pathLabelPortable: 'Portable Destination Folder',
    browse: 'Browse...',
    hintInstall: 'Includes uninstall.exe so built-in auto-updates continue to work.',
    hintPortable: 'Choose any folder you like (Desktop, secondary drive, or USB).',
    desktopShortcut: 'Create Desktop shortcut',
    startShortcut: 'Create Start Menu shortcut',
    cancel: 'Cancel',
    startInstall: 'Install Now',
    startPortable: 'Extract Now',
    noPayload:
      'This installer binary has no embedded app payload (development stub). Run the packaging script to embed MSE Launcher.exe.',
    uninstallTitle: 'Uninstall MSE Launcher',
    uninstallSubtitle: 'The application and shortcuts will be removed from this device.',
    uninstallDirLabel: 'Installed Location',
    removeDataLabel: 'Also delete launcher data (instances, accounts, settings)',
    removeDataHint: 'Leave unchecked if you only plan to reinstall or upgrade.',
    startUninstall: 'Uninstall',
    progressInstallTitle: 'Installing MSE Launcher...',
    progressPortableTitle: 'Extracting Portable Build...',
    progressUninstallTitle: 'Uninstalling MSE Launcher...',
    stages: {
      closing: 'Closing running MSE Launcher instances...',
      extracting: 'Extracting application files...',
      uninstaller: 'Writing uninstaller...',
      portable: 'Writing portable marker...',
      shortcuts: 'Creating shortcuts...',
      registry: 'Updating Windows registration...',
      removing: 'Removing application files...',
      done: 'Completed!',
    },
    doneInstallTitle: 'MSE Launcher Is Ready',
    doneInstallSub: 'The application has been installed to your selected folder.',
    donePortableTitle: 'Portable Extraction Complete',
    donePortableSub: 'MSE Launcher Portable is ready in your chosen folder.',
    doneUninstallTitle: 'Uninstall Complete',
    doneUninstallSub: 'MSE Launcher has been removed from your device.',
    launchAfter: 'Launch MSE Launcher now',
    finish: 'Finish',
    errorTitle: 'Installation Failed',
    back: 'Back',
    close: 'Close',
    folderDialogTitle: 'Select Destination Folder',
  },
}

const appState = {
  lang: 'id',
  mode: 'install', // 'install' | 'uninstall'
  installMode: 'install', // 'install' | 'portable'
  hasPayload: false,
  installDir: '',
  portableDir: '',
  uninstallDir: '',
  installedExePath: null,
}

const $ = (id) => document.getElementById(id)

function showView(viewId) {
  for (const id of ['view-install', 'view-uninstall', 'view-progress', 'view-done', 'view-error']) {
    $(id).classList.toggle('hidden', id !== viewId)
  }
}

function t() {
  return STRINGS[appState.lang] || STRINGS.en
}

function applyTranslations() {
  const s = t()
  $('lang-id').classList.toggle('active', appState.lang === 'id')
  $('lang-en').classList.toggle('active', appState.lang === 'en')

  $('install-title').textContent = s.installTitle
  $('install-subtitle').textContent = s.installSubtitle
  $('mode-std-title').textContent = s.stdTitle
  $('mode-std-badge').textContent = s.stdBadge
  $('mode-std-desc').textContent = s.stdDesc
  $('mode-port-title').textContent = s.portTitle
  $('mode-port-badge').textContent = s.portBadge
  $('mode-port-desc').textContent = s.portDesc

  const isPortable = appState.installMode === 'portable'
  $('path-label').textContent = isPortable ? s.pathLabelPortable : s.pathLabelInstall
  $('btn-browse').textContent = s.browse
  $('path-hint').textContent = isPortable ? s.hintPortable : s.hintInstall
  $('lbl-desktop').textContent = s.desktopShortcut
  $('lbl-start').textContent = s.startShortcut
  $('row-start-sc').classList.toggle('hidden', isPortable)
  $('btn-cancel-install').textContent = s.cancel
  $('btn-start-install').textContent = isPortable ? s.startPortable : s.startInstall

  $('uninstall-title').textContent = s.uninstallTitle
  $('uninstall-subtitle').textContent = s.uninstallSubtitle
  $('uninstall-dir-label').textContent = s.uninstallDirLabel
  $('lbl-remove-data').textContent = s.removeDataLabel
  $('uninstall-data-hint').textContent = s.removeDataHint
  $('btn-cancel-uninstall').textContent = s.cancel
  $('btn-start-uninstall').textContent = s.startUninstall

  $('lbl-launch-after').textContent = s.launchAfter
  $('btn-finish').textContent = s.finish
  $('error-title').textContent = s.errorTitle
  $('btn-error-back').textContent = s.back
  $('btn-error-close').textContent = s.close

  if (!appState.hasPayload && appState.mode === 'install') {
    $('payload-warning').textContent = s.noPayload
    $('payload-warning').classList.remove('hidden')
    $('btn-start-install').disabled = true
  } else {
    $('payload-warning').classList.add('hidden')
    $('btn-start-install').disabled = false
  }
}

function setInstallMode(nextMode) {
  const input = $('path-input')
  if (appState.installMode === 'install') {
    appState.installDir = input.value
  } else {
    appState.portableDir = input.value
  }

  appState.installMode = nextMode
  $('mode-standard').classList.toggle('active', nextMode === 'install')
  $('mode-portable').classList.toggle('active', nextMode === 'portable')
  input.value = nextMode === 'portable' ? appState.portableDir : appState.installDir
  applyTranslations()
}

async function init() {
  const state = await invoke('installer_get_state')
  appState.lang = state.defaultLang === 'id' ? 'id' : 'en'
  appState.mode = state.mode
  appState.hasPayload = state.hasPayload
  appState.installDir = state.defaultInstallDir
  appState.portableDir = state.defaultPortableDir
  appState.uninstallDir = state.uninstallDir || ''

  $('version-pill').textContent = state.appVersion
  $('path-input').value = appState.installDir
  $('uninstall-dir-value').textContent = appState.uninstallDir

  applyTranslations()
  showView(appState.mode === 'uninstall' ? 'view-uninstall' : 'view-install')

  await listen('installer:progress', (event) => {
    const { percent, stage } = event.payload
    $('progress-fill').style.width = `${percent}%`
    $('progress-pct').textContent = `${percent}%`
    $('progress-stage').textContent = t().stages[stage] || stage
  })
}

$('lang-id').addEventListener('click', () => {
  appState.lang = 'id'
  applyTranslations()
})

$('lang-en').addEventListener('click', () => {
  appState.lang = 'en'
  applyTranslations()
})

$('btn-minimize').addEventListener('click', () => invoke('installer_window_minimize'))
$('btn-close').addEventListener('click', () => invoke('installer_window_close'))
$('btn-cancel-install').addEventListener('click', () => invoke('installer_window_close'))
$('btn-cancel-uninstall').addEventListener('click', () => invoke('installer_window_close'))
$('btn-error-close').addEventListener('click', () => invoke('installer_window_close'))

$('btn-error-back').addEventListener('click', () => {
  showView(appState.mode === 'uninstall' ? 'view-uninstall' : 'view-install')
})

$('mode-standard').addEventListener('click', () => setInstallMode('install'))
$('mode-portable').addEventListener('click', () => setInstallMode('portable'))

$('btn-browse').addEventListener('click', async () => {
  const current = $('path-input').value.trim()
  const picked = await invoke('installer_pick_folder', {
    initialPath: current || null,
    title: t().folderDialogTitle,
  })
  if (picked) {
    $('path-input').value = picked
    if (appState.installMode === 'portable') {
      appState.portableDir = picked
    } else {
      appState.installDir = picked
    }
  }
})

$('btn-start-install').addEventListener('click', async () => {
  const targetDir = $('path-input').value.trim()
  if (!targetDir) return

  const s = t()
  const isPortable = appState.installMode === 'portable'
  $('progress-title').textContent = isPortable ? s.progressPortableTitle : s.progressInstallTitle
  $('progress-fill').style.width = '0%'
  $('progress-pct').textContent = '0%'
  $('progress-stage').textContent = ''
  showView('view-progress')

  try {
    const done = await invoke('installer_run_install', {
      options: {
        installMode: appState.installMode,
        targetDir,
        desktopShortcut: $('chk-desktop').checked,
        startMenuShortcut: !isPortable && $('chk-start').checked,
      },
    })
    appState.installedExePath = done.installedExePath
    $('done-title').textContent = isPortable ? s.donePortableTitle : s.doneInstallTitle
    $('done-subtitle').textContent = isPortable ? s.donePortableSub : s.doneInstallSub
    $('row-launch-after').classList.remove('hidden')
    showView('view-done')
  } catch (err) {
    $('error-message').textContent = String(err)
    showView('view-error')
  }
})

$('btn-start-uninstall').addEventListener('click', async () => {
  const s = t()
  $('progress-title').textContent = s.progressUninstallTitle
  $('progress-fill').style.width = '0%'
  $('progress-pct').textContent = '0%'
  $('progress-stage').textContent = ''
  showView('view-progress')

  try {
    await invoke('installer_run_uninstall', {
      removeUserData: $('chk-remove-data').checked,
    })
    appState.installedExePath = null
    $('done-title').textContent = s.doneUninstallTitle
    $('done-subtitle').textContent = s.doneUninstallSub
    $('row-launch-after').classList.add('hidden')
    showView('view-done')
  } catch (err) {
    $('error-message').textContent = String(err)
    showView('view-error')
  }
})

$('btn-finish').addEventListener('click', async () => {
  const shouldLaunch =
    appState.mode === 'install' &&
    $('chk-launch-after').checked &&
    Boolean(appState.installedExePath)
  await invoke('installer_launch_and_exit', {
    exePath: shouldLaunch ? appState.installedExePath : null,
  })
})

init().catch((err) => {
  $('error-message').textContent = String(err)
  showView('view-error')
})
