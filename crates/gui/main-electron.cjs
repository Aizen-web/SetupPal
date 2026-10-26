/**
 * SetupPal: Native Desktop Application Entrypoint (Electron)
 * Window spec from ui.md Section 5: 1100 x 720 (minimum 900 x 600)
 */

const { app, BrowserWindow, ipcMain, shell } = require('electron');
const path = require('path');

let mainWindow;

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 720,
    minWidth: 900,
    minHeight: 600,
    backgroundColor: '#F5EEDF',
    title: 'SetupPal: Zero-Trust Orchestrator',
    autoHideMenuBar: true,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.cjs')
    }
  });

  // Check if Vite dev server is running or load from dist
  const devServerUrl = process.env.VITE_DEV_SERVER_URL || 'http://localhost:5173';
  
  mainWindow.loadURL(devServerUrl).catch(() => {
    // Fallback to local built index.html
    mainWindow.loadFile(path.join(__dirname, 'dist', 'index.html'));
  });

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url);
    return { action: 'deny' };
  });

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

app.whenReady().then(() => {
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
